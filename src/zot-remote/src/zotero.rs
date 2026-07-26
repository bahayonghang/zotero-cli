use std::collections::BTreeMap;
use std::io::SeekFrom;
use std::path::Path;

use reqwest::header::{CONTENT_TYPE, HeaderValue};
use reqwest::{Method, StatusCode, Url};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use uuid::Uuid;
use zot_core::{LibraryScope, SavedSearch, SavedSearchCondition, ZotError, ZotResult};

use crate::http::{
    HttpRuntime, ensure_empty, ensure_status, read_json, remote_err, send_with_retry,
};

const API_BASE: &str = "https://api.zotero.org";
const ZOTERO_API_KEY_HEADER: &str = "zotero-api-key";
const ZOTERO_API_VERSION_HEADER: &str = "Zotero-API-Version";
const ZOTERO_API_VERSION: &str = "3";
const MAX_ATTACHMENT_BYTES: u64 = 100 * 1024 * 1024;
const MAX_UPLOAD_OVERHEAD_BYTES: usize = 1024 * 1024;

#[derive(Clone)]
pub struct ZoteroRemote {
    client: reqwest::Client,
    library_id: String,
    api_key: String,
    api_version: HeaderValue,
    scope: LibraryScope,
    base_url: String,
    #[cfg(any(test, feature = "test-support"))]
    allow_insecure_loopback_uploads: bool,
}

impl ZoteroRemote {
    pub fn new(
        runtime: &HttpRuntime,
        library_id: impl Into<String>,
        api_key: &str,
        scope: LibraryScope,
    ) -> ZotResult<Self> {
        // Validate the API key parses as a header value up front; actual
        // attachment happens per-request via `http_*` helpers so we can share
        // the underlying connection pool across remote clients.
        HeaderValue::from_str(api_key).map_err(|err| ZotError::InvalidInput {
            code: "api-key".to_string(),
            message: err.to_string(),
            hint: None,
        })?;
        Ok(Self {
            client: runtime.client_clone(),
            library_id: library_id.into(),
            api_key: api_key.to_string(),
            api_version: HeaderValue::from_static(ZOTERO_API_VERSION),
            scope,
            base_url: std::env::var("ZOT_ZOTERO_API_BASE").unwrap_or_else(|_| API_BASE.to_string()),
            #[cfg(any(test, feature = "test-support"))]
            allow_insecure_loopback_uploads: false,
        })
    }

    /// Construct a client pointed at an explicit base URL (fake server in
    /// tests), bypassing the `ZOT_ZOTERO_API_BASE` env override so parallel
    /// tests never race on process-global environment state.
    #[cfg(any(test, feature = "test-support"))]
    #[doc(hidden)]
    pub fn with_base_url_for_tests(
        runtime: &HttpRuntime,
        library_id: impl Into<String>,
        api_key: &str,
        scope: LibraryScope,
        base_url: impl Into<String>,
    ) -> ZotResult<Self> {
        let mut remote = Self::new(runtime, library_id, api_key, scope)?;
        remote.base_url = base_url.into();
        remote.allow_insecure_loopback_uploads = true;
        Ok(remote)
    }

    fn zotero_request(&self, method: Method, endpoint: &str) -> reqwest::RequestBuilder {
        self.client
            .request(method, self.endpoint(endpoint))
            .header(ZOTERO_API_KEY_HEADER, &self.api_key)
            .header(ZOTERO_API_VERSION_HEADER, self.api_version.clone())
    }

    fn zotero_get(&self, endpoint: &str) -> reqwest::RequestBuilder {
        self.zotero_request(Method::GET, endpoint)
    }

    fn zotero_post(&self, endpoint: &str) -> reqwest::RequestBuilder {
        self.zotero_request(Method::POST, endpoint)
    }

    fn zotero_put(&self, endpoint: &str) -> reqwest::RequestBuilder {
        self.zotero_request(Method::PUT, endpoint)
    }

    fn zotero_patch(&self, endpoint: &str) -> reqwest::RequestBuilder {
        self.zotero_request(Method::PATCH, endpoint)
    }

    fn zotero_delete(&self, endpoint: &str) -> reqwest::RequestBuilder {
        self.zotero_request(Method::DELETE, endpoint)
    }

    fn external_upload_request(&self, upload_url: &str) -> ZotResult<reqwest::RequestBuilder> {
        let url = Url::parse(upload_url).map_err(|err| ZotError::InvalidInput {
            code: "attachment-upload-url".to_string(),
            message: format!("Invalid attachment upload URL: {err}"),
            hint: Some("Retry attachment authorization to obtain a valid HTTPS URL".to_string()),
        })?;
        let secure = url.scheme() == "https";
        #[cfg(any(test, feature = "test-support"))]
        let secure = secure
            || (self.allow_insecure_loopback_uploads
                && url.scheme() == "http"
                && url.host_str().is_some_and(|host| {
                    host.eq_ignore_ascii_case("localhost")
                        || host
                            .parse::<std::net::IpAddr>()
                            .is_ok_and(|address| address.is_loopback())
                }));
        if !secure {
            return Err(ZotError::InvalidInput {
                code: "attachment-upload-url".to_string(),
                message: "Attachment upload URL must use HTTPS".to_string(),
                hint: Some(
                    "Retry attachment authorization; do not upload to an insecure URL".to_string(),
                ),
            });
        }
        Ok(self.client.post(url))
    }

    pub async fn create_item(&self, doi: Option<&str>, url: Option<&str>) -> ZotResult<String> {
        let payload = if let Some(doi) = doi {
            json!([{ "itemType": "journalArticle", "DOI": doi }])
        } else if let Some(url) = url {
            json!([{ "itemType": "webpage", "url": url }])
        } else {
            return Err(ZotError::InvalidInput {
                code: "item-create".to_string(),
                message: "Either DOI or URL is required".to_string(),
                hint: None,
            });
        };
        self.create_items(&payload, "create-item")
            .await
            .and_then(first_created_key)
    }

    pub async fn create_item_from_value(&self, value: Value) -> ZotResult<String> {
        let payload = Value::Array(vec![value]);
        self.create_items(&payload, "create-item-raw")
            .await
            .and_then(first_created_key)
    }

