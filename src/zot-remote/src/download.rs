use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::path::Path;

use reqwest::header::{CONTENT_LENGTH, CONTENT_TYPE, LOCATION};
use reqwest::{StatusCode, Url};
use tokio::io::AsyncWriteExt;
use zot_core::{ZotError, ZotResult};

use crate::http::{HttpRuntime, ensure_status, send_with_retry};

pub const MAX_PDF_BYTES: u64 = 100 * 1024 * 1024;
const MAX_REDIRECTS: usize = 5;
const PDF_MAGIC: &[u8] = b"%PDF-";

#[derive(Clone, Copy)]
struct DownloadPolicy {
    allow_loopback_http: bool,
    max_bytes: u64,
    max_redirects: usize,
}

const PRODUCTION_POLICY: DownloadPolicy = DownloadPolicy {
    allow_loopback_http: false,
    max_bytes: MAX_PDF_BYTES,
    max_redirects: MAX_REDIRECTS,
};

/// Download an untrusted provider URL into an existing temporary file.
/// Redirects are followed only after the next destination passes the same
/// scheme and network-boundary checks as the initial URL.
pub async fn download_pdf_to_path(
    runtime: &HttpRuntime,
    source: &str,
    destination: &Path,
) -> ZotResult<()> {
    download_pdf_with_policy(runtime, source, destination, PRODUCTION_POLICY).await
}

async fn download_pdf_with_policy(
    runtime: &HttpRuntime,
    source: &str,
    destination: &Path,
    policy: DownloadPolicy,
) -> ZotResult<()> {
    let mut url = parse_download_url(source, policy.allow_loopback_http)?;

    for redirect_count in 0..=policy.max_redirects {
        validate_destination(&url, policy.allow_loopback_http).await?;
        let response =
            send_with_retry(runtime.download_client().get(url.clone()), "pdf-download").await?;

        if response.status().is_redirection() {
            if redirect_count == policy.max_redirects {
                return Err(download_error(
                    "pdf-download-redirect",
                    format!("PDF download exceeded {} redirects", policy.max_redirects),
                    Some("Use a direct HTTPS PDF URL".to_string()),
                    Some(response.status()),
                ));
            }
            let location = response
                .headers()
                .get(LOCATION)
                .and_then(|value| value.to_str().ok())
                .ok_or_else(|| {
                    download_error(
                        "pdf-download-redirect",
                        "PDF redirect response is missing a valid Location header".to_string(),
                        Some("Use a direct HTTPS PDF URL".to_string()),
                        Some(response.status()),
                    )
                })?;
            url = url.join(location).map_err(|err| ZotError::InvalidInput {
                code: "pdf-download-url".to_string(),
                message: format!("Invalid PDF redirect URL: {err}"),
                hint: Some("Use a direct HTTPS PDF URL".to_string()),
            })?;
            url = parse_download_url(url.as_str(), policy.allow_loopback_http)?;
            continue;
        }

        return write_pdf_response(response, destination, policy.max_bytes).await;
    }

    Err(download_error(
        "pdf-download-redirect",
        "PDF redirect handling did not reach a terminal response".to_string(),
        None,
        None,
    ))
}

fn parse_download_url(source: &str, allow_loopback_http: bool) -> ZotResult<Url> {
    let url = Url::parse(source).map_err(|err| ZotError::InvalidInput {
        code: "pdf-download-url".to_string(),
        message: format!("Invalid PDF download URL: {err}"),
        hint: Some("Use a public HTTPS PDF URL".to_string()),
    })?;
    let allowed_scheme = url.scheme() == "https"
        || (allow_loopback_http
            && url.scheme() == "http"
            && url.host_str().is_some_and(is_loopback_host));
    if !allowed_scheme {
        return Err(ZotError::InvalidInput {
            code: "pdf-download-url".to_string(),
            message: "PDF download URL must use HTTPS".to_string(),
            hint: Some("Use a public HTTPS PDF URL".to_string()),
        });
    }
    if !url.username().is_empty() || url.password().is_some() || url.host_str().is_none() {
        return Err(ZotError::InvalidInput {
            code: "pdf-download-url".to_string(),
            message: "PDF download URL must not contain credentials and must have a host"
                .to_string(),
            hint: Some("Use a public HTTPS PDF URL without userinfo".to_string()),
        });
    }
    Ok(url)
}

