use std::collections::BTreeMap;
use std::path::Path;

use reqwest::header::{CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue};
use reqwest::{Method, StatusCode};
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;
use zot_core::{LibraryScope, ZotError, ZotResult};

const API_BASE: &str = "https://api.zotero.org";

#[derive(Clone)]
pub struct ZoteroRemote {
    client: reqwest::Client,
    library_id: String,
    scope: LibraryScope,
}

impl ZoteroRemote {
    pub fn new(
        library_id: impl Into<String>,
        api_key: &str,
        scope: LibraryScope,
    ) -> ZotResult<Self> {
        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static("zotero-api-key"),
            HeaderValue::from_str(api_key).map_err(|err| ZotError::InvalidInput {
                code: "api-key".to_string(),
                message: err.to_string(),
                hint: None,
            })?,
        );
        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .map_err(remote_err("client-build"))?;
        Ok(Self {
            client,
            library_id: library_id.into(),
            scope,
        })
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
        let response = self
            .client
            .post(self.endpoint("items"))
            .header("Zotero-Write-Token", Uuid::new_v4().to_string())
            .json(&payload)
            .send()
            .await
            .map_err(remote_err("create-item"))?;
        let body: MultiWriteResponse = self.ensure_json(response, "create-item").await?;
        body.successful
            .and_then(|successful| successful.get("0").and_then(|entry| entry.key.clone()))
            .ok_or_else(|| ZotError::Remote {
                code: "create-item".to_string(),
                message: "Unexpected create item response".to_string(),
                hint: None,
                status: None,
            })
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
            .client
            .put(self.endpoint(&format!("items/{key}")))
            .header("If-Unmodified-Since-Version", version.to_string())
            .json(&item.data)
            .send()
            .await
            .map_err(remote_err("update-item"))?;
        self.ensure_empty(response, "update-item").await
    }

    pub async fn delete_item(&self, key: &str) -> ZotResult<()> {
        let item = self.get_item_data(key).await?;
        let response = self
            .client
            .delete(self.endpoint(&format!("items/{key}")))
            .header("If-Unmodified-Since-Version", item.version().to_string())
            .send()
            .await
            .map_err(remote_err("delete-item"))?;
        self.ensure_empty(response, "delete-item").await
    }

    pub async fn restore_item(&self, key: &str) -> ZotResult<()> {
        let mut item = self.get_item_data(key).await?;
        item.data["deleted"] = Value::Number(0.into());
        let response = self
            .client
            .patch(self.endpoint(&format!("items/{key}")))
            .header("If-Unmodified-Since-Version", item.version().to_string())
            .json(&item.data)
            .send()
            .await
            .map_err(remote_err("restore-item"))?;
        self.ensure_empty(response, "restore-item").await
    }

    pub async fn add_note(&self, parent_key: &str, content: &str) -> ZotResult<String> {
        let payload = json!([{
            "itemType": "note",
            "parentItem": parent_key,
            "note": content,
        }]);
        let response = self
            .client
            .post(self.endpoint("items"))
            .header("Zotero-Write-Token", Uuid::new_v4().to_string())
            .json(&payload)
            .send()
            .await
            .map_err(remote_err("add-note"))?;
        let body: MultiWriteResponse = self.ensure_json(response, "add-note").await?;
        body.successful
            .and_then(|successful| successful.get("0").and_then(|entry| entry.key.clone()))
            .ok_or_else(|| ZotError::Remote {
                code: "add-note".to_string(),
                message: "Unexpected add note response".to_string(),
                hint: None,
                status: None,
            })
    }

    pub async fn update_note(&self, note_key: &str, content: &str) -> ZotResult<()> {
        let mut item = self.get_item_data(note_key).await?;
        item.data["note"] = Value::String(content.to_string());
        let response = self
            .client
            .put(self.endpoint(&format!("items/{note_key}")))
            .header("If-Unmodified-Since-Version", item.version().to_string())
            .json(&item.data)
            .send()
            .await
            .map_err(remote_err("update-note"))?;
        self.ensure_empty(response, "update-note").await
    }

    pub async fn add_tags(&self, key: &str, tags: &[String]) -> ZotResult<()> {
        let mut item = self.get_item_data(key).await?;
        let existing = item
            .data
            .get("tags")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let mut merged = existing
            .into_iter()
            .filter_map(|entry| {
                entry
                    .get("tag")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned)
            })
            .collect::<Vec<_>>();
        for tag in tags {
            if !merged.contains(tag) {
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
            .client
            .put(self.endpoint(&format!("items/{key}")))
            .header("If-Unmodified-Since-Version", item.version().to_string())
            .json(&item.data)
            .send()
            .await
            .map_err(remote_err("add-tags"))?;
        self.ensure_empty(response, "add-tags").await
    }

    pub async fn remove_tags(&self, key: &str, tags: &[String]) -> ZotResult<()> {
        let mut item = self.get_item_data(key).await?;
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
                    .map(|tag| !tags.iter().any(|candidate| candidate == tag))
                    .unwrap_or(true)
            })
            .collect::<Vec<_>>();
        item.data["tags"] = Value::Array(filtered);
        let response = self
            .client
            .put(self.endpoint(&format!("items/{key}")))
            .header("If-Unmodified-Since-Version", item.version().to_string())
            .json(&item.data)
            .send()
            .await
            .map_err(remote_err("remove-tags"))?;
        self.ensure_empty(response, "remove-tags").await
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
        let response = self
            .client
            .post(self.endpoint("collections"))
            .header("Zotero-Write-Token", Uuid::new_v4().to_string())
            .json(&payload)
            .send()
            .await
            .map_err(remote_err("create-collection"))?;
        let body: MultiWriteResponse = self.ensure_json(response, "create-collection").await?;
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
            .client
            .put(self.endpoint(&format!("collections/{key}")))
            .header(
                "If-Unmodified-Since-Version",
                collection.version().to_string(),
            )
            .json(&collection.data)
            .send()
            .await
            .map_err(remote_err("rename-collection"))?;
        self.ensure_empty(response, "rename-collection").await
    }

    pub async fn delete_collection(&self, key: &str) -> ZotResult<()> {
        let collection = self.get_collection_data(key).await?;
        let response = self
            .client
            .delete(self.endpoint(&format!("collections/{key}")))
            .header(
                "If-Unmodified-Since-Version",
                collection.version().to_string(),
            )
            .send()
            .await
            .map_err(remote_err("delete-collection"))?;
        self.ensure_empty(response, "delete-collection").await
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
            .client
            .patch(self.endpoint(&format!("items/{item_key}")))
            .header("If-Unmodified-Since-Version", item.version().to_string())
            .json(&item.data)
            .send()
            .await
            .map_err(remote_err("add-item-to-collection"))?;
        self.ensure_empty(response, "add-item-to-collection").await
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
            .client
            .patch(self.endpoint(&format!("items/{item_key}")))
            .header("If-Unmodified-Since-Version", item.version().to_string())
            .json(&item.data)
            .send()
            .await
            .map_err(remote_err("remove-item-from-collection"))?;
        self.ensure_empty(response, "remove-item-from-collection")
            .await
    }

    pub async fn upload_attachment(&self, parent_key: &str, file_path: &Path) -> ZotResult<String> {
        let attachment_key = self.create_attachment_item(parent_key, file_path).await?;
        let auth = self
            .authorize_attachment_upload(&attachment_key, file_path)
            .await?;
        if auth.exists.unwrap_or(false) {
            return Ok(attachment_key);
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
        let bytes = tokio::fs::read(file_path)
            .await
            .map_err(|source| ZotError::Io {
                path: file_path.to_path_buf(),
                source,
            })?;
        let mut payload = prefix.into_bytes();
        payload.extend_from_slice(&bytes);
        payload.extend_from_slice(suffix.as_bytes());
        let upload_response = self
            .client
            .post(upload_url)
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
            .client
            .post(self.endpoint(&format!("items/{attachment_key}/file")))
            .header("If-None-Match", "*")
            .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(format!("upload={upload_key}"))
            .send()
            .await
            .map_err(remote_err("attachment-register"))?;
        self.ensure_empty(register_response, "attachment-register")
            .await?;
        Ok(attachment_key)
    }

    fn endpoint(&self, path: &str) -> String {
        let scope = match self.scope {
            LibraryScope::User => format!("users/{}", self.library_id),
            LibraryScope::Group { .. } => format!("groups/{}", self.library_id),
        };
        format!("{API_BASE}/{scope}/{path}")
    }

    async fn create_attachment_item(
        &self,
        parent_key: &str,
        file_path: &Path,
    ) -> ZotResult<String> {
        let filename = file_path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("attachment.bin");
        let content_type = guess_content_type(filename);
        let payload = json!([{
            "itemType": "attachment",
            "parentItem": parent_key,
            "linkMode": "imported_file",
            "title": filename,
            "filename": filename,
            "contentType": content_type,
        }]);
        let response = self
            .client
            .post(self.endpoint("items"))
            .header("Zotero-Write-Token", Uuid::new_v4().to_string())
            .json(&payload)
            .send()
            .await
            .map_err(remote_err("create-attachment-item"))?;
        let body: MultiWriteResponse = self.ensure_json(response, "create-attachment-item").await?;
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
        file_path: &Path,
    ) -> ZotResult<FileUploadAuthorization> {
        let bytes = tokio::fs::read(file_path)
            .await
            .map_err(|source| ZotError::Io {
                path: file_path.to_path_buf(),
                source,
            })?;
        let filename = file_path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("attachment.bin");
        let metadata = tokio::fs::metadata(file_path)
            .await
            .map_err(|source| ZotError::Io {
                path: file_path.to_path_buf(),
                source,
            })?;
        let modified = metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|duration| duration.as_millis())
            .unwrap_or_default();
        let md5_hash = format!("{:x}", md5::compute(&bytes));
        let body = format!(
            "md5={}&filename={}&filesize={}&mtime={}",
            md5_hash,
            urlencoding::encode(filename),
            bytes.len(),
            modified
        );
        let response = self
            .client
            .post(self.endpoint(&format!("items/{attachment_key}/file")))
            .header("If-None-Match", "*")
            .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .map_err(remote_err("attachment-authorize"))?;
        self.ensure_json(response, "attachment-authorize").await
    }

    async fn get_item_data(&self, key: &str) -> ZotResult<EditableObject> {
        let response = self
            .client
            .request(Method::GET, self.endpoint(&format!("items/{key}")))
            .send()
            .await
            .map_err(remote_err("get-item"))?;
        self.ensure_json(response, "get-item").await
    }

    async fn get_collection_data(&self, key: &str) -> ZotResult<EditableObject> {
        let response = self
            .client
            .request(Method::GET, self.endpoint(&format!("collections/{key}")))
            .send()
            .await
            .map_err(remote_err("get-collection"))?;
        self.ensure_json(response, "get-collection").await
    }

    async fn ensure_empty(&self, response: reqwest::Response, code: &str) -> ZotResult<()> {
        if response.status().is_success() {
            Ok(())
        } else {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            Err(ZotError::Remote {
                code: code.to_string(),
                message: format!("Request failed with status {status}: {body}"),
                hint: http_hint(StatusCode::from_u16(status).ok()),
                status: Some(status),
            })
        }
    }

    async fn ensure_json<T: for<'de> Deserialize<'de>>(
        &self,
        response: reqwest::Response,
        code: &str,
    ) -> ZotResult<T> {
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(ZotError::Remote {
                code: code.to_string(),
                message: format!("Request failed with status {}: {body}", status.as_u16()),
                hint: http_hint(Some(status)),
                status: Some(status.as_u16()),
            });
        }
        response.json::<T>().await.map_err(|err| ZotError::Remote {
            code: code.to_string(),
            message: err.to_string(),
            hint: http_hint(err.status()),
            status: err.status().map(|status| status.as_u16()),
        })
    }
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
    data: Value,
}

impl EditableObject {
    fn version(&self) -> i64 {
        self.data
            .get("version")
            .and_then(Value::as_i64)
            .unwrap_or(0)
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

fn guess_content_type(filename: &str) -> &'static str {
    if filename.ends_with(".pdf") {
        "application/pdf"
    } else if filename.ends_with(".txt") {
        "text/plain"
    } else {
        "application/octet-stream"
    }
}

fn remote_err(code: &'static str) -> impl Fn(reqwest::Error) -> ZotError {
    move |err| ZotError::Remote {
        code: code.to_string(),
        message: err.to_string(),
        hint: http_hint(err.status()),
        status: err.status().map(|status| status.as_u16()),
    }
}

fn http_hint(status: Option<StatusCode>) -> Option<String> {
    match status {
        Some(StatusCode::FORBIDDEN) => Some("Check that the API key has write access".to_string()),
        Some(StatusCode::PRECONDITION_FAILED) => {
            Some("Object changed remotely; re-fetch before retrying".to_string())
        }
        Some(StatusCode::PRECONDITION_REQUIRED) => {
            Some("Missing version or If-Match precondition".to_string())
        }
        Some(StatusCode::CONFLICT) => Some("The target library is locked".to_string()),
        _ => None,
    }
}