    pub async fn update_item_fields(
        &self,
        key: &str,
        fields: &BTreeMap<String, String>,
    ) -> ZotResult<()> {
        let mut item = self.get_item_data(key).await?;
        for (field, value) in fields {
            item.data[field] = Value::String(value.clone());
        }
        let version = item.version();
        let response = self
            .zotero_put(&format!("items/{key}"))
            .header("If-Unmodified-Since-Version", version.to_string())
            .json(&item.data)
            .send()
            .await
            .map_err(remote_err("update-item"))?;
        ensure_empty(response, "update-item").await
    }

    pub async fn delete_item(&self, key: &str) -> ZotResult<()> {
        let item = self.get_item_data(key).await?;
        let payload = json!({ "deleted": 1 });
        let response = self
            .zotero_patch(&format!("items/{key}"))
            .header("If-Unmodified-Since-Version", item.version().to_string())
            .json(&payload)
            .send()
            .await
            .map_err(remote_err("delete-item"))?;
        ensure_empty(response, "delete-item").await
    }

    pub async fn restore_item(&self, key: &str) -> ZotResult<()> {
        let mut item = self.get_item_data(key).await?;
        item.data["deleted"] = Value::Number(0.into());
        let response = self
            .zotero_patch(&format!("items/{key}"))
            .header("If-Unmodified-Since-Version", item.version().to_string())
            .json(&item.data)
            .send()
            .await
            .map_err(remote_err("restore-item"))?;
        ensure_empty(response, "restore-item").await
    }

    pub async fn add_note(&self, parent_key: &str, content: &str) -> ZotResult<String> {
        let payload = json!([{
            "itemType": "note",
            "parentItem": parent_key,
            "note": content,
        }]);
        self.create_items(&payload, "add-note")
            .await
            .and_then(first_created_key)
    }

    pub async fn update_note(&self, note_key: &str, content: &str) -> ZotResult<()> {
        let mut item = self.get_item_data(note_key).await?;
        item.data["note"] = Value::String(content.to_string());
        let response = self
            .zotero_put(&format!("items/{note_key}"))
            .header("If-Unmodified-Since-Version", item.version().to_string())
            .json(&item.data)
            .send()
            .await
            .map_err(remote_err("update-note"))?;
        ensure_empty(response, "update-note").await
    }

    pub async fn add_tags(&self, key: &str, tags: &[String]) -> ZotResult<()> {
        let mut item = self.get_item_data(key).await?;
        let existing = item
            .data
            .get("tags")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut merged: Vec<String> = Vec::new();
        for entry in existing {
            if let Some(tag) = entry.get("tag").and_then(Value::as_str)
                && seen.insert(tag.to_string())
            {
                merged.push(tag.to_string());
            }
        }
        for tag in tags {
            if seen.insert(tag.clone()) {
                merged.push(tag.clone());
            }
        }
        item.data["tags"] = Value::Array(
            merged
                .into_iter()
                .map(|tag| json!({ "tag": tag }))
                .collect(),
        );
        let response = self
            .zotero_put(&format!("items/{key}"))
            .header("If-Unmodified-Since-Version", item.version().to_string())
            .json(&item.data)
            .send()
            .await
            .map_err(remote_err("add-tags"))?;
        ensure_empty(response, "add-tags").await
    }

    pub async fn remove_tags(&self, key: &str, tags: &[String]) -> ZotResult<()> {
        let mut item = self.get_item_data(key).await?;
        let drop: std::collections::HashSet<&str> = tags.iter().map(String::as_str).collect();
        let filtered = item
            .data
            .get("tags")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter(|entry| {
                entry
                    .get("tag")
                    .and_then(Value::as_str)
                    .map(|tag| !drop.contains(tag))
                    .unwrap_or(true)
            })
            .collect::<Vec<_>>();
        item.data["tags"] = Value::Array(filtered);
        let response = self
            .zotero_put(&format!("items/{key}"))
            .header("If-Unmodified-Since-Version", item.version().to_string())
            .json(&item.data)
            .send()
            .await
            .map_err(remote_err("remove-tags"))?;
        ensure_empty(response, "remove-tags").await
    }

    pub async fn create_collection(
        &self,
        name: &str,
        parent_key: Option<&str>,
    ) -> ZotResult<String> {
        let payload = json!([{
            "name": name,
            "parentCollection": parent_key.unwrap_or(""),
        }]);
        let response = send_with_retry(
            self.zotero_post("collections")
                .header("Zotero-Write-Token", Uuid::new_v4().to_string())
                .json(&payload),
            "create-collection",
        )
        .await?;
        let body: MultiWriteResponse = read_json(response, "create-collection").await?;
        body.successful
            .and_then(|successful| successful.get("0").and_then(|entry| entry.key.clone()))
            .ok_or_else(|| ZotError::Remote {
                code: "create-collection".to_string(),
                message: "Unexpected create collection response".to_string(),
                hint: None,
                status: None,
            })
    }

    pub async fn rename_collection(&self, key: &str, new_name: &str) -> ZotResult<()> {
        let mut collection = self.get_collection_data(key).await?;
        collection.data["name"] = Value::String(new_name.to_string());
        let response = self
            .zotero_put(&format!("collections/{key}"))
            .header(
                "If-Unmodified-Since-Version",
                collection.version().to_string(),
            )
            .json(&collection.data)
            .send()
            .await
            .map_err(remote_err("rename-collection"))?;
        ensure_empty(response, "rename-collection").await
    }

    pub async fn delete_collection(&self, key: &str) -> ZotResult<()> {
        let collection = self.get_collection_data(key).await?;
        let response = self
            .zotero_delete(&format!("collections/{key}"))
            .header(
                "If-Unmodified-Since-Version",
                collection.version().to_string(),
            )
            .send()
            .await
            .map_err(remote_err("delete-collection"))?;
        ensure_empty(response, "delete-collection").await
    }