async fn validate_destination(url: &Url, allow_loopback: bool) -> ZotResult<()> {
    let host = url.host_str().ok_or_else(|| ZotError::InvalidInput {
        code: "pdf-download-url".to_string(),
        message: "PDF download URL is missing a host".to_string(),
        hint: Some("Use a public HTTPS PDF URL".to_string()),
    })?;
    let port = url
        .port_or_known_default()
        .ok_or_else(|| ZotError::InvalidInput {
            code: "pdf-download-url".to_string(),
            message: "PDF download URL has no usable port".to_string(),
            hint: Some("Use a public HTTPS PDF URL".to_string()),
        })?;

    let addresses = if let Ok(address) = host.parse::<IpAddr>() {
        vec![address]
    } else {
        tokio::net::lookup_host((host, port))
            .await
            .map_err(|err| {
                download_error(
                    "pdf-download-dns",
                    format!("Unable to resolve PDF download host: {err}"),
                    Some("Verify the provider URL and DNS configuration".to_string()),
                    None,
                )
            })?
            .map(|socket| socket.ip())
            .collect::<Vec<_>>()
    };

    if addresses.is_empty()
        || addresses
            .iter()
            .any(|address| !is_public_ip(*address) && !(allow_loopback && address.is_loopback()))
    {
        return Err(ZotError::InvalidInput {
            code: "pdf-download-address".to_string(),
            message: "PDF download host resolves to a non-public address".to_string(),
            hint: Some("Use a public HTTPS PDF URL".to_string()),
        });
    }
    Ok(())
}

async fn write_pdf_response(
    mut response: reqwest::Response,
    destination: &Path,
    max_bytes: u64,
) -> ZotResult<()> {
    if !response.status().is_success() {
        return ensure_status(response, "pdf-download-http")
            .await
            .map(|_| ());
    }
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .map(str::trim);
    if !content_type.is_some_and(|value| value.eq_ignore_ascii_case("application/pdf")) {
        return Err(download_error(
            "pdf-download-content-type",
            "PDF download response must use Content-Type application/pdf".to_string(),
            Some("Use a provider URL that serves the PDF directly".to_string()),
            Some(response.status()),
        ));
    }
    if response
        .headers()
        .get(CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .is_some_and(|length| length > max_bytes)
    {
        return Err(download_size_error(max_bytes));
    }

    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(destination)
        .await
        .map_err(|source| ZotError::Io {
            path: destination.to_path_buf(),
            source,
        })?;
    let mut total = 0_u64;
    let mut leading = Vec::with_capacity(PDF_MAGIC.len());
    while let Some(chunk) = response.chunk().await.map_err(|err| {
        download_error(
            "pdf-download-body",
            format!("Unable to read PDF response body: {err}"),
            Some("Retry with a direct provider PDF URL".to_string()),
            err.status(),
        )
    })? {
        total = total
            .checked_add(chunk.len() as u64)
            .ok_or_else(|| download_size_error(max_bytes))?;
        if total > max_bytes {
            return Err(download_size_error(max_bytes));
        }
        for byte in chunk.iter().copied() {
            if leading.is_empty() && byte.is_ascii_whitespace() {
                continue;
            }
            if leading.len() < PDF_MAGIC.len() {
                leading.push(byte);
            }
        }
        file.write_all(&chunk)
            .await
            .map_err(|source| ZotError::Io {
                path: destination.to_path_buf(),
                source,
            })?;
    }
    if !leading.starts_with(PDF_MAGIC) {
        return Err(download_error(
            "pdf-download-magic",
            "Downloaded content is not a PDF".to_string(),
            Some("Use a provider URL that serves a valid PDF".to_string()),
            Some(response.status()),
        ));
    }
    file.flush().await.map_err(|source| ZotError::Io {
        path: destination.to_path_buf(),
        source,
    })?;
    file.sync_all().await.map_err(|source| ZotError::Io {
        path: destination.to_path_buf(),
        source,
    })?;
    Ok(())
}

fn download_size_error(max_bytes: u64) -> ZotError {
    ZotError::InvalidInput {
        code: "pdf-download-size".to_string(),
        message: format!("PDF download exceeds the {max_bytes}-byte limit"),
        hint: Some("Download and inspect the file manually before attaching it".to_string()),
    }
}

fn download_error(
    code: &str,
    message: String,
    hint: Option<String>,
    status: Option<StatusCode>,
) -> ZotError {
    ZotError::Remote {
        code: code.to_string(),
        message,
        hint,
        status: status.map(|status| status.as_u16()),
    }
}

fn is_loopback_host(host: &str) -> bool {
    host.eq_ignore_ascii_case("localhost")
        || host
            .parse::<IpAddr>()
            .is_ok_and(|address| address.is_loopback())
}

fn is_public_ip(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(address) => is_public_ipv4(address),
        IpAddr::V6(address) => is_public_ipv6(address),
    }
}

