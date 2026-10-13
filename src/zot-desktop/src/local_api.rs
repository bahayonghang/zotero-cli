//! Authenticated Zotero 10+ Local API. Never shares Web API credentials or versions.
use std::time::Duration;

use reqwest::{Method, Response, StatusCode, Url};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Map, Value, json};
use zot_core::{LibraryScope, SecretString, ZotError, ZotResult};

use crate::CONNECTOR_BASE_URL;

const SERVER_ID: &str = "Zotero-Server-ID";

#[derive(Debug, Clone, Serialize)]
pub struct LocalApiProbe {
    pub zotero_version: Option<String>,
    pub api_version: Option<String>,
    pub schema_version: Option<String>,
    pub server_id: Option<String>,
    pub supported: bool,
}

impl LocalApiProbe {
    pub fn writable_server_id(&self) -> ZotResult<&str> {
        if !self.supported {
            return Err(error(
                "local-api-unsupported",
                "Local writes require Zotero 10+, API v3 and a Server ID",
                None,
            ));
        }
        self.server_id
            .as_deref()
            .ok_or_else(|| error("local-api-protocol", "Missing Server ID", None))
    }
}

#[derive(Debug, Deserialize)]
pub struct LocalAuthorization {
    pub key: SecretString,
    pub remember: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LocalObject {
    pub key: String,
    pub version: u64,
    pub data: Map<String, Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatchReceipt {
    Accepted,
    /// A request may have committed, but no reliable response was received.
    Unknown,
}

#[derive(Debug, Clone)]
pub struct LocalApiClient {
    client: reqwest::Client,
    base: Url,
    timeout: Duration,
}

impl LocalApiClient {
    pub fn new() -> ZotResult<Self> {
        Self::build(CONNECTOR_BASE_URL, Duration::from_secs(30))
    }

    #[cfg(any(test, feature = "test-support"))]
    #[doc(hidden)]
    pub fn with_base_url_for_tests(base: &str, timeout: Duration) -> ZotResult<Self> {
        Self::build(base, timeout)
    }

    fn build(base: &str, timeout: Duration) -> ZotResult<Self> {
        let base =
            Url::parse(base).map_err(|_| error("local-api-url", "Invalid Local API URL", None))?;
        if base.scheme() != "http"
            || !matches!(base.host_str(), Some("127.0.0.1" | "localhost" | "[::1]"))
            || !base.username().is_empty()
            || base.password().is_some()
            || base.query().is_some()
            || base.fragment().is_some()
            || base.path() != "/"
        {
            return Err(error(
                "local-api-url",
                "Local API requires an HTTP loopback origin",
                None,
            ));
        }
        let client = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(5))
            .user_agent(concat!("zot/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|_| {
                error(
                    "local-api-client",
                    "Cannot initialize Local API client",
                    None,
                )
            })?;
        Ok(Self {
            client,
            base,
            timeout,
        })
    }

    fn request(
        &self,
        method: Method,
        path: &str,
        server: Option<&str>,
    ) -> ZotResult<reqwest::RequestBuilder> {
        let url = self
            .base
            .join(&format!("api/{path}"))
            .map_err(|_| error("local-api-url", "Invalid Local API endpoint", None))?;
        let mut request = self
            .client
            .request(method, url)
            .header("Zotero-API-Version", "3")
            .timeout(self.timeout);
        if let Some(server) = server {
            request = request.header(SERVER_ID, server);
        }
        Ok(request)
    }

    pub async fn probe(&self) -> ZotResult<LocalApiProbe> {
        let response = self
            .request(Method::GET, "", None)?
            .send()
            .await
            .map_err(read_error)?;
        check_status(&response)?;
        let zotero_version = header(&response, "X-Zotero-Version");
        let api_version = header(&response, "Zotero-API-Version");
        let schema_version = header(&response, "Zotero-Schema-Version");
        let server_id = header(&response, SERVER_ID).filter(|v| !v.is_empty());
        let supported = zotero_version
            .as_deref()
            .and_then(|v| v.split('.').next())
            .and_then(|v| v.parse::<u32>().ok())
            .is_some_and(|v| v >= 10)
            && api_version.as_deref() == Some("3")
            && server_id.is_some();
        Ok(LocalApiProbe {
            zotero_version,
            api_version,
            schema_version,
            server_id,
            supported,
        })
    }

    pub async fn authorize(&self, server: &str) -> ZotResult<LocalAuthorization> {
        let response = self
            .request(Method::POST, "local/authorize", Some(server))?
            .timeout(Duration::from_secs(120))
            .json(&json!({"appName": "zot CLI"}))
            .send()
            .await
            .map_err(read_error)?;
        check_response(&response, server)?;
        let authorization: LocalAuthorization = decode(response).await?;
        if authorization.key.is_empty() {
            return Err(error(
                "local-api-protocol",
                "Authorization returned no key",
                None,
            ));
        }
        Ok(authorization)
    }

    async fn get<T: DeserializeOwned>(&self, server: &str, path: &str) -> ZotResult<T> {
        let response = self
            .request(Method::GET, path, Some(server))?
            .send()
            .await
            .map_err(read_error)?;
        check_response(&response, server)?;
        decode(response).await
    }

    pub async fn item(
        &self,
        server: &str,
        scope: &LibraryScope,
        key: &str,
    ) -> ZotResult<LocalObject> {
        self.object(server, scope, "items", key).await
    }

    pub async fn collection(
        &self,
        server: &str,
        scope: &LibraryScope,
        key: &str,
    ) -> ZotResult<LocalObject> {
        self.object(server, scope, "collections", key).await
    }

    async fn object(
        &self,
        server: &str,
        scope: &LibraryScope,
        kind: &str,
        key: &str,
    ) -> ZotResult<LocalObject> {
        validate_key(key)?;
        let object: LocalObject = self
            .get(server, &format!("{}/{kind}/{key}", prefix(scope)))
            .await?;
        if object.key != key
            || object.data.get("key").and_then(Value::as_str) != Some(key)
            || object.data.get("version").and_then(Value::as_u64) != Some(object.version)
        {
            return Err(error(
                "local-api-protocol",
                "Object identity or version is inconsistent",
                None,
            ));
        }
        Ok(object)
    }

    pub async fn fields(&self, server: &str, item_type: &str) -> ZotResult<Vec<String>> {
        self.schema_names(server, "itemTypeFields", item_type, "field")
            .await
    }

    pub async fn creator_types(&self, server: &str, item_type: &str) -> ZotResult<Vec<String>> {
        self.schema_names(server, "itemTypeCreatorTypes", item_type, "creatorType")
            .await
    }

    async fn schema_names(
        &self,
        server: &str,
        endpoint: &str,
        item_type: &str,
        field: &str,
    ) -> ZotResult<Vec<String>> {
        let mut query = self.base.clone();
        query.query_pairs_mut().append_pair("itemType", item_type);
        let path = format!("{endpoint}?{}", query.query().unwrap_or_default());
        let response = self
            .request(Method::GET, &path, Some(server))?
            .send()
            .await
            .map_err(read_error)?;
        check_response(&response, server)?;
        let entries: Vec<Value> = decode(response).await?;
        entries
            .into_iter()
            .map(|entry| {
                entry
                    .get(field)
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned)
                    .ok_or_else(|| error("local-api-protocol", "Invalid schema response", None))
            })
            .collect()
    }