    pub async fn add_item_to_collection(
        &self,
        item_key: &str,
        collection_key: &str,
    ) -> ZotResult<()> {
        let mut item = self.get_item_data(item_key).await?;
        let current = item
            .data
            .get("collections")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|entry| entry.as_str().map(ToOwned::to_owned))
            .collect::<Vec<_>>();
        if !current.iter().any(|existing| existing == collection_key) {
            let mut next = current;
            next.push(collection_key.to_string());
            item.data["collections"] = Value::Array(next.into_iter().map(Value::String).collect());
        }
        let response = self
            .zotero_patch(&format!("items/{item_key}"))
            .header("If-Unmodified-Since-Version", item.version().to_string())
            .json(&item.data)
            .send()
            .await
            .map_err(remote_err("add-item-to-collection"))?;
        ensure_empty(response, "add-item-to-collection").await
    }

    pub async fn remove_item_from_collection(
        &self,
        item_key: &str,
        collection_key: &str,
    ) -> ZotResult<()> {
        let mut item = self.get_item_data(item_key).await?;
        let next = item
            .data
            .get("collections")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter(|entry| {
                entry
                    .as_str()
                    .map(|value| value != collection_key)
                    .unwrap_or(true)
            })
            .collect::<Vec<_>>();
        item.data["collections"] = Value::Array(next);
        let response = self
            .zotero_patch(&format!("items/{item_key}"))
            .header("If-Unmodified-Since-Version", item.version().to_string())
            .json(&item.data)
            .send()
            .await
            .map_err(remote_err("remove-item-from-collection"))?;
        ensure_empty(response, "remove-item-from-collection").await
    }

    pub async fn upload_attachment(&self, parent_key: &str, file_path: &Path) -> ZotResult<String> {
        let mut source = PreparedAttachment::open(file_path).await?;
        let attachment_key = self
            .create_attachment_item(parent_key, &source.filename)
            .await?;
        match self
            .complete_attachment_upload(&attachment_key, &mut source)
            .await
        {
            Ok(()) => Ok(attachment_key),
            Err(error) => Err(self.with_attachment_cleanup(&attachment_key, error).await),
        }
    }

    async fn complete_attachment_upload(
        &self,
        attachment_key: &str,
        source: &mut PreparedAttachment,
    ) -> ZotResult<()> {
        let auth = self
            .authorize_attachment_upload(attachment_key, source)
            .await?;
        if auth.exists.unwrap_or(false) {
            return Ok(());
        }
        let upload_url = auth.url.clone().ok_or_else(|| ZotError::Remote {
            code: "attachment-upload".to_string(),
            message: "Upload authorization missing URL".to_string(),
            hint: None,
            status: None,
        })?;
        let upload_key = auth.upload_key.clone().ok_or_else(|| ZotError::Remote {
            code: "attachment-upload".to_string(),
            message: "Upload authorization missing uploadKey".to_string(),
            hint: None,
            status: None,
        })?;
        let content_type = auth
            .content_type
            .clone()
            .unwrap_or_else(|| "multipart/form-data".to_string());
        let prefix = auth.prefix.unwrap_or_default();
        let suffix = auth.suffix.unwrap_or_default();
        let payload = source.upload_payload(prefix, suffix).await?;
        let upload_response = self
            .external_upload_request(&upload_url)?
            .header(CONTENT_TYPE, content_type)
            .body(payload)
            .send()
            .await
            .map_err(remote_err("attachment-upload"))?;
        if upload_response.status() != StatusCode::CREATED {
            return Err(ZotError::Remote {
                code: "attachment-upload".to_string(),
                message: format!(
                    "Attachment upload failed with status {}",
                    upload_response.status()
                ),
                hint: None,
                status: Some(upload_response.status().as_u16()),
            });
        }

        let register_response = self
            .zotero_post(&format!("items/{attachment_key}/file"))
            .header("If-None-Match", "*")
            .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(format!("upload={upload_key}"))
            .send()
            .await
            .map_err(remote_err("attachment-register"))?;
        ensure_empty(register_response, "attachment-register").await?;
        Ok(())
    }

    pub async fn add_linked_attachment(
        &self,
        parent_key: &str,
        url: &str,
        title: &str,
    ) -> ZotResult<String> {
        self.create_item_from_value(json!({
            "itemType": "attachment",
            "parentItem": parent_key,
            "linkMode": "linked_url",
            "title": title,
            "url": url,
            "contentType": "application/pdf",
        }))
        .await
    }

    pub async fn list_saved_searches(&self) -> ZotResult<Vec<SavedSearch>> {
        let response = send_with_retry(self.zotero_get("searches"), "list-saved-searches").await?;
        let body: Vec<RawSavedSearch> = read_json(response, "list-saved-searches").await?;
        Ok(body.into_iter().map(Into::into).collect())
    }

    pub async fn create_saved_search(
        &self,
        name: &str,
        conditions: &[SavedSearchCondition],
    ) -> ZotResult<String> {
        let payload = json!([{
            "name": name,
            "conditions": conditions,
        }]);
        self.create_searches(&payload, "create-saved-search")
            .await
            .and_then(first_created_key)
    }

    pub async fn delete_saved_searches(&self, keys: &[String]) -> ZotResult<()> {
        if keys.is_empty() {
            return Ok(());
        }
        let response = self
            .zotero_delete(&format!("searches?searchKey={}", keys.join(",")))
            .header(
                "If-Unmodified-Since-Version",
                self.library_version().await?.to_string(),
            )
            .send()
            .await
            .map_err(remote_err("delete-saved-searches"))?;
        ensure_empty(response, "delete-saved-searches").await
    }

    pub async fn list_item_versions(&self, since: Option<i64>) -> ZotResult<BTreeMap<String, i64>> {
        let endpoint = if let Some(since) = since {
            format!("items?format=versions&since={since}")
        } else {
            "items?format=versions".to_string()
        };
        let response = send_with_retry(self.zotero_get(&endpoint), "list-item-versions").await?;
        read_json(response, "list-item-versions").await
    }

    pub async fn delete_note(&self, note_key: &str) -> ZotResult<()> {
        self.delete_item(note_key).await
    }

    pub async fn get_item_json(&self, key: &str) -> ZotResult<Value> {
        self.get_item_flat(key).await
    }

    pub async fn get_item_flat(&self, key: &str) -> ZotResult<Value> {
        let item = self.get_item_data(key).await?;
        Ok(item.into_flat_value())
    }

    pub async fn list_children(&self, key: &str) -> ZotResult<Vec<Value>> {
        let response = send_with_retry(
            self.zotero_get(&format!("items/{key}/children")),
            "list-children",
        )
        .await?;
        read_json(response, "list-children").await
    }

    pub async fn list_children_flat(&self, key: &str) -> ZotResult<Vec<Value>> {
        let response = send_with_retry(
            self.zotero_get(&format!("items/{key}/children")),
            "list-children",
        )
        .await?;
        let children: Vec<EditableObject> = read_json(response, "list-children").await?;
        Ok(children
            .into_iter()
            .map(EditableObject::into_flat_value)
            .collect())
    }

    pub async fn update_item_value(&self, item: &Value) -> ZotResult<()> {
        self.update_flat_item_value(item).await
    }

    pub async fn update_flat_item_value(&self, item: &Value) -> ZotResult<()> {
        let key =
            item.get("key")
                .and_then(Value::as_str)
                .ok_or_else(|| ZotError::InvalidInput {
                    code: "update-item-value".to_string(),
                    message: "Missing item key in payload".to_string(),
                    hint: None,
                })?;
        let version =
            item.get("version")
                .and_then(Value::as_i64)
                .ok_or_else(|| ZotError::InvalidInput {
                    code: "update-item-value".to_string(),
                    message: "Missing item version in payload".to_string(),
                    hint: None,
                })?;
        let response = self
            .zotero_put(&format!("items/{key}"))
            .header("If-Unmodified-Since-Version", version.to_string())
            .json(&sanitize_flat_item_value(item))
            .send()
            .await
            .map_err(remote_err("update-item-value"))?;
        ensure_empty(response, "update-item-value").await
    }

    pub async fn set_deleted(&self, key: &str, deleted: bool) -> ZotResult<()> {
        let item = self.get_item_data(key).await?;
        let response = self
            .zotero_patch(&format!("items/{key}"))
            .header("If-Unmodified-Since-Version", item.version().to_string())
            .json(&json!({ "deleted": if deleted { 1 } else { 0 } }))
            .send()
            .await
            .map_err(remote_err("set-deleted"))?;
        ensure_empty(response, "set-deleted").await
    }

    async fn create_items(&self, payload: &Value, code: &str) -> ZotResult<Vec<String>> {
        let response = send_with_retry(
            self.zotero_post("items")
                .header("Zotero-Write-Token", Uuid::new_v4().to_string())
                .json(payload),
            "create-items",
        )
        .await?;
        let body: MultiWriteResponse = read_json(response, code).await?;
        Ok(body
            .successful
            .unwrap_or_default()
            .into_values()
            .filter_map(|entry| entry.key)
            .collect())
    }

    async fn create_searches(&self, payload: &Value, code: &str) -> ZotResult<Vec<String>> {
        let response = send_with_retry(
            self.zotero_post("searches")
                .header("Zotero-Write-Token", Uuid::new_v4().to_string())
                .json(payload),
            "create-searches",
        )
        .await?;
        let body: MultiWriteResponse = read_json(response, code).await?;
        Ok(body
            .successful
            .unwrap_or_default()
            .into_values()
            .filter_map(|entry| entry.key)
            .collect())
    }

    fn endpoint(&self, path: &str) -> String {
        let scope = match self.scope {
            LibraryScope::User => format!("users/{}", self.library_id),
            LibraryScope::Group { .. } => format!("groups/{}", self.library_id),
        };
        format!("{}/{scope}/{path}", self.base_url)
    }

    /// Canonical public URI for an item in this library
    /// (`http://zotero.org/users|groups/{library_id}/items/{KEY}`), the form
    /// Zotero stores in item `relations` values such as `dc:replaces`. This
    /// is the fixed zotero.org namespace, not the API `base_url`.
    pub fn item_uri(&self, key: &str) -> String {
        let scope = match self.scope {
            LibraryScope::User => "users",
            LibraryScope::Group { .. } => "groups",
        };
        format!("http://zotero.org/{scope}/{}/items/{key}", self.library_id)
    }

    async fn library_version(&self) -> ZotResult<i64> {
        let response = send_with_retry(
            self.zotero_get("items?limit=1&format=keys"),
            "library-version",
        )
        .await?;
        let response = ensure_status(response, "library-version").await?;
        let version = response
            .headers()
            .get("Last-Modified-Version")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<i64>().ok())
            .ok_or_else(|| ZotError::Remote {
                code: "library-version".to_string(),
                message: "Response missing Last-Modified-Version header".to_string(),
                hint: None,
                status: None,
            })?;
        Ok(version)
    }

    async fn create_attachment_item(&self, parent_key: &str, filename: &str) -> ZotResult<String> {
        let content_type = guess_content_type(filename);
        let payload = json!([{
            "itemType": "attachment",
            "parentItem": parent_key,
            "linkMode": "imported_file",
            "title": filename,
            "filename": filename,
            "contentType": content_type,
        }]);
        let response = send_with_retry(
            self.zotero_post("items")
                .header("Zotero-Write-Token", Uuid::new_v4().to_string())
                .json(&payload),
            "create-attachment-item",
        )
        .await?;
        let body: MultiWriteResponse = read_json(response, "create-attachment-item").await?;
        body.successful
            .and_then(|successful| successful.get("0").and_then(|entry| entry.key.clone()))
            .ok_or_else(|| ZotError::Remote {
                code: "create-attachment-item".to_string(),
                message: "Unexpected attachment item response".to_string(),
                hint: None,
                status: None,
            })
    }

    async fn authorize_attachment_upload(
        &self,
        attachment_key: &str,
        source: &PreparedAttachment,
    ) -> ZotResult<FileUploadAuthorization> {
        let body = format!(
            "md5={}&filename={}&filesize={}&mtime={}",
            source.md5_hash,
            urlencoding::encode(&source.filename),
            source.size,
            source.modified
        );
        let response = self
            .zotero_post(&format!("items/{attachment_key}/file"))
            .header("If-None-Match", "*")
            .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .map_err(remote_err("attachment-authorize"))?;
        let auth: FileUploadAuthorization = read_json(response, "attachment-authorize").await?;
        Ok(auth)
    }

    async fn cleanup_attachment_item(&self, attachment_key: &str) -> ZotResult<()> {
        let item = self.get_item_data(attachment_key).await?;
        let response = self
            .zotero_delete(&format!("items/{attachment_key}"))
            .header("If-Unmodified-Since-Version", item.version().to_string())
            .send()
            .await
            .map_err(remote_err("attachment-cleanup"))?;
        ensure_empty(response, "attachment-cleanup").await
    }

    async fn with_attachment_cleanup(&self, attachment_key: &str, error: ZotError) -> ZotError {
        let original = error.payload();
        let status = match error {
            ZotError::Remote { status, .. } | ZotError::Connector { status, .. } => status,
            _ => None,
        };
        let cleanup = match self.cleanup_attachment_item(attachment_key).await {
            Ok(()) => "Orphan attachment cleanup succeeded".to_string(),
            Err(cleanup_error) => format!(
                "Orphan attachment cleanup failed: {}",
                sanitize_cleanup_message(&cleanup_error.payload().message)
            ),
        };
        let hint = match original.hint {
            Some(hint) => Some(format!("{hint}; {cleanup}")),
            None => Some(cleanup),
        };
        ZotError::Remote {
            code: original.code,
            message: original.message,
            hint,
            status,
        }
    }

    async fn get_item_data(&self, key: &str) -> ZotResult<EditableObject> {
        let response =
            send_with_retry(self.zotero_get(&format!("items/{key}")), "get-item").await?;
        read_json(response, "get-item").await
    }

    async fn get_collection_data(&self, key: &str) -> ZotResult<EditableObject> {
        let response = send_with_retry(
            self.zotero_get(&format!("collections/{key}")),
            "get-collection",
        )
        .await?;
        read_json(response, "get-collection").await
    }
}