fn is_public_ipv4(address: Ipv4Addr) -> bool {
    let [a, b, c, _] = address.octets();
    !(a == 0
        || a == 10
        || a == 127
        || (a == 100 && (64..=127).contains(&b))
        || (a == 169 && b == 254)
        || (a == 172 && (16..=31).contains(&b))
        || (a == 192 && b == 0 && c == 0)
        || (a == 192 && b == 0 && c == 2)
        || (a == 192 && b == 168)
        || (a == 198 && (b == 18 || b == 19))
        || (a == 198 && b == 51 && c == 100)
        || (a == 203 && b == 0 && c == 113)
        || a >= 224)
}

fn is_public_ipv6(address: Ipv6Addr) -> bool {
    if let Some(mapped) = address.to_ipv4_mapped() {
        return is_public_ipv4(mapped);
    }
    let segments = address.segments();
    !(address.is_unspecified()
        || address.is_loopback()
        || address.is_multicast()
        || (segments[0] & 0xfe00) == 0xfc00
        || (segments[0] & 0xffc0) == 0xfe80
        || (segments[0] == 0x2001 && segments[1] == 0x0db8))
}

#[cfg(test)]
mod tests {
    use std::net::IpAddr;
    use std::str::FromStr;

    use tempfile::NamedTempFile;

    use super::{DownloadPolicy, download_pdf_with_policy, is_public_ip, parse_download_url};
    use crate::http::HttpRuntime;
    use crate::test_support::spawn_server_with_headers;

    const TEST_POLICY: DownloadPolicy = DownloadPolicy {
        allow_loopback_http: true,
        max_bytes: 1024,
        max_redirects: 5,
    };

    #[test]
    fn rejects_non_public_ip_ranges() {
        for address in [
            "127.0.0.1",
            "10.0.0.1",
            "100.64.0.1",
            "169.254.1.1",
            "172.16.0.1",
            "192.168.1.1",
            "192.0.2.1",
            "198.18.0.1",
            "198.51.100.1",
            "203.0.113.1",
            "224.0.0.1",
            "::1",
            "fe80::1",
            "fc00::1",
            "2001:db8::1",
            "::ffff:127.0.0.1",
        ] {
            assert!(
                !is_public_ip(IpAddr::from_str(address).expect("valid test IP")),
                "address should be rejected: {address}"
            );
        }
        assert!(is_public_ip(
            IpAddr::from_str("1.1.1.1").expect("valid public IP")
        ));
        assert!(is_public_ip(
            IpAddr::from_str("2606:4700:4700::1111").expect("valid public IP")
        ));
    }

    #[test]
    fn download_urls_require_https_without_userinfo() {
        assert!(parse_download_url("https://example.test/paper.pdf", false).is_ok());
        for invalid in [
            "http://example.test/paper.pdf",
            "ftp://example.test/paper.pdf",
            "https://user:secret@example.test/paper.pdf",
            "not-a-url",
        ] {
            let error = parse_download_url(invalid, false).expect_err("URL must fail closed");
            assert_eq!(error.payload().code, "pdf-download-url");
        }
    }

