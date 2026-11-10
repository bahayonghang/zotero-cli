use once_cell::sync::Lazy;
use regex::Regex;
use serde::Deserialize;
use zot_core::{ZotError, ZotResult};

use crate::http::{HttpRuntime, ensure_status, remote_err, send_with_retry};

const API_BASE: &str = "https://api.semanticscholar.org/graph/v1";
static ARXIV_VERSION_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"v\d+$").expect("valid regex"));
static BIORXIV_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(10\.1101/\d{4}\.\d{2}\.\d{2}\.\d+)(?:v\d+)?").expect("valid regex"));
static ARXIV_PATTERNS: Lazy<[Regex; 4]> = Lazy::new(|| {
    [
        Regex::new(r"arxiv\.org/(?:abs|pdf)/(\d{4}\.\d{4,5}(?:v\d+)?)").expect("valid arxiv regex"),
        Regex::new(r"arxiv\.org/(?:abs|pdf)/([a-z\-]+/\d{7}(?:v\d+)?)").expect("valid arxiv regex"),
        Regex::new(r"10\.48550/arXiv\.(\d{4}\.\d{4,5}(?:v\d+)?)").expect("valid arxiv regex"),
        Regex::new(r"arXiv:(\d{4}\.\d{4,5}(?:v\d+)?)").expect("valid arxiv regex"),
    ]
});

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreprintInfo {
    pub preprint_id: String,
    pub source: String,
    pub api_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicationStatus {
    pub preprint_id: String,
    pub source: String,
    pub title: String,
    pub is_published: bool,
    pub venue: Option<String>,
    pub journal_name: Option<String>,
    pub doi: Option<String>,
    pub publication_date: Option<String>,
}

pub fn extract_preprint_info(
    url: Option<&str>,
    doi: Option<&str>,
    extra: Option<&str>,
) -> Option<PreprintInfo> {
    for source in [
        url.unwrap_or_default(),
        doi.unwrap_or_default(),
        extra.unwrap_or_default(),
    ] {
        for re in ARXIV_PATTERNS.iter() {
            if let Some(captures) = re.captures(source)
                && let Some(matched) = captures.get(1)
            {
                let id = matched.as_str().to_string();
                let normalized = ARXIV_VERSION_RE.replace(&id, "").to_string();
                return Some(PreprintInfo {
                    preprint_id: normalized.clone(),
                    source: "arxiv".to_string(),
                    api_id: format!("arXiv:{normalized}"),
                });
            }
        }
    }

    for source in [doi.unwrap_or_default(), url.unwrap_or_default()] {
        if let Some(captures) = BIORXIV_RE.captures(source)
            && let Some(matched) = captures.get(1)
        {
            let preprint_doi = matched.as_str().to_string();
            return Some(PreprintInfo {
                preprint_id: preprint_doi.clone(),
                source: "doi".to_string(),
                api_id: format!("DOI:{preprint_doi}"),
            });
        }
    }

    None
}

#[derive(Clone)]
pub struct SemanticScholarClient {
    client: reqwest::Client,
    api_key: Option<String>,
    base_url: String,
}

impl SemanticScholarClient {
    pub fn new(runtime: &HttpRuntime, api_key: Option<&str>) -> ZotResult<Self> {
        let api_key = api_key
            .filter(|value| !value.is_empty())
            .map(|value| value.to_string());
        if let Some(value) = api_key.as_deref() {
            reqwest::header::HeaderValue::from_str(value).map_err(|err| {
                ZotError::InvalidInput {
                    code: "ss-api-key".to_string(),
                    message: err.to_string(),
                    hint: None,
                }
            })?;
        }
        Ok(Self {
            client: runtime.client_clone(),
            api_key,
            base_url: std::env::var("ZOT_SEMANTIC_SCHOLAR_API_BASE")
                .unwrap_or_else(|_| API_BASE.to_string()),
        })
    }

    /// Construct a client pointed at an explicit base URL (fake server in
    /// tests), bypassing the `ZOT_SEMANTIC_SCHOLAR_API_BASE` env override so
    /// parallel tests never race on process-global environment state.
    #[cfg(test)]
    fn with_base_url(
        runtime: &HttpRuntime,
        api_key: Option<&str>,
        base_url: impl Into<String>,
    ) -> ZotResult<Self> {
        let mut client = Self::new(runtime, api_key)?;
        client.base_url = base_url.into();
        Ok(client)
    }

    pub async fn check_publication(
        &self,
        info: &PreprintInfo,
    ) -> ZotResult<Option<PublicationStatus>> {
        let url = format!(
            "{}/paper/{}?fields=externalIds,journal,venue,publicationDate,title,publicationVenue",
            self.base_url, info.api_id
        );
        let mut request = self.client.get(url);
        if let Some(api_key) = self.api_key.as_deref() {
            request = request.header("x-api-key", api_key);
        }
        let response = send_with_retry(request, "ss-request").await?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        let response = ensure_status(response, "ss-http").await?;

        let payload: SemanticScholarPaper = response.json().await.map_err(remote_err("ss-json"))?;
        let journal_name = payload
            .journal
            .and_then(|journal| journal.name)
            .or_else(|| payload.publication_venue.and_then(|venue| venue.name));
        let venue = payload.venue;
        let formal_doi = payload.external_ids.and_then(|ids| ids.doi);
        let preprint_venue = venue
            .as_deref()
            .map(|value| value.to_lowercase())
            .unwrap_or_default();
        let journal_venue = journal_name
            .as_deref()
            .map(|value| value.to_lowercase())
            .unwrap_or_default();
        let is_preprint_doi = formal_doi
            .as_deref()
            .map(|doi| doi.starts_with("10.48550/") || doi.starts_with("10.1101/"))
            .unwrap_or(false);
        let venue_is_preprint = ["arxiv", "biorxiv", "medrxiv", "ssrn"]
            .iter()
            .any(|token| preprint_venue.contains(token) || journal_venue.contains(token));
        let is_published = (venue.is_some() || journal_name.is_some())
            && !venue_is_preprint
            && formal_doi.is_some()
            && !is_preprint_doi;

        Ok(Some(PublicationStatus {
            preprint_id: info.preprint_id.clone(),
            source: info.source.clone(),
            title: payload.title.unwrap_or_default(),
            is_published,
            venue,
            journal_name,
            doi: if is_preprint_doi { None } else { formal_doi },
            publication_date: payload.publication_date,
        }))
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SemanticScholarPaper {
    external_ids: Option<ExternalIds>,
    journal: Option<Journal>,
    venue: Option<String>,
    publication_date: Option<String>,
    title: Option<String>,
    publication_venue: Option<PublicationVenue>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExternalIds {
    #[serde(rename = "DOI")]
    doi: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Journal {
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PublicationVenue {
    name: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::{PreprintInfo, SemanticScholarClient, extract_preprint_info};
    use crate::http::HttpRuntime;
    use crate::test_support::spawn_server;

    #[test]
    fn extracts_arxiv_and_biorxiv() {
        let arxiv = extract_preprint_info(Some("https://arxiv.org/abs/1706.03762v1"), None, None);
        assert!(arxiv.is_some());
        let biorxiv = extract_preprint_info(None, Some("10.1101/2024.01.02.123456v2"), None);
        assert!(biorxiv.is_some());
    }

    #[tokio::test]
    async fn check_publication_reports_published_paper_via_injected_base_url() {
        // A non-preprint venue plus a formal (non-arXiv/bioRxiv) DOI resolves to
        // "published"; the client is aimed at the fake server via with_base_url.
        let body = r#"{"title":"Attention Is All You Need","venue":"NeurIPS","externalIds":{"DOI":"10.5555/formal"},"publicationDate":"2017-12-01","journal":{"name":"NeurIPS"}}"#;
        let (base_url, server) = spawn_server(vec![(200, body)]);
        let client = SemanticScholarClient::with_base_url(&HttpRuntime::default(), None, base_url)
            .expect("construct ss client");

        let info = PreprintInfo {
            preprint_id: "1706.03762".to_string(),
            source: "arxiv".to_string(),
            api_id: "arXiv:1706.03762".to_string(),
        };
        let result = client.check_publication(&info).await;

        let captured = server.join().expect("server thread panicked");
        assert_eq!(captured.len(), 1);
        assert_eq!(captured[0].method, "GET");
        match result {
            Ok(Some(status)) => {
                assert!(status.is_published);
                assert_eq!(status.title, "Attention Is All You Need");
                assert_eq!(status.doi.as_deref(), Some("10.5555/formal"));
            }
            other => panic!("expected Ok(Some(published)), got {other:?}"),
        }
    }
}