struct PreparedAttachment {
    file: tokio::fs::File,
    path: std::path::PathBuf,
    filename: String,
    size: u64,
    modified: u128,
    md5_hash: String,
}

impl PreparedAttachment {
    async fn open(path: &Path) -> ZotResult<Self> {
        let mut file = tokio::fs::File::open(path)
            .await
            .map_err(|source| ZotError::Io {
                path: path.to_path_buf(),
                source,
            })?;
        let metadata = file.metadata().await.map_err(|source| ZotError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        if !metadata.is_file() {
            return Err(ZotError::InvalidInput {
                code: "attachment-file".to_string(),
                message: "Attachment source must be a regular file".to_string(),
                hint: Some("Choose a regular local file to attach".to_string()),
            });
        }
        if metadata.len() > MAX_ATTACHMENT_BYTES {
            return Err(attachment_size_error());
        }

        let mut digest = md5::Context::new();
        let mut total = 0_u64;
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            let read = file
                .read(&mut buffer)
                .await
                .map_err(|source| ZotError::Io {
                    path: path.to_path_buf(),
                    source,
                })?;
            if read == 0 {
                break;
            }
            total = total
                .checked_add(read as u64)
                .ok_or_else(attachment_size_error)?;
            if total > MAX_ATTACHMENT_BYTES {
                return Err(attachment_size_error());
            }
            digest.consume(&buffer[..read]);
        }
        if total != metadata.len() {
            return Err(attachment_changed_error());
        }
        file.seek(SeekFrom::Start(0))
            .await
            .map_err(|source| ZotError::Io {
                path: path.to_path_buf(),
                source,
            })?;

        let filename = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("attachment.bin")
            .to_string();
        let modified = metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|duration| duration.as_millis())
            .unwrap_or_default();
        Ok(Self {
            file,
            path: path.to_path_buf(),
            filename,
            size: metadata.len(),
            modified,
            md5_hash: format!("{:x}", digest.finalize()),
        })
    }

    async fn upload_payload(&mut self, prefix: String, suffix: String) -> ZotResult<Vec<u8>> {
        let overhead = prefix
            .len()
            .checked_add(suffix.len())
            .ok_or_else(attachment_size_error)?;
        if overhead > MAX_UPLOAD_OVERHEAD_BYTES {
            return Err(ZotError::Remote {
                code: "attachment-upload".to_string(),
                message: "Attachment upload authorization overhead is too large".to_string(),
                hint: Some("Retry attachment authorization".to_string()),
                status: None,
            });
        }
        let capacity = usize::try_from(self.size)
            .ok()
            .and_then(|size| size.checked_add(overhead))
            .ok_or_else(attachment_size_error)?;
        self.file
            .seek(SeekFrom::Start(0))
            .await
            .map_err(|source| ZotError::Io {
                path: self.path.clone(),
                source,
            })?;
        let mut payload = Vec::with_capacity(capacity);
        payload.extend_from_slice(prefix.as_bytes());
        let file_start = payload.len();
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            let read = self
                .file
                .read(&mut buffer)
                .await
                .map_err(|source| ZotError::Io {
                    path: self.path.clone(),
                    source,
                })?;
            if read == 0 {
                break;
            }
            if payload.len().saturating_sub(file_start) + read > MAX_ATTACHMENT_BYTES as usize {
                return Err(attachment_size_error());
            }
            payload.extend_from_slice(&buffer[..read]);
        }
        if payload.len().saturating_sub(file_start) != self.size as usize {
            return Err(attachment_changed_error());
        }
        payload.extend_from_slice(suffix.as_bytes());
        Ok(payload)
    }
}