    #[tokio::test]
    async fn downloads_valid_pdf_through_bounded_temp_path() {
        let (url, server) = spawn_server_with_headers(vec![(
            200,
            "%PDF-1.7\n%%EOF",
            vec![("Content-Type", "application/pdf; charset=binary")],
        )]);
        let temporary = NamedTempFile::new().expect("create download temp");

        let result =
            download_pdf_with_policy(&HttpRuntime::default(), &url, temporary.path(), TEST_POLICY)
                .await;
        let captured = server.join().expect("server thread panicked");

        assert!(result.is_ok(), "valid PDF should download: {result:?}");
        assert_eq!(captured.len(), 1);
        assert!(
            std::fs::read(temporary.path())
                .expect("read downloaded PDF")
                .starts_with(b"%PDF-")
        );
    }

    #[tokio::test]
    async fn rejects_private_redirect_before_second_request() {
        let (url, server) = spawn_server_with_headers(vec![(
            302,
            "",
            vec![("Location", "https://10.0.0.1/private.pdf")],
        )]);
        let temporary = NamedTempFile::new().expect("create download temp");

        let error =
            download_pdf_with_policy(&HttpRuntime::default(), &url, temporary.path(), TEST_POLICY)
                .await
                .expect_err("private redirect must fail closed");
        let captured = server.join().expect("server thread panicked");

        assert_eq!(error.payload().code, "pdf-download-address");
        assert_eq!(captured.len(), 1);
    }

    #[tokio::test]
    async fn enforces_redirect_budget() {
        let responses = vec![
            (302, "", vec![("Location", "/again")]),
            (302, "", vec![("Location", "/again")]),
        ];
        let (url, server) = spawn_server_with_headers(responses);
        let temporary = NamedTempFile::new().expect("create download temp");
        let policy = DownloadPolicy {
            max_redirects: 1,
            ..TEST_POLICY
        };

        let error =
            download_pdf_with_policy(&HttpRuntime::default(), &url, temporary.path(), policy)
                .await
                .expect_err("redirect loop must stop");
        let captured = server.join().expect("server thread panicked");

        assert_eq!(error.payload().code, "pdf-download-redirect");
        assert_eq!(captured.len(), 2);
    }

    #[tokio::test]
    async fn rejects_wrong_content_type_and_magic() {
        for (body, content_type, expected_code) in [
            ("%PDF-1.7", "text/html", "pdf-download-content-type"),
            (
                "<html>not pdf</html>",
                "application/pdf",
                "pdf-download-magic",
            ),
        ] {
            let body: &'static str = Box::leak(body.to_string().into_boxed_str());
            let content_type: &'static str = Box::leak(content_type.to_string().into_boxed_str());
            let (url, server) =
                spawn_server_with_headers(vec![(200, body, vec![("Content-Type", content_type)])]);
            let temporary = NamedTempFile::new().expect("create download temp");

            let error = download_pdf_with_policy(
                &HttpRuntime::default(),
                &url,
                temporary.path(),
                TEST_POLICY,
            )
            .await
            .expect_err("invalid PDF response must fail");
            let _ = server.join().expect("server thread panicked");
            assert_eq!(error.payload().code, expected_code);
        }
    }

    #[tokio::test]
    async fn rejects_declared_and_actual_oversize_downloads() {
        let policy = DownloadPolicy {
            max_bytes: 8,
            ..TEST_POLICY
        };
        let fixtures = [
            (
                "%PDF-1.7",
                vec![("Content-Type", "application/pdf"), ("Content-Length", "9")],
            ),
            (
                "%PDF-1.7-too-large",
                vec![("Content-Type", "application/pdf")],
            ),
        ];
        for (body, headers) in fixtures {
            let (url, server) = spawn_server_with_headers(vec![(200, body, headers)]);
            let temporary = NamedTempFile::new().expect("create download temp");
            let error =
                download_pdf_with_policy(&HttpRuntime::default(), &url, temporary.path(), policy)
                    .await
                    .expect_err("oversize PDF must fail");
            let _ = server.join().expect("server thread panicked");
            assert_eq!(error.payload().code, "pdf-download-size");
        }
    }
}