    pub async fn patch_item(
        &self,
        server: &str,
        scope: &LibraryScope,
        key: &str,
        version: u64,
        patch: &Map<String, Value>,
        authorization: &SecretString,
    ) -> ZotResult<PatchReceipt> {
        validate_key(key)?;
        let mut body = patch.clone();
        body.insert("version".to_string(), version.into());
        let response = self
            .request(
                Method::PATCH,
                &format!("{}/items/{key}", prefix(scope)),
                Some(server),
            )?
            .header("Zotero-API-Key", authorization.expose_secret())
            .header("If-Unmodified-Since-Version", version.to_string())
            .json(&body)
            .send()
            .await;
        let response = match response {
            Ok(response) => response,
            Err(_) => return Ok(PatchReceipt::Unknown),
        };
        // Server failures can occur after saving; never retry them as a rejected write.
        if response.status().is_server_error() {
            return Ok(PatchReceipt::Unknown);
        }
        check_status(&response)?;
        if header(&response, SERVER_ID).as_deref() != Some(server)
            || response.status() != StatusCode::NO_CONTENT
        {
            return Ok(PatchReceipt::Unknown);
        }
        Ok(PatchReceipt::Accepted)
    }
}

fn prefix(scope: &LibraryScope) -> String {
    match scope {
        LibraryScope::User => "users/0".to_string(),
        LibraryScope::Group { group_id } => format!("groups/{group_id}"),
    }
}

fn validate_key(key: &str) -> ZotResult<()> {
    if key.len() != 8
        || !key
            .bytes()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
    {
        return Err(error(
            "local-api-key",
            "Expected an eight-character Zotero object key",
            None,
        ));
    }
    Ok(())
}

fn header(response: &Response, name: &str) -> Option<String> {
    response
        .headers()
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map(ToOwned::to_owned)
}

fn check_response(response: &Response, server: &str) -> ZotResult<()> {
    check_status(response)?;
    if header(response, SERVER_ID).as_deref() != Some(server) {
        return Err(error(
            "local-api-conflict",
            "Zotero instance changed; preview again",
            None,
        ));
    }
    Ok(())
}

fn check_status(response: &Response) -> ZotResult<()> {
    if response.status().is_success() {
        return Ok(());
    }
    let (code, message) = match response.status().as_u16() {
        401 => (
            "local-api-unauthorized",
            "Local authorization is missing or expired",
        ),
        403 => (
            "local-api-forbidden",
            "Local API disabled, authorization denied, or target not writable",
        ),
        404 => (
            "local-api-not-found",
            "Target not found in the selected local library",
        ),
        412 => (
            "local-api-conflict",
            "Zotero instance or object version changed; preview again",
        ),
        428 => (
            "local-api-precondition",
            "Local API requires instance and version preconditions",
        ),
        429 => (
            "local-api-rate-limit",
            "Authorization rate limited; retry after the indicated delay",
        ),
        _ => ("local-api-http", "Local API rejected the request"),
    };
    let mut result = error(code, message, Some(response.status().as_u16()));
    if let ZotError::LocalApi { hint, .. } = &mut result {
        if let Some(delay) = header(response, "Retry-After") {
            *hint = Some(format!("Retry-After: {delay}"));
        }
    }
    Err(result)
}

async fn decode<T: DeserializeOwned>(response: Response) -> ZotResult<T> {
    response.json().await.map_err(|_| {
        error(
            "local-api-protocol",
            "Invalid Local API JSON response",
            None,
        )
    })
}

fn read_error(err: reqwest::Error) -> ZotError {
    let code = if err.is_timeout() {
        "local-api-timeout"
    } else {
        "local-api-unreachable"
    };
    error(
        code,
        "Cannot read Zotero Local API; check that Zotero is running and local communication is enabled",
        None,
    )
}

fn error(code: &str, message: &str, status: Option<u16>) -> ZotError {
    ZotError::LocalApi {
        code: code.to_string(),
        message: message.to_string(),
        hint: None,
        status,
    }
}

#[cfg(test)]
#[path = "local_api_tests.rs"]
mod tests;