fn attachment_size_error() -> ZotError {
    ZotError::InvalidInput {
        code: "attachment-size".to_string(),
        message: format!("Attachment exceeds the {MAX_ATTACHMENT_BYTES}-byte limit"),
        hint: Some("Choose a smaller attachment".to_string()),
    }
}

fn attachment_changed_error() -> ZotError {
    ZotError::InvalidInput {
        code: "attachment-changed".to_string(),
        message: "Attachment changed while it was being prepared".to_string(),
        hint: Some("Retry after the file is no longer being modified".to_string()),
    }
}

fn sanitize_cleanup_message(message: &str) -> String {
    let mut sanitized = String::new();
    let mut previous_space = true;
    for character in message.chars().take(512) {
        if character.is_whitespace() {
            if !previous_space {
                sanitized.push(' ');
                previous_space = true;
            }
        } else if !character.is_control() {
            sanitized.push(character);
            previous_space = false;
        }
    }
    sanitized.trim().to_string()
}

#[derive(Debug, Deserialize)]
struct MultiWriteResponse {
    successful: Option<BTreeMap<String, WriteEntry>>,
}

#[derive(Debug, Deserialize)]
struct WriteEntry {
    key: Option<String>,
}

#[derive(Debug, Deserialize)]
struct EditableObject {
    key: String,
    version: i64,
    data: Value,
}

