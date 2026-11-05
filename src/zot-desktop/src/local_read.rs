//! Read-only Local API mapping onto the existing item DTOs.
//!
//! GETs are built by [`crate::local_api::LocalApiClient::read_exchange`]. This module does not
//! authorize, send a write key, or follow redirects. Fields that have no DTO home are omitted.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};
use zot_core::{
    Attachment, ChildAnnotation, ChildAttachment, ChildItem, ChildNote, CitationKeyMatch, Creator,
    Item, LibraryScope, Note, SearchItemDirection, SearchItemSort, SearchResult, ZotError,
    ZotResult, extra_line_has_citation_key, item_matches_creator_filter,
    item_matches_field_filters, item_matches_metadata_query, item_matches_tag_filter,
    item_matches_year_filter, sort_search_items,
};

use crate::local_api::{LocalApiClient, error, next_start, prefix};

const CHILD_TYPES: &[&str] = &["attachment", "note", "annotation"];
const SKIP_DATA_KEYS: &[&str] = &[
    "key",
    "version",
    "itemType",
    "title",
    "creators",
    "abstractNote",
    "date",
    "url",
    "DOI",
    "tags",
    "collections",
    "relations",
    "dateAdded",
    "dateModified",
    "deleted",
    "parentItem",
    "inPublications",
    "contentType",
    "filename",
    "linkMode",
    "charset",
    "path",
    "note",
    "md5",
    "mtime",
    "annotationType",
    "annotationText",
    "annotationComment",
    "annotationColor",
    "annotationPageLabel",
    "annotationSortIndex",
    "annotationPosition",
];

/// Item JSON fields this mapper does not copy into a DTO or invent as zero.
pub const ABSENT_ITEM_FIELDS: &[&str] = &[
    "version",
    "relations",
    "tagType",
    "deleted",
    "linkMode",
    "charset",
    "path",
    "annotationPosition",
    "libraryID",
];

#[derive(Debug, Clone, Copy)]
pub struct DesktopScanLimits {
    pub page_size: u64,
    pub max_pages: u64,
    pub version_attempts: u32,
}