impl EditableObject {
    fn version(&self) -> i64 {
        self.version
    }

    fn into_flat_value(self) -> Value {
        let mut data = self.data;
        if let Some(object) = data.as_object_mut() {
            object.insert("key".to_string(), Value::String(self.key));
            object.insert("version".to_string(), Value::Number(self.version.into()));
        }
        data
    }
}

#[derive(Debug, Deserialize)]
struct FileUploadAuthorization {
    exists: Option<bool>,
    url: Option<String>,
    #[serde(rename = "contentType")]
    content_type: Option<String>,
    prefix: Option<String>,
    suffix: Option<String>,
    #[serde(rename = "uploadKey")]
    upload_key: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawSavedSearch {
    key: String,
    version: i64,
    library: Option<RawSearchLibrary>,
    data: RawSavedSearchData,
}

#[derive(Debug, Deserialize)]
struct RawSearchLibrary {
    #[serde(rename = "type")]
    library_type: Option<String>,
    id: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct RawSavedSearchData {
    name: String,
    #[serde(default)]
    conditions: Vec<SavedSearchCondition>,
}

impl From<RawSavedSearch> for SavedSearch {
    fn from(value: RawSavedSearch) -> Self {
        Self {
            key: value.key,
            version: value.version,
            name: value.data.name,
            conditions: value.data.conditions,
            library_type: value
                .library
                .as_ref()
                .and_then(|library| library.library_type.clone()),
            library_id: value.library.and_then(|library| library.id),
        }
    }
}

fn first_created_key(keys: Vec<String>) -> ZotResult<String> {
    keys.into_iter().next().ok_or_else(|| ZotError::Remote {
        code: "create-item".to_string(),
        message: "Unexpected create item response".to_string(),
        hint: None,
        status: None,
    })
}

fn sanitize_flat_item_value(item: &Value) -> Value {
    let mut payload = item.clone();
    if let Some(object) = payload.as_object_mut() {
        object.remove("key");
        object.remove("version");
    }
    payload
}

fn guess_content_type(filename: &str) -> &'static str {
    if filename.ends_with(".pdf") {
        "application/pdf"
    } else if filename.ends_with(".txt") {
        "text/plain"
    } else {
        "application/octet-stream"
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::json;
    use uuid::Uuid;
    use zot_core::{LibraryScope, ZotError};

    use super::{MAX_ATTACHMENT_BYTES, ZoteroRemote};
    use crate::http::HttpRuntime;
    use crate::test_support::spawn_server;

    fn client(base_url: String) -> ZoteroRemote {
        ZoteroRemote::with_base_url_for_tests(
            &HttpRuntime::default(),
            "12345",
            "test-key",
            LibraryScope::User,
            base_url,
        )
        .expect("construct zotero remote")
    }

    #[test]
    fn production_upload_urls_require_https() {
        let remote = ZoteroRemote::new(
            &HttpRuntime::default(),
            "12345",
            "test-key",
            LibraryScope::User,
        )
        .expect("construct zotero remote");

        assert!(
            remote
                .external_upload_request("https://uploads.example.test/file")
                .is_ok()
        );
        for invalid in [
            "http://127.0.0.1/upload",
            "ftp://uploads.example.test/file",
            "not-a-url",
        ] {
            let err = remote
                .external_upload_request(invalid)
                .expect_err("insecure upload URL must fail");
            assert_eq!(
                err.payload().code,
                "attachment-upload-url",
                "url: {invalid}"
            );
        }
    }

    #[tokio::test]
    async fn external_attachment_upload_never_receives_zotero_api_key() {
        let (upload_url, upload_server) = spawn_server(vec![(201, "")]);
        let auth: &'static str = Box::leak(
            format!(
                r#"{{"exists":false,"url":"{upload_url}","uploadKey":"UPLOAD-KEY","contentType":"application/octet-stream","prefix":"","suffix":""}}"#
            )
            .into_boxed_str(),
        );
        let created = r#"{"successful":{"0":{"key":"ATTACH01"}}}"#;
        let (api_url, api_server) = spawn_server(vec![(200, created), (200, auth), (204, "")]);
        let remote = client(api_url);
        let file_path =
            std::env::temp_dir().join(format!("zot-attachment-upload-{}.bin", Uuid::new_v4()));
        std::fs::write(&file_path, b"attachment bytes").expect("write attachment fixture");

        let result = remote.upload_attachment("PARENT01", &file_path).await;
        std::fs::remove_file(&file_path).expect("remove attachment fixture");
        let api_requests = api_server.join().expect("API server thread panicked");
        let upload_requests = upload_server.join().expect("upload server thread panicked");

        assert_eq!(result.expect("attachment upload succeeds"), "ATTACH01");
        assert_eq!(api_requests.len(), 3);
        assert!(
            api_requests
                .iter()
                .all(|request| { request.header("zotero-api-key") == Some("test-key") })
        );
        assert!(
            api_requests
                .iter()
                .all(|request| request.header("Zotero-API-Version") == Some("3"))
        );
        assert_eq!(upload_requests.len(), 1);
        assert_eq!(upload_requests[0].method, "POST");
        assert_eq!(upload_requests[0].header("zotero-api-key"), None);
        assert_eq!(upload_requests[0].header("Zotero-API-Version"), None);
    }

    #[tokio::test]
    async fn authorization_failure_hard_deletes_orphan_attachment() {
        let created = r#"{"successful":{"0":{"key":"ATTACH01"}}}"#;
        let item = r#"{"key":"ATTACH01","version":7,"data":{"itemType":"attachment"}}"#;
        let (api_url, api_server) = spawn_server(vec![
            (200, created),
            (500, "authorize failed"),
            (200, item),
            (204, ""),
        ]);
        let remote = client(api_url);
        let file = tempfile::NamedTempFile::new().expect("create attachment fixture");
        std::fs::write(file.path(), b"attachment bytes").expect("write attachment fixture");

        let result = remote.upload_attachment("PARENT01", file.path()).await;
        let captured = api_server.join().expect("API server thread panicked");

        match result {
            Err(ZotError::Remote { code, hint, .. }) => {
                assert_eq!(code, "attachment-authorize");
                assert!(hint.is_some_and(|hint| hint.contains("cleanup succeeded")));
            }
            other => panic!("expected authorization failure, got {other:?}"),
        }
        assert_eq!(
            captured
                .iter()
                .map(|request| request.method.as_str())
                .collect::<Vec<_>>(),
            vec!["POST", "POST", "GET", "DELETE"]
        );
        assert_eq!(captured[3].header("If-Unmodified-Since-Version"), Some("7"));
    }

    #[tokio::test]
    async fn external_upload_failure_hard_deletes_orphan_attachment() {
        let (upload_url, upload_server) = spawn_server(vec![(500, "upload failed")]);
        let auth: &'static str = Box::leak(
            format!(
                r#"{{"exists":false,"url":"{upload_url}","uploadKey":"UPLOAD-KEY","contentType":"application/octet-stream","prefix":"","suffix":""}}"#
            )
            .into_boxed_str(),
        );
        let created = r#"{"successful":{"0":{"key":"ATTACH01"}}}"#;
        let item = r#"{"key":"ATTACH01","version":8,"data":{"itemType":"attachment"}}"#;
        let (api_url, api_server) =
            spawn_server(vec![(200, created), (200, auth), (200, item), (204, "")]);
        let remote = client(api_url);
        let file = tempfile::NamedTempFile::new().expect("create attachment fixture");
        std::fs::write(file.path(), b"attachment bytes").expect("write attachment fixture");

        let result = remote.upload_attachment("PARENT01", file.path()).await;
        let captured = api_server.join().expect("API server thread panicked");
        let _ = upload_server.join().expect("upload server thread panicked");

        match result {
            Err(ZotError::Remote { code, hint, .. }) => {
                assert_eq!(code, "attachment-upload");
                assert!(hint.is_some_and(|hint| hint.contains("cleanup succeeded")));
            }
            other => panic!("expected upload failure, got {other:?}"),
        }
        assert_eq!(captured[2].method, "GET");
        assert_eq!(captured[3].method, "DELETE");
    }

    #[tokio::test]
    async fn registration_failure_hard_deletes_orphan_attachment() {
        let (upload_url, upload_server) = spawn_server(vec![(201, "")]);
        let auth: &'static str = Box::leak(
            format!(
                r#"{{"exists":false,"url":"{upload_url}","uploadKey":"UPLOAD-KEY","contentType":"application/octet-stream","prefix":"","suffix":""}}"#
            )
            .into_boxed_str(),
        );
        let created = r#"{"successful":{"0":{"key":"ATTACH01"}}}"#;
        let item = r#"{"key":"ATTACH01","version":9,"data":{"itemType":"attachment"}}"#;
        let (api_url, api_server) = spawn_server(vec![
            (200, created),
            (200, auth),
            (500, "register failed"),
            (200, item),
            (204, ""),
        ]);
        let remote = client(api_url);
        let file = tempfile::NamedTempFile::new().expect("create attachment fixture");
        std::fs::write(file.path(), b"attachment bytes").expect("write attachment fixture");

        let result = remote.upload_attachment("PARENT01", file.path()).await;
        let captured = api_server.join().expect("API server thread panicked");
        let _ = upload_server.join().expect("upload server thread panicked");