impl Default for DesktopScanLimits {
    fn default() -> Self {
        Self {
            page_size: 100,
            max_pages: 20,
            version_attempts: 3,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DesktopSearchQuery {
    pub query: String,
    pub collection: Option<String>,
    pub item_type: Option<String>,
    pub tag: Option<String>,
    pub creator: Option<String>,
    pub year: Option<String>,
    pub title: Option<String>,
    pub journal: Option<String>,
    pub topic: Option<String>,
    pub sort: Option<SearchItemSort>,
    pub direction: SearchItemDirection,
    pub limit: usize,
    pub offset: usize,
    pub exclude_trashed: bool,
}

impl Default for DesktopSearchQuery {
    fn default() -> Self {
        Self {
            query: String::new(),
            collection: None,
            item_type: None,
            tag: None,
            creator: None,
            year: None,
            title: None,
            journal: None,
            topic: None,
            sort: None,
            direction: SearchItemDirection::Desc,
            limit: 50,
            offset: 0,
            exclude_trashed: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DesktopItemView {
    pub item: Item,
    pub notes: Vec<Note>,
    pub attachments: Vec<Attachment>,
}

enum Attempt<T> {
    Ready(T),
    Inconsistent,
}

struct ListedItem {
    item: Item,
    deleted: bool,
}

pub struct DesktopRead {
    client: LocalApiClient,
    server_id: Option<String>,
    scope: LibraryScope,
    limits: DesktopScanLimits,
}

impl DesktopRead {
    pub fn new(client: LocalApiClient, server_id: Option<String>, scope: LibraryScope) -> Self {
        Self {
            client,
            server_id,
            scope,
            limits: DesktopScanLimits::default(),
        }
    }

    pub fn with_limits(mut self, limits: DesktopScanLimits) -> Self {
        self.limits = limits;
        self
    }

    pub async fn item_view(&self, key: &str) -> ZotResult<Option<DesktopItemView>> {
        require_read_key(key)?;
        for _ in 0..self.limits.version_attempts.max(1) {
            match self.item_view_once(key).await? {
                Attempt::Ready(value) => return Ok(value),
                Attempt::Inconsistent => continue,
            }
        }
        Err(inconsistent())
    }

    pub async fn children_for_keys(
        &self,
        keys: &[String],
    ) -> ZotResult<BTreeMap<String, Vec<ChildItem>>> {
        for key in keys {
            require_read_key(key)?;
        }
        for _ in 0..self.limits.version_attempts.max(1) {
            match self.children_once(keys).await? {
                Attempt::Ready(value) => return Ok(value),
                Attempt::Inconsistent => continue,
            }
        }
        Err(inconsistent())
    }

    pub async fn local_pdf_path(&self, key: &str) -> ZotResult<PathBuf> {
        require_read_key(key)?;
        for _ in 0..self.limits.version_attempts.max(1) {
            match self.pdf_once(key).await? {
                Attempt::Ready(value) => return Ok(value),
                Attempt::Inconsistent => continue,
            }
        }
        Err(inconsistent())
    }

    pub async fn search(&self, query: &DesktopSearchQuery) -> ZotResult<SearchResult> {
        for _ in 0..self.limits.version_attempts.max(1) {
            match self.search_once(query).await? {
                Attempt::Ready(value) => return Ok(value),
                Attempt::Inconsistent => continue,
            }
        }
        Err(inconsistent())
    }

    pub async fn citation_key_matches(&self, citekey: &str) -> ZotResult<Vec<CitationKeyMatch>> {
        for _ in 0..self.limits.version_attempts.max(1) {
            match self.citekey_once(citekey).await? {
                Attempt::Ready(value) => return Ok(value),
                Attempt::Inconsistent => continue,
            }
        }
        Err(inconsistent())
    }

    async fn item_view_once(&self, key: &str) -> ZotResult<Attempt<Option<DesktopItemView>>> {
        let mut seen = None;
        let exchange = self
            .client
            .read_exchange(
                &format!("{}/items/{key}", prefix(&self.scope)),
                self.server(),
            )
            .await?;
        if exchange.status.as_u16() == 404 {
            return Ok(Attempt::Ready(None));
        }
        if !observe(&mut seen, exchange.library_version) {
            return Ok(Attempt::Inconsistent);
        }
        let value = parse_json(&exchange.body)?;
        let Some(item) = map_primary_item(&value) else {
            return Ok(Attempt::Ready(None));
        };
        if CHILD_TYPES.contains(&item.item_type.as_str()) {
            return Ok(Attempt::Ready(None));
        }
        let children = match self.fetch_children(key, &mut seen).await? {
            Attempt::Inconsistent => return Ok(Attempt::Inconsistent),
            Attempt::Ready(children) => children,
        };
        Ok(Attempt::Ready(Some(DesktopItemView {
            notes: notes_of(&children, key),
            attachments: attachments_of(&children, key),
            item,
        })))
    }

    async fn children_once(
        &self,
        keys: &[String],
    ) -> ZotResult<Attempt<BTreeMap<String, Vec<ChildItem>>>> {
        let mut seen = None;
        let mut grouped = BTreeMap::new();
        for key in keys {
            match self.fetch_children(key, &mut seen).await? {
                Attempt::Inconsistent => return Ok(Attempt::Inconsistent),
                Attempt::Ready(children) => {
                    grouped.insert(key.clone(), children);
                }
            }
        }
        Ok(Attempt::Ready(grouped))
    }

    async fn pdf_once(&self, key: &str) -> ZotResult<Attempt<PathBuf>> {
        let mut seen = None;
        let item = self
            .client
            .read_exchange(
                &format!("{}/items/{key}", prefix(&self.scope)),
                self.server(),
            )
            .await?;
        if item.status.as_u16() == 404 {
            return Err(item_not_found(key));
        }
        if !observe(&mut seen, item.library_version) {
            return Ok(Attempt::Inconsistent);
        }
        let value = parse_json(&item.body)?;
        if map_primary_item(&value).is_none() {
            return Err(item_not_found(key));
        }
        let children = match self.fetch_children(key, &mut seen).await? {
            Attempt::Inconsistent => return Ok(Attempt::Inconsistent),
            Attempt::Ready(children) => children,
        };
        let Some(attachment_key) = first_pdf_key(&children) else {
            return Err(item_no_pdf(key));
        };
        let file = self
            .client
            .read_exchange(
                &format!(
                    "{}/items/{attachment_key}/file/view/url",
                    prefix(&self.scope)
                ),
                self.server(),
            )
            .await?;
        if matches!(file.status.as_u16(), 405 | 501) {
            return Err(error(
                "desktop-file-url-unsupported",
                "This Zotero version has no attachment file URL endpoint",
                Some(file.status.as_u16()),
            ));
        }
        if file.status.as_u16() == 404 {
            return Err(invalid(
                "desktop-file-missing",
                format!("Attachment file for '{attachment_key}' is missing"),
                Some("The Local API has no file URL for this attachment".to_string()),
            ));
        }
        if !file.status.is_success() {
            return Err(error(
                "local-api-http",
                "Local API rejected the attachment file URL read",
                Some(file.status.as_u16()),
            ));
        }
        if !observe(&mut seen, file.library_version) {
            return Ok(Attempt::Inconsistent);
        }
        let url = file.body.trim().trim_matches('"').trim();
        let path = local_path_from_file_url(url)?;
        verify_local_pdf(&path)?;
        Ok(Attempt::Ready(path))
    }

    async fn search_once(&self, query: &DesktopSearchQuery) -> ZotResult<Attempt<SearchResult>> {
        let mut seen = None;
        let collection = if let Some(name) = query.collection.as_deref() {
            match self.resolve_collection(name, &mut seen).await? {
                Attempt::Inconsistent => return Ok(Attempt::Inconsistent),
                Attempt::Ready(key) => Some(key),
            }
        } else {
            None
        };
        let listed = match self.fetch_items(!query.exclude_trashed, &mut seen).await? {
            Attempt::Inconsistent => return Ok(Attempt::Inconsistent),
            Attempt::Ready(listed) => listed,
        };
        Ok(Attempt::Ready(finish_search(
            listed,
            query,
            collection.as_deref(),
        )))
    }

    async fn citekey_once(&self, citekey: &str) -> ZotResult<Attempt<Vec<CitationKeyMatch>>> {
        let mut seen = None;
        let listed = match self.fetch_items(true, &mut seen).await? {
            Attempt::Inconsistent => return Ok(Attempt::Inconsistent),
            Attempt::Ready(listed) => listed,
        };
        let mut matches = BTreeMap::<String, CitationKeyMatch>::new();
        for listed in listed {
            if let Some(found) = citekey_hit(&listed.item, citekey) {
                let replace = matches
                    .get(&found.item.key)
                    .is_none_or(|existing| existing.source != "citation-key");
                if replace {
                    matches.insert(found.item.key.clone(), found);
                }
            }
        }
        Ok(Attempt::Ready(matches.into_values().collect()))
    }

    async fn resolve_collection(
        &self,
        name: &str,
        seen: &mut Option<u64>,
    ) -> ZotResult<Attempt<String>> {
        if self.server_id.is_none() {
            if collection_key_shape(name) {
                return Ok(Attempt::Ready(name.to_string()));
            }
            return Err(invalid(
                "collection-not-found",
                format!("Collection '{name}' cannot be resolved without a Zotero server id"),
                Some(
                    "Pass a collection key, or retry after a snapshot has recorded the server id"
                        .to_string(),
                ),
            ));
        }
        let rows = match self
            .fetch_pages(
                |start| {
                    format!(
                        "{}/collections?limit={}&start={start}",
                        prefix(&self.scope),
                        self.page_size()
                    )
                },
                seen,
                false,
            )
            .await?
        {
            Attempt::Inconsistent => return Ok(Attempt::Inconsistent),
            Attempt::Ready(rows) => rows,
        };
        let mut by_key = None;
        let mut by_name = Vec::new();
        for row in rows {
            let Some((key, collection_name)) = map_collection(&row) else {
                continue;
            };
            if key == name {
                by_key = Some(key);
                break;
            }
            if collection_name == name {
                by_name.push(key);
            }
        }
        if let Some(key) = by_key {
            return Ok(Attempt::Ready(key));
        }
        match by_name.as_slice() {
            [key] => Ok(Attempt::Ready(key.clone())),
            [] => Err(invalid(
                "collection-not-found",
                format!("Collection '{name}' not found"),
                Some("Use 'zot collection list' to inspect collection names".to_string()),
            )),
            keys => Err(invalid(
                "collection-ambiguous",
                format!("Collection name '{name}' matches multiple collections"),
                Some(format!(
                    "Use one of these collection keys: {}",
                    keys.join(", ")
                )),
            )),
        }
    }

    async fn fetch_items(
        &self,
        include_trashed: bool,
        seen: &mut Option<u64>,
    ) -> ZotResult<Attempt<Vec<ListedItem>>> {
        let rows = match self
            .fetch_pages(
                |start| items_path(&self.scope, start, self.page_size(), include_trashed),
                seen,
                false,
            )
            .await?
        {
            Attempt::Inconsistent => return Ok(Attempt::Inconsistent),
            Attempt::Ready(rows) => rows,
        };
        let mut listed = Vec::new();
        for row in rows {
            let Some(data) = data_map(&row) else {
                continue;
            };
            if is_child_type(data) {
                continue;
            }
            let Some(item) = map_primary_item(&row) else {
                continue;
            };
            listed.push(ListedItem {
                deleted: is_deleted(data),
                item,
            });
        }
        Ok(Attempt::Ready(listed))
    }

    async fn fetch_children(
        &self,
        key: &str,
        seen: &mut Option<u64>,
    ) -> ZotResult<Attempt<Vec<ChildItem>>> {
        let rows = match self
            .fetch_pages(
                |start| {
                    format!(
                        "{}/items/{key}/children?limit={}&start={start}",
                        prefix(&self.scope),
                        self.page_size()
                    )
                },
                seen,
                true,
            )
            .await?
        {
            Attempt::Inconsistent => return Ok(Attempt::Inconsistent),
            Attempt::Ready(rows) => rows,
        };
        Ok(Attempt::Ready(map_children(&rows, key)))
    }

    async fn fetch_pages<F>(
        &self,
        mut path_at: F,
        seen: &mut Option<u64>,
        missing_is_empty: bool,
    ) -> ZotResult<Attempt<Vec<Value>>>
    where
        F: FnMut(u64) -> String,
    {
        let mut rows = Vec::new();
        let mut start = 0_u64;
        let mut declared_total: Option<u64> = None;
        let page_size = self.page_size();
        for page in 0..self.limits.max_pages.max(1) {
            let exchange = self
                .client
                .read_exchange(&path_at(start), self.server())
                .await?;
            if exchange.status.as_u16() == 404 && page == 0 && missing_is_empty {
                return Ok(Attempt::Ready(Vec::new()));
            }
            if !exchange.status.is_success() {
                return Err(error(
                    "local-api-http",
                    "Local API rejected the paged read",
                    Some(exchange.status.as_u16()),
                ));
            }
            if !observe(seen, exchange.library_version) {
                return Ok(Attempt::Inconsistent);
            }
            if let Some(total) = exchange.total {
                if declared_total.is_some_and(|previous| previous != total) {
                    return Ok(Attempt::Inconsistent);
                }
                declared_total = Some(total);
            }
            let page_rows = parse_array(&exchange.body)?;
            let count = u64::try_from(page_rows.len()).unwrap_or(u64::MAX);
            rows.extend(page_rows);
            let collected = u64::try_from(rows.len()).unwrap_or(u64::MAX);
            if count < page_size {
                if declared_total.is_some_and(|total| total > collected) {
                    return Err(incomplete());
                }
                return Ok(Attempt::Ready(rows));
            }
            match next_start(exchange.link.as_deref(), start)? {
                Some(next) => start = next,
                None => {
                    if declared_total.is_some_and(|total| total > collected) {
                        return Err(incomplete());
                    }
                    return Ok(Attempt::Ready(rows));
                }
            }
        }
        Err(incomplete())
    }

    fn server(&self) -> Option<&str> {
        self.server_id.as_deref()
    }

    fn page_size(&self) -> u64 {
        self.limits.page_size.max(1)
    }
}

fn finish_search(
    listed: Vec<ListedItem>,
    query: &DesktopSearchQuery,
    collection: Option<&str>,
) -> SearchResult {
    let mut items = listed
        .into_iter()
        .filter(|listed| !query.exclude_trashed || !listed.deleted)
        .map(|listed| listed.item)
        .filter(|item| item_matches_query(item, query, collection))
        .collect::<Vec<_>>();
    sort_search_items(&mut items, query.sort, query.direction);
    let total = items.len();
    let items = items
        .into_iter()
        .skip(query.offset)
        .take(query.limit)
        .collect::<Vec<_>>();
    SearchResult {
        items,
        total,
        query: query.query.clone(),
        fulltext_index: if query.query.is_empty() {
            None
        } else {
            Some("unavailable".to_string())
        },
    }
}

fn item_matches_query(item: &Item, query: &DesktopSearchQuery, collection: Option<&str>) -> bool {
    if let Some(kind) = query.item_type.as_deref() {
        if item.item_type != kind {
            return false;
        }
    }
    if let Some(tag) = query.tag.as_deref() {
        if !item_matches_tag_filter(item, tag) {
            return false;
        }
    }
    if let Some(creator) = query.creator.as_deref() {
        if !item_matches_creator_filter(item, creator) {
            return false;
        }
    }
    if let Some(year) = query.year.as_deref() {
        if !item_matches_year_filter(item, year) {
            return false;
        }
    }
    if !item_matches_field_filters(
        item,
        query.title.as_deref(),
        query.journal.as_deref(),
        query.topic.as_deref(),
    ) {
        return false;
    }
    if !query.query.is_empty() && !item_matches_metadata_query(item, &query.query) {
        return false;
    }
    if let Some(collection) = collection {
        if !item.collections.iter().any(|key| key == collection) {
            return false;
        }
    }
    true
}

fn citekey_hit(item: &Item, citekey: &str) -> Option<CitationKeyMatch> {
    if item
        .extra
        .get("citationKey")
        .is_some_and(|value| value == citekey)
    {
        return Some(CitationKeyMatch {
            citekey: citekey.to_string(),
            source: "citation-key".to_string(),
            item: item.clone(),
        });
    }
    let extra = item.extra.get("extra").map(String::as_str).unwrap_or("");
    if extra_line_has_citation_key(extra, citekey) {
        return Some(CitationKeyMatch {
            citekey: citekey.to_string(),
            source: "extra".to_string(),
            item: item.clone(),
        });
    }
    None
}

fn items_path(scope: &LibraryScope, start: u64, limit: u64, include_trashed: bool) -> String {
    let mut path = format!("{}/items?limit={limit}&start={start}", prefix(scope));
    if include_trashed {
        path.push_str("&includeTrashed=1");
    }
    path
}

fn observe(seen: &mut Option<u64>, next: Option<u64>) -> bool {
    match (*seen, next) {
        (None, Some(version)) => {
            *seen = Some(version);
            true
        }
        (Some(previous), Some(version)) => previous == version,
        // A missing library version cannot prove the pages belong to one commit.
        _ => false,
    }
}

fn require_read_key(key: &str) -> ZotResult<()> {
    if key.is_empty() || !key.chars().all(|ch| ch.is_ascii_alphanumeric()) {
        return Err(invalid(
            "local-api-key",
            "Item key must be ASCII letters or digits".to_string(),
            None,
        ));
    }
    Ok(())
}

fn collection_key_shape(value: &str) -> bool {
    value.len() == 8
        && value
            .chars()
            .all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit())
}

pub fn local_path_from_file_url(url: &str) -> ZotResult<PathBuf> {
    let parsed = reqwest::Url::parse(url).map_err(|_| {
        invalid(
            "desktop-file-type",
            "Attachment file URL is not a local file URL".to_string(),
            None,
        )
    })?;
    if parsed.scheme() != "file" {
        return Err(invalid(
            "desktop-file-type",
            "Refusing a non-file attachment URL".to_string(),
            Some("Only file URLs returned by the Local API are read".to_string()),
        ));
    }
    match parsed.host_str() {
        None | Some("localhost") | Some("127.0.0.1") => {}
        Some(_) => {
            return Err(invalid(
                "desktop-file-type",
                "Refusing a remote attachment URL".to_string(),
                Some("Network file URLs are not followed".to_string()),
            ));
        }
    }
    parsed.to_file_path().map_err(|_| {
        invalid(
            "desktop-file-type",
            "Cannot convert the attachment file URL to a local path".to_string(),
            None,
        )
    })
}

pub fn verify_local_pdf(path: &Path) -> ZotResult<()> {
    let metadata = std::fs::symlink_metadata(path).map_err(|source| {
        if source.kind() == std::io::ErrorKind::NotFound {
            invalid(
                "desktop-file-missing",
                format!("Attachment file is missing: {}", path.display()),
                Some("The Local API returned a file URL, but that file is not on disk".to_string()),
            )
        } else {
            ZotError::Io {
                path: path.to_path_buf(),
                source,
            }
        }
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(file_type("Attachment path is not a regular PDF file"));
    }
    let extension = path.extension().and_then(|ext| ext.to_str()).unwrap_or("");
    if !extension.eq_ignore_ascii_case("pdf") {
        return Err(file_type("Attachment path is not a PDF file"));
    }
    let mut file = std::fs::File::open(path).map_err(|source| ZotError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let mut magic = [0_u8; 5];
    std::io::Read::read_exact(&mut file, &mut magic)
        .map_err(|_| file_type("Attachment file is not a PDF"))?;
    if &magic != b"%PDF-" {
        return Err(file_type("Attachment file is not a PDF"));
    }
    Ok(())
}

fn map_primary_item(value: &Value) -> Option<Item> {
    let data = data_map(value)?;
    let item_type = data
        .get("itemType")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    if item_type.is_empty() {
        return None;
    }
    let mut extra = BTreeMap::new();
    for (field, field_value) in data {
        if SKIP_DATA_KEYS.contains(&field.as_str()) {
            continue;
        }
        if let Some(text) = field_value.as_str() {
            extra.insert(field.clone(), text.to_string());
        }
    }
    Some(Item {
        key: object_key(value, data),
        item_type,
        title: match data.get("title") {
            Some(Value::String(title)) => title.clone(),
            _ => String::new(),
        },
        creators: map_creators(data.get("creators")),
        abstract_note: optional_string(data, "abstractNote"),
        date: optional_string(data, "date"),
        url: optional_string(data, "url"),
        doi: optional_string(data, "DOI"),
        tags: map_tags(data.get("tags")),
        collections: map_collection_keys(data.get("collections")),
        date_added: optional_string(data, "dateAdded"),
        date_modified: optional_string(data, "dateModified"),
        extra,
    })
}

fn map_children(values: &[Value], parent_key: &str) -> Vec<ChildItem> {
    let mut notes = Vec::new();
    let mut attachments = Vec::new();
    let mut annotations = Vec::new();
    for value in values {
        let Some(data) = data_map(value) else {
            continue;
        };
        let key = object_key(value, data);
        let item_type = data.get("itemType").and_then(Value::as_str).unwrap_or("");
        let parent = data
            .get("parentItem")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);
        match item_type {
            "note" => notes.push(ChildItem::Note(ChildNote {
                key,
                parent_key: Some(parent.unwrap_or_else(|| parent_key.to_string())),
                content: html_to_text(data.get("note").and_then(Value::as_str).unwrap_or("")),
                tags: map_tags(data.get("tags")),
            })),
            "attachment" => attachments.push(ChildItem::Attachment(ChildAttachment {
                key,
                parent_key: Some(parent.unwrap_or_else(|| parent_key.to_string())),
                filename: attachment_filename(data),
                content_type: data
                    .get("contentType")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                tags: Vec::new(),
            })),
            "annotation" => annotations.push(ChildItem::Annotation(ChildAnnotation {
                key,
                parent_key: parent,
                annotation_type: annotation_type(data),
                text: data
                    .get("annotationText")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                comment: data
                    .get("annotationComment")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                color: optional_string(data, "annotationColor"),
                page_label: optional_string(data, "annotationPageLabel"),
                tags: Vec::new(),
            })),
            _ => {}
        }
    }
    notes.extend(attachments);
    notes.extend(annotations);
    notes
}

fn notes_of(children: &[ChildItem], parent_key: &str) -> Vec<Note> {
    children
        .iter()
        .filter_map(|child| match child {
            ChildItem::Note(note) => Some(Note {
                key: note.key.clone(),
                parent_key: note
                    .parent_key
                    .clone()
                    .unwrap_or_else(|| parent_key.to_string()),
                content: note.content.clone(),
                tags: note.tags.clone(),
            }),
            _ => None,
        })
        .collect()
}

fn attachments_of(children: &[ChildItem], parent_key: &str) -> Vec<Attachment> {
    children
        .iter()
        .filter_map(|child| match child {
            ChildItem::Attachment(attachment) => Some(Attachment {
                key: attachment.key.clone(),
                parent_key: attachment
                    .parent_key
                    .clone()
                    .unwrap_or_else(|| parent_key.to_string()),
                filename: attachment.filename.clone(),
                content_type: attachment.content_type.clone(),
            }),
            _ => None,
        })
        .collect()
}

fn first_pdf_key(children: &[ChildItem]) -> Option<String> {
    children.iter().find_map(|child| match child {
        ChildItem::Attachment(attachment) if attachment.content_type == "application/pdf" => {
            Some(attachment.key.clone())
        }
        _ => None,
    })
}

fn map_collection(value: &Value) -> Option<(String, String)> {
    let data = data_map(value)?;
    let key = object_key(value, data);
    if key.is_empty() {
        return None;
    }
    let name = data
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    Some((key, name))
}

fn data_map(value: &Value) -> Option<&Map<String, Value>> {
    let object = value.as_object()?;
    match object.get("data") {
        Some(Value::Object(data)) => Some(data),
        _ => Some(object),
    }
}

fn object_key(value: &Value, data: &Map<String, Value>) -> String {
    data.get("key")
        .and_then(Value::as_str)
        .or_else(|| value.get("key").and_then(Value::as_str))
        .unwrap_or("")
        .to_string()
}

fn optional_string(data: &Map<String, Value>, name: &str) -> Option<String> {
    match data.get(name) {
        Some(Value::String(value)) => Some(value.clone()),
        _ => None,
    }
}

fn map_creators(value: Option<&Value>) -> Vec<Creator> {
    let Some(Value::Array(creators)) = value else {
        return Vec::new();
    };
    creators
        .iter()
        .filter_map(|creator| {
            let object = creator.as_object()?;
            let creator_type = object
                .get("creatorType")
                .and_then(Value::as_str)
                .unwrap_or("author")
                .to_string();
            if let Some(name) = object.get("name").and_then(Value::as_str) {
                return Some(Creator {
                    first_name: String::new(),
                    last_name: name.to_string(),
                    creator_type,
                });
            }
            Some(Creator {
                first_name: object
                    .get("firstName")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                last_name: object
                    .get("lastName")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                creator_type,
            })
        })
        .collect()
}

fn map_tags(value: Option<&Value>) -> Vec<String> {
    let Some(Value::Array(tags)) = value else {
        return Vec::new();
    };
    let mut names = tags
        .iter()
        .filter_map(|tag| {
            tag.as_object()
                .and_then(|object| object.get("tag"))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .collect::<Vec<_>>();
    names.sort();
    names
}

fn map_collection_keys(value: Option<&Value>) -> Vec<String> {
    let Some(Value::Array(keys)) = value else {
        return Vec::new();
    };
    let mut keys = keys
        .iter()
        .filter_map(|key| key.as_str().map(str::to_string))
        .collect::<Vec<_>>();
    keys.sort();
    keys
}

fn is_deleted(data: &Map<String, Value>) -> bool {
    match data.get("deleted") {
        Some(Value::Bool(value)) => *value,
        Some(Value::Number(value)) => value.as_i64() == Some(1),
        _ => false,
    }
}

fn is_child_type(data: &Map<String, Value>) -> bool {
    data.get("itemType")
        .and_then(Value::as_str)
        .is_some_and(|kind| CHILD_TYPES.contains(&kind))
}

fn attachment_filename(data: &Map<String, Value>) -> String {
    let raw_path = data.get("path").and_then(Value::as_str).unwrap_or("");
    if raw_path.is_empty() {
        return data
            .get("filename")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
    }
    raw_path
        .strip_prefix("storage:")
        .unwrap_or(raw_path)
        .to_string()
}

fn annotation_type(data: &Map<String, Value>) -> String {
    match data.get("annotationType") {
        Some(Value::String(value)) => value.clone(),
        Some(Value::Number(value)) => match value.as_i64() {
            Some(1) => "highlight".to_string(),
            Some(2) => "note".to_string(),
            Some(3) => "image".to_string(),
            Some(4) => "ink".to_string(),
            Some(5) => "underline".to_string(),
            _ => "unknown".to_string(),
        },
        _ => "unknown".to_string(),
    }
}

fn html_to_text(html: &str) -> String {
    let with_breaks = html
        .replace("<br>", "\n")
        .replace("<br/>", "\n")
        .replace("<br />", "\n")
        .replace("</p>", "\n\n")
        .replace("</li>", "\n");
    let mut out = String::new();
    let mut in_tag = false;
    for ch in with_breaks.chars() {
        if ch == '<' {
            in_tag = true;
            continue;
        }
        if in_tag {
            if ch == '>' {
                in_tag = false;
            }
            continue;
        }
        out.push(ch);
    }
    out.replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .trim()
        .to_string()
}

fn parse_json(body: &str) -> ZotResult<Value> {
    serde_json::from_str(body).map_err(|_| {
        error(
            "local-api-protocol",
            "Invalid Local API JSON response",
            None,
        )
    })
}

fn parse_array(body: &str) -> ZotResult<Vec<Value>> {
    match parse_json(body)? {
        Value::Array(rows) => Ok(rows),
        _ => Err(error(
            "local-api-protocol",
            "Local API page was not a JSON array",
            None,
        )),
    }
}

fn inconsistent() -> ZotError {
    error(
        "desktop-read-inconsistent",
        "Zotero library version changed while this command was reading",
        None,
    )
}

fn incomplete() -> ZotError {
    ZotError::LocalApi {
        code: "desktop-search-incomplete".to_string(),
        message: "Desktop item scan stopped before the library was fully read".to_string(),
        hint: Some(
            "Narrow the query, or close Zotero and use the snapshot. Incomplete scans do not report a total."
                .to_string(),
        ),
        status: None,
    }
}

fn item_not_found(key: &str) -> ZotError {
    invalid("item-not-found", format!("Item '{key}' not found"), None)
}

fn item_no_pdf(key: &str) -> ZotError {
    invalid(
        "item-no-pdf",
        format!("Item '{key}' has no PDF attachment"),
        None,
    )
}

fn file_type(message: &str) -> ZotError {
    invalid("desktop-file-type", message.to_string(), None)
}

fn invalid(code: &str, message: String, hint: Option<String>) -> ZotError {
    ZotError::InvalidInput {
        code: code.to_string(),
        message,
        hint,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::BTreeSet;
    use std::time::Duration;
    use tiny_http::{Header, Response as FakeResponse, Server};

    fn limits(page_size: u64, max_pages: u64) -> DesktopScanLimits {
        DesktopScanLimits {
            page_size,
            max_pages,
            version_attempts: 3,
        }
    }

    fn headers(version: &'static str) -> Vec<(&'static str, &'static str)> {
        vec![
            ("Zotero-API-Version", "3"),
            ("Zotero-Server-ID", "server-abc"),
            ("Last-Modified-Version", version),
        ]
    }

    type Reply = (u16, String, Vec<(&'static str, &'static str)>);
    type CapturedHit = (String, String, Vec<(String, String)>);

    fn server(responses: Vec<Reply>) -> (DesktopRead, std::thread::JoinHandle<Vec<CapturedHit>>) {
        let http = Server::http("127.0.0.1:0").expect("server");
        let client = LocalApiClient::with_base_url_for_tests(
            &format!("http://{}/", http.server_addr()),
            Duration::from_secs(2),
        )
        .expect("client");
        let reader = DesktopRead::new(client, Some("server-abc".to_string()), LibraryScope::User)
            .with_limits(limits(1, 5));
        let handle = std::thread::spawn(move || {
            let mut captured = Vec::new();
            for (status, body, headers) in responses {
                let request = http
                    .recv_timeout(Duration::from_secs(3))
                    .expect("recv")
                    .expect("request");
                captured.push((
                    request.method().to_string(),
                    request.url().to_string(),
                    request
                        .headers()
                        .iter()
                        .map(|header| {
                            (
                                header.field.to_string().to_lowercase(),
                                header.value.to_string(),
                            )
                        })
                        .collect(),
                ));
                let mut response = FakeResponse::from_string(body).with_status_code(status);
                for (name, value) in headers {
                    response.add_header(Header::from_bytes(name, value).expect("header"));
                }
                request.respond(response).expect("respond");
            }
            captured
        });
        (reader, handle)
    }

    fn article(key: &str, title: &str) -> Value {
        json!({
            "key": key,
            "version": 4,
            "data": {
                "key": key,
                "version": 4,
                "itemType": "journalArticle",
                "title": title,
                "creators": [
                    {"creatorType": "author", "firstName": "Ashish", "lastName": "Vaswani"},
                    {"creatorType": "author", "name": "DeepMind"}
                ],
                "abstractNote": "This abstract mentions Nature",
                "date": "2017",
                "DOI": "10.5555/attention",
                "publicationTitle": "Nature",
                "journalAbbreviation": "Nat.",
                "citationKey": "Smith2024",
                "extra": "Citation Key: Smith2024",
                "tags": [{"tag": "transformer", "type": 1}, {"tag": "attention", "type": 0}],
                "collections": ["COLML01"],
                "dateAdded": "2024-01-01",
                "dateModified": "2024-01-02",
                "relations": {"dc:relation": ["http://example.invalid"]},
                "deleted": false
            }
        })
    }

    #[test]
    fn maps_item_fields_without_inventing_absent_ones() {
        let item = map_primary_item(&article("ATTN001", "Deep  Learning")).expect("item");
        assert_eq!(item.title, "Deep  Learning");
        assert_eq!(item.creators[1].first_name, "");
        assert_eq!(item.creators[1].last_name, "DeepMind");
        assert_eq!(
            item.tags,
            vec!["attention".to_string(), "transformer".to_string()]
        );
        assert_eq!(
            item.extra.get("citationKey").map(String::as_str),
            Some("Smith2024")
        );
        assert_eq!(
            item.extra.get("publicationTitle").map(String::as_str),
            Some("Nature")
        );
        assert!(item.abstract_note.is_some());
        for absent in ABSENT_ITEM_FIELDS {
            assert!(!item.extra.contains_key(*absent), "{absent} was invented");
        }
        assert_ne!(item.doi.as_deref(), Some("0"));
    }

    #[test]
    fn file_urls_reject_network_targets_and_round_trip_local_paths() {
        let path = std::env::temp_dir().join("My Papers").join("论文.pdf");
        let url = reqwest::Url::from_file_path(&path).expect("file url");
        assert_eq!(
            local_path_from_file_url(url.as_str()).expect("convert"),
            path
        );
        assert!(local_path_from_file_url("https://example.invalid/paper.pdf").is_err());
        assert!(local_path_from_file_url("file://files.example/share/paper.pdf").is_err());
    }

    #[cfg(windows)]
    #[test]
    fn windows_file_url_decodes_drive_spaces_and_han_characters() {
        let path = local_path_from_file_url("file:///D:/My%20Papers/%E8%AE%BA%E6%96%87.pdf")
            .expect("windows path");
        assert_eq!(path, PathBuf::from(r"D:\My Papers\论文.pdf"));
    }

    #[test]
    fn verifies_regular_pdf_magic_and_reports_missing_files() {
        let dir = std::env::temp_dir().join(format!("zot-pdf-verify-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let pdf = dir.join("论文.pdf");
        std::fs::write(&pdf, b"%PDF-1.4\nFIXED-PAGE-1-TEXT\n").expect("write pdf");
        verify_local_pdf(&pdf).expect("pdf");
        let missing = dir.join("missing.pdf");
        let error = verify_local_pdf(&missing).expect_err("missing");
        assert_eq!(error.payload().code, "desktop-file-missing");
        let text = dir.join("notes.pdf");
        std::fs::write(&text, b"hello").expect("write text");
        assert_eq!(
            verify_local_pdf(&text).expect_err("magic").payload().code,
            "desktop-file-type"
        );
        assert_eq!(
            verify_local_pdf(&dir).expect_err("dir").payload().code,
            "desktop-file-type"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn item_get_maps_children_and_does_not_authorize() {
        let item = article("ATTN001", "Attention Is All You Need");
        let children = json!([
            {"key": "NOTE004", "data": {"key": "NOTE004", "itemType": "note", "parentItem": "ATTN001", "note": "<p>Hello</p>", "tags": [{"tag": "t"}]}},
            {"key": "ATCH005", "data": {"key": "ATCH005", "itemType": "attachment", "parentItem": "ATTN001", "contentType": "application/pdf", "path": "storage:attention.pdf"}},
            {"key": "LINK006", "data": {"key": "LINK006", "itemType": "attachment", "parentItem": "ATTN001", "contentType": "application/pdf", "path": "C:\\Papers\\论文.pdf"}},
            {"key": "ANNO011", "data": {"key": "ANNO011", "itemType": "annotation", "parentItem": "ATCH005", "annotationType": "highlight", "annotationText": "core", "tags": [{"tag": "hidden"}]}}
        ]);
        let (reader, handle) = server(vec![
            (200, item.to_string(), headers("7")),
            (
                200,
                children.to_string(),
                headers("7")
                    .into_iter()
                    .chain([("Total-Results", "4")])
                    .collect(),
            ),
        ]);
        let view = reader
            .item_view("ATTN001")
            .await
            .expect("view")
            .expect("item");
        assert_eq!(view.item.key, "ATTN001");
        assert_eq!(view.notes.len(), 1);
        assert_eq!(view.notes[0].content, "Hello");
        assert_eq!(view.notes[0].tags, vec!["t".to_string()]);
        assert_eq!(view.attachments[0].filename, "attention.pdf");
        assert_eq!(view.attachments[1].filename, "C:\\Papers\\论文.pdf");
        let captured = handle.join().expect("join");
        assert!(captured.iter().all(|(method, _, _)| method == "GET"));
        assert!(
            captured
                .iter()
                .all(|(_, url, _)| !url.contains("authorize"))
        );
        assert!(captured.iter().all(|(_, _, headers)| {
            headers.iter().all(|(name, _)| name != "zotero-api-key")
                && headers
                    .iter()
                    .any(|(name, value)| name == "zotero-server-id" && value == "server-abc")
        }));
    }

    #[tokio::test]
    async fn item_404_does_not_request_children() {
        let (reader, handle) = server(vec![(404, "{}".to_string(), headers("7"))]);
        assert!(reader.item_view("MISSING1").await.expect("404").is_none());
        assert_eq!(handle.join().expect("join").len(), 1);
    }

    #[tokio::test]
    async fn group_scope_uses_group_prefix_for_the_same_key() {
        let (http, client) = {
            let http = Server::http("127.0.0.1:0").expect("server");
            let client = LocalApiClient::with_base_url_for_tests(
                &format!("http://{}/", http.server_addr()),
                Duration::from_secs(2),
            )
            .expect("client");
            (http, client)
        };
        let reader = DesktopRead::new(
            client,
            Some("server-abc".to_string()),
            LibraryScope::Group { group_id: 9 },
        )
        .with_limits(limits(10, 2));
        let handle = std::thread::spawn(move || {
            let request = http
                .recv_timeout(Duration::from_secs(3))
                .expect("recv")
                .expect("request");
            let url = request.url().to_string();
            let response =
                FakeResponse::from_string(article("ITEM0001", "Group Only Paper").to_string())
                    .with_status_code(200);
            let mut response = response;
            for (name, value) in headers("3") {
                response.add_header(Header::from_bytes(name, value).expect("header"));
            }
            request.respond(response).expect("respond");
            let child = http
                .recv_timeout(Duration::from_secs(3))
                .expect("recv")
                .expect("request");
            let child_url = child.url().to_string();
            let mut response = FakeResponse::from_string("[]").with_status_code(200);
            for (name, value) in headers("3") {
                response.add_header(Header::from_bytes(name, value).expect("header"));
            }
            child.respond(response).expect("respond");
            (url, child_url)
        });
        let view = reader
            .item_view("ITEM0001")
            .await
            .expect("group")
            .expect("item");
        assert_eq!(view.item.title, "Group Only Paper");
        let (url, child_url) = handle.join().expect("join");
        assert_eq!(url, "/api/groups/9/items/ITEM0001");
        assert!(child_url.starts_with("/api/groups/9/items/ITEM0001/children"));
    }

    #[tokio::test]
    async fn search_filters_before_the_page_and_rejects_an_incomplete_walk() {
        let decoy = article("DECOY001", "Deep X Learning");
        let real = article("REAL0001", "Deep  Learning");
        let mut first_headers = headers("4");
        first_headers.push(("Total-Results", "2"));
        first_headers.push(("Link", "</api/users/0/items?limit=1&start=1>; rel=\"next\""));
        let (reader, handle) = server(vec![
            (200, json!([decoy]).to_string(), first_headers),
            (200, json!([real]).to_string(), headers("4")),
        ]);
        let found = reader
            .search(&DesktopSearchQuery {
                title: Some("deep learning".to_string()),
                ..DesktopSearchQuery::default()
            })
            .await
            .expect("search");
        assert_eq!(found.total, 1);
        assert_eq!(found.items[0].key, "REAL0001");
        assert!(found.fulltext_index.is_none());
        let captured = handle.join().expect("join");
        assert!(captured.iter().all(|(method, _, headers)| {
            method == "GET" && headers.iter().all(|(name, _)| name != "zotero-api-key")
        }));

        let mut only = headers("4");
        only.push(("Total-Results", "2"));
        only.push(("Link", "</api/users/0/items?limit=1&start=1>; rel=\"next\""));
        let (reader, handle) = server(vec![(
            200,
            json!([article("DECOY001", "Deep X Learning")]).to_string(),
            only,
        )]);
        let reader = reader.with_limits(limits(1, 1));
        let error = reader
            .search(&DesktopSearchQuery::default())
            .await
            .expect_err("incomplete");
        assert_eq!(error.payload().code, "desktop-search-incomplete");
        handle.join().expect("join");

        let mut hidden = headers("4");
        hidden.push(("Total-Results", "2"));
        let (reader, handle) = server(vec![(
            200,
            json!([article("ONLY0001", "Only")]).to_string(),
            hidden,
        )]);
        let error = reader
            .search(&DesktopSearchQuery::default())
            .await
            .expect_err("declared total was not collected");
        assert_eq!(error.payload().code, "desktop-search-incomplete");
        handle.join().expect("join");

        let mut short = headers("4");
        short.push(("Total-Results", "3"));
        let (reader, handle) = server(vec![(200, "[]".to_string(), short)]);
        let error = reader
            .search(&DesktopSearchQuery::default())
            .await
            .expect_err("short page under-declares");
        assert_eq!(error.payload().code, "desktop-search-incomplete");
        handle.join().expect("join");
    }

    #[tokio::test]
    async fn version_change_restarts_the_walk_instead_of_mixing_pages() {
        let first = article("AAAA0001", "Alpha");
        let second = article("BBBB0002", "Beta");
        let stable_a = article("AAAA0001", "Alpha");
        let stable_b = article("BBBB0002", "Beta");
        let mut page_one = headers("1");
        page_one.push(("Link", "</api/users/0/items?limit=1&start=1>; rel=\"next\""));
        page_one.push(("Total-Results", "2"));
        let mut retry_one = headers("5");
        retry_one.push(("Link", "</api/users/0/items?limit=1&start=1>; rel=\"next\""));
        retry_one.push(("Total-Results", "2"));
        let (reader, handle) = server(vec![
            (200, json!([first]).to_string(), page_one),
            (200, json!([second]).to_string(), headers("2")),
            (200, json!([stable_a]).to_string(), retry_one),
            (200, json!([stable_b]).to_string(), headers("5")),
        ]);
        let found = reader
            .search(&DesktopSearchQuery::default())
            .await
            .expect("stable");
        let keys = found
            .items
            .iter()
            .map(|item| item.key.as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(keys, BTreeSet::from(["AAAA0001", "BBBB0002"]));
        assert_eq!(handle.join().expect("join").len(), 4);
    }

    #[tokio::test]
    async fn file_redirect_and_missing_endpoint_stop() {
        let item = article("ATTN001", "Attention");
        let children = json!([
            {"key": "ATCH005", "data": {"key": "ATCH005", "itemType": "attachment", "parentItem": "ATTN001", "contentType": "application/pdf", "path": "storage:attention.pdf"}}
        ]);
        let (reader, handle) = server(vec![
            (200, item.to_string(), headers("7")),
            (200, children.to_string(), headers("7")),
            (
                302,
                "https://example.invalid/secret.pdf".to_string(),
                vec![("Location", "https://example.invalid/secret.pdf")],
            ),
        ]);
        let error = reader
            .local_pdf_path("ATTN001")
            .await
            .expect_err("redirect");
        assert_eq!(error.payload().code, "local-api-redirect");
        assert_eq!(handle.join().expect("join").len(), 3);

        let (reader, handle) = server(vec![
            (
                200,
                article("ATTN001", "Attention").to_string(),
                headers("7"),
            ),
            (200, children.to_string(), headers("7")),
            (405, String::new(), headers("7")),
        ]);
        let error = reader
            .local_pdf_path("ATTN001")
            .await
            .expect_err("endpoint");
        assert_eq!(error.payload().code, "desktop-file-url-unsupported");
        handle.join().expect("join");
    }
}