        match result {
            Err(ZotError::Remote { code, hint, .. }) => {
                assert_eq!(code, "attachment-register");
                assert!(hint.is_some_and(|hint| hint.contains("cleanup succeeded")));
            }
            other => panic!("expected registration failure, got {other:?}"),
        }
        assert_eq!(captured[3].method, "GET");
        assert_eq!(captured[4].method, "DELETE");
    }

    #[tokio::test]
    async fn cleanup_failure_keeps_original_error_and_cleanup_evidence() {
        let created = r#"{"successful":{"0":{"key":"ATTACH01"}}}"#;
        let (api_url, api_server) = spawn_server(vec![
            (200, created),
            (500, "authorize failed"),
            (500, "cleanup failed"),
            (500, "cleanup failed"),
            (500, "cleanup failed"),
        ]);
        let remote = client(api_url);
        let file = tempfile::NamedTempFile::new().expect("create attachment fixture");
        std::fs::write(file.path(), b"attachment bytes").expect("write attachment fixture");

        let result = remote.upload_attachment("PARENT01", file.path()).await;
        let captured = api_server.join().expect("API server thread panicked");

        match result {
            Err(ZotError::Remote { code, hint, .. }) => {
                assert_eq!(code, "attachment-authorize");
                assert!(hint.is_some_and(|hint| hint.contains("cleanup failed")));
            }
            other => panic!("expected original failure, got {other:?}"),
        }
        assert_eq!(captured.len(), 5);
        assert!(captured[2..].iter().all(|request| request.method == "GET"));
    }

    #[tokio::test]
    async fn oversize_attachment_fails_before_any_request() {
        let remote = client("http://127.0.0.1:1".to_string());
        let file = tempfile::NamedTempFile::new().expect("create attachment fixture");
        file.as_file()
            .set_len(MAX_ATTACHMENT_BYTES + 1)
            .expect("make sparse oversize fixture");

        let error = remote
            .upload_attachment("PARENT01", file.path())
            .await
            .expect_err("oversize attachment must fail locally");

        assert_eq!(error.payload().code, "attachment-size");
    }

    #[test]
    fn item_uri_uses_user_and_group_scope_prefixes() {
        // `dc:replaces` relations must carry the canonical zotero.org URI,
        // never the (overridable) API base URL.
        let runtime = HttpRuntime::default();
        let user = ZoteroRemote::with_base_url_for_tests(
            &runtime,
            "12345",
            "test-key",
            LibraryScope::User,
            "http://127.0.0.1:1",
        )
        .expect("construct user-scope remote");
        assert_eq!(
            user.item_uri("ABCD1234"),
            "http://zotero.org/users/12345/items/ABCD1234"
        );

        let group = ZoteroRemote::with_base_url_for_tests(
            &runtime,
            "67890",
            "test-key",
            LibraryScope::Group { group_id: 67890 },
            "http://127.0.0.1:1",
        )
        .expect("construct group-scope remote");
        assert_eq!(
            group.item_uri("ABCD1234"),
            "http://zotero.org/groups/67890/items/ABCD1234"
        );
    }

    #[tokio::test]
    async fn update_item_fields_carries_version_precondition_and_succeeds() {
        // GET returns the stored item at version 42; the follow-up PUT must
        // carry `If-Unmodified-Since-Version: 42`, and a 204 maps to Ok(()).
        let item =
            r#"{"key":"ABCD1234","version":42,"data":{"itemType":"journalArticle","title":"Old"}}"#;
        let (base_url, server) = spawn_server(vec![(200, item), (204, "")]);
        let remote = client(base_url);

        let fields = BTreeMap::from([("title".to_string(), "New".to_string())]);
        let result = remote.update_item_fields("ABCD1234", &fields).await;

        let captured = server.join().expect("server thread panicked");
        assert!(result.is_ok(), "expected Ok, got {result:?}");
        assert_eq!(captured.len(), 2);
        assert!(
            captured
                .iter()
                .all(|request| request.header("Zotero-API-Version") == Some("3"))
        );
        assert_eq!(captured[1].method, "PUT");
        assert_eq!(
            captured[1].header("If-Unmodified-Since-Version"),
            Some("42")
        );
    }

    #[tokio::test]
    async fn precondition_failure_maps_to_remote_412_with_conflict_hint() {
        let (base_url, server) = spawn_server(vec![(412, "conflict")]);
        let remote = client(base_url);

        let payload = json!({"key":"ABCD1234","version":10,"itemType":"journalArticle"});
        let result = remote.update_flat_item_value(&payload).await;
        let _ = server.join().expect("server thread panicked");

        match result {
            Err(ZotError::Remote { status, hint, .. }) => {
                assert_eq!(status, Some(412));
                assert_eq!(
                    hint.as_deref(),
                    Some("Object changed remotely; re-fetch before retrying")
                );
            }
            other => panic!("expected Remote 412, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn not_found_maps_to_remote_404_without_hint() {
        let (base_url, server) = spawn_server(vec![(404, "missing")]);
        let remote = client(base_url);

        let payload = json!({"key":"ABCD1234","version":10,"itemType":"journalArticle"});
        let result = remote.update_flat_item_value(&payload).await;
        let _ = server.join().expect("server thread panicked");

        match result {
            Err(ZotError::Remote { status, hint, .. }) => {
                assert_eq!(status, Some(404));
                assert_eq!(hint, None);
            }
            other => panic!("expected Remote 404, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn server_error_maps_to_remote_500_without_hint() {
        let (base_url, server) = spawn_server(vec![(500, "boom")]);
        let remote = client(base_url);

        let payload = json!({"key":"ABCD1234","version":10,"itemType":"journalArticle"});
        let result = remote.update_flat_item_value(&payload).await;
        let captured = server.join().expect("server thread panicked");
        assert_eq!(captured.len(), 1, "conditional PUT must not be retried");

        match result {
            Err(ZotError::Remote { status, hint, .. }) => {
                assert_eq!(status, Some(500));
                assert_eq!(hint, None);
            }
            other => panic!("expected Remote 500, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn create_item_decodes_multi_write_response() {
        let body = r#"{"successful":{"0":{"key":"NEWKEY12"}}}"#;
        let (base_url, server) = spawn_server(vec![(200, body)]);
        let remote = client(base_url);

        let result = remote.create_item(Some("10.1234/example"), None).await;

        let captured = server.join().expect("server thread panicked");
        assert_eq!(captured.len(), 1);
        assert_eq!(captured[0].method, "POST");
        assert!(
            captured[0].url.ends_with("/items"),
            "unexpected create url: {}",
            captured[0].url
        );
        assert_eq!(result.ok(), Some("NEWKEY12".to_string()));
        assert_eq!(captured[0].header("Zotero-API-Version"), Some("3"));
    }

    #[tokio::test]
    async fn retries_gets_on_429_and_server_errors() {
        let (base_url, server) = crate::test_support::spawn_server_with_headers(vec![
            (429, "limited", vec![("Retry-After", "0")]),
            (503, "busy", vec![("Retry-After", "0")]),
            (200, "[]", vec![]),
        ]);
        let remote = client(base_url);

        let result = remote.list_saved_searches().await;
        let captured = server.join().expect("server thread panicked");

        assert!(result.is_ok(), "expected retry recovery, got {result:?}");
        assert_eq!(captured.len(), 3);
        assert!(captured.iter().all(|request| request.method == "GET"));
    }

    #[tokio::test]
    async fn retries_write_token_create_with_the_same_token() {
        let created = r#"{"successful":{"0":{"key":"NEWKEY12"}}}"#;
        let (base_url, server) = crate::test_support::spawn_server_with_headers(vec![
            (503, "busy", vec![("Retry-After", "0")]),
            (200, created, vec![]),
        ]);
        let remote = client(base_url);

        let result = remote.create_item(Some("10.1234/example"), None).await;
        let captured = server.join().expect("server thread panicked");

        assert_eq!(result.ok(), Some("NEWKEY12".to_string()));
        assert_eq!(captured.len(), 2);
        let first = captured[0]
            .header("Zotero-Write-Token")
            .expect("first request has write token");
        assert_eq!(captured[1].header("Zotero-Write-Token"), Some(first));
    }

    #[tokio::test]
    async fn error_bodies_are_bounded_and_sanitized() {
        let body: &'static str =
            Box::leak(format!("remote\tmessage\0{}", "x".repeat(8 * 1024)).into_boxed_str());
        let (base_url, server) = spawn_server(vec![(500, body)]);
        let remote = client(base_url);
        let payload = json!({"key":"ABCD1234","version":10,"itemType":"journalArticle"});

        let result = remote.update_flat_item_value(&payload).await;
        let _ = server.join().expect("server thread panicked");

        match result {
            Err(ZotError::Remote { message, .. }) => {
                assert!(message.contains("[truncated]"));
                assert!(!message.contains('\0'));
                assert!(
                    message.len() < 4300,
                    "bounded message length: {}",
                    message.len()
                );
            }
            other => panic!("expected bounded Remote error, got {other:?}"),
        }
    }
}
