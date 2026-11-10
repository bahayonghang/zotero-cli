//! Read-only Local API mapping onto the existing item DTOs.
//!
//! GETs are built by [`crate::local_api::LocalApiClient::read_exchange`]. This module does not
//! authorize, send a write key, or follow redirects. Fields that have no DTO home are omitted.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};
use zot_core::{
    Attachment, ChildAnnotation, ChildAttachment, ChildItem, ChildNote, CitationKeyMatch,
    Collection, Creator, Item, LibraryScope, LibraryStats, Note, SearchItemDirection,
    SearchItemSort, SearchResult, TagSummary, ZotError, ZotResult, extra_line_has_citation_key,
    item_matches_creator_filter, item_matches_field_filters, item_matches_metadata_query,
    item_matches_tag_filter, item_matches_year_filter, sort_search_items,
};

use crate::local_api::{LocalApiClient, error, next_start, prefix};

const CHILD_TYPES: &[&str] = &["attachment", "note", "annotation"];
const COMPLETE_PAGE_SIZE: u64 = 100;
const COMPLETE_MAX_PAGES: u64 = 100;
const COMPLETE_MAX_ROWS: u64 = 10_000;
const COMPLETE_VERSION_ATTEMPTS: u32 = 3;
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

enum CompleteMiss {
    Http,
    Collection(String),
}

struct FlatCollection {
    key: String,
    name: String,
    parent: Option<String>,
}

struct InventoryRow {
    key: String,
    item_type: String,
    deleted: bool,
    parent: Option<String>,
    content_type: String,
    tags: Vec<String>,
    collections: Vec<String>,
    item: Item,
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

    pub async fn collection_list(&self) -> ZotResult<Vec<Collection>> {
        for _ in 0..COMPLETE_VERSION_ATTEMPTS {
            let mut seen = None;
            match self
                .fetch_complete(
                    |start| complete_collections_path(&self.scope, start),
                    &mut seen,
                    CompleteMiss::Http,
                )
                .await?
            {
                Attempt::Ready(rows) => return Ok(assemble_collections(&rows)),
                Attempt::Inconsistent => {}
            }
        }
        Err(inconsistent())
    }

    pub async fn collection_get(&self, key: &str) -> ZotResult<Collection> {
        // A single collection response reports the object version. The list
        // reports the library version. Read the node from the list only.
        let tree = self.collection_list().await?;
        find_collection(&tree, key).ok_or_else(|| collection_missing(key, false))
    }

    pub async fn collection_subcollections(&self, key: &str) -> ZotResult<Vec<Collection>> {
        let tree = self.collection_list().await?;
        find_children(&tree, key).ok_or_else(|| collection_missing(key, false))
    }

    pub async fn collection_search(&self, query: &str, limit: usize) -> ZotResult<Vec<Collection>> {
        let query_lc = query.to_lowercase();
        let mut flat = Vec::new();
        for collection in self.collection_list().await? {
            flatten_collection(&collection, &mut flat);
        }
        Ok(flat
            .into_iter()
            .filter(|collection| collection.name.to_lowercase().contains(&query_lc))
            .take(limit)
            .collect())
    }

    pub async fn collection_items(&self, key: &str) -> ZotResult<Vec<Item>> {
        self.collection_members(key).await
    }

    pub async fn collection_item_count(&self, key: &str) -> ZotResult<usize> {
        Ok(self.collection_members(key).await?.len())
    }

    pub async fn collection_tags(&self, key: &str) -> ZotResult<Vec<TagSummary>> {
        Ok(summarize_tags(&self.collection_members(key).await?))
    }

    pub async fn library_list(
        &self,
        collection: Option<&str>,
        limit: usize,
        offset: usize,
        exclude_trashed: bool,
    ) -> ZotResult<SearchResult> {
        for _ in 0..COMPLETE_VERSION_ATTEMPTS {
            let mut seen = None;
            let collection_key = if let Some(name) = collection {
                match self.resolve_listed_collection(name, &mut seen).await? {
                    Attempt::Ready(key) => Some(key),
                    Attempt::Inconsistent => continue,
                }
            } else {
                None
            };
            let rows = match self
                .fetch_complete(
                    |start| {
                        bibliographic_items_path(
                            &self.scope,
                            start,
                            !exclude_trashed,
                            collection_key.as_deref(),
                        )
                    },
                    &mut seen,
                    CompleteMiss::Http,
                )
                .await?
            {
                Attempt::Ready(rows) => rows,
                Attempt::Inconsistent => continue,
            };
            return Ok(finish_library_list(
                rows,
                collection_key.as_deref(),
                limit,
                offset,
                exclude_trashed,
            ));
        }
        Err(inconsistent())
    }

    pub async fn library_stats(&self, include_trashed: bool) -> ZotResult<LibraryStats> {
        Ok(aggregate_stats(
            &self.inventory(true).await?,
            include_trashed,
        ))
    }

    pub async fn library_tags(&self) -> ZotResult<Vec<TagSummary>> {
        Ok(aggregate_all_tags(&self.inventory(true).await?))
    }

    async fn inventory(&self, include_trashed: bool) -> ZotResult<Vec<InventoryRow>> {
        for _ in 0..COMPLETE_VERSION_ATTEMPTS {
            let mut seen = None;
            match self
                .fetch_complete(
                    |start| complete_items_path(&self.scope, start, include_trashed),
                    &mut seen,
                    CompleteMiss::Http,
                )
                .await?
            {
                Attempt::Ready(rows) => {
                    return Ok(rows.iter().filter_map(map_inventory).collect());
                }
                Attempt::Inconsistent => {}
            }
        }
        Err(inconsistent())
    }

    async fn collection_members(&self, name: &str) -> ZotResult<Vec<Item>> {
        for _ in 0..COMPLETE_VERSION_ATTEMPTS {
            let mut seen = None;
            let key = match self.resolve_listed_collection(name, &mut seen).await? {
                Attempt::Ready(key) => key,
                Attempt::Inconsistent => continue,
            };
            // 10.0.8 drops the collection condition when the key is unknown, so
            // this URL is requested only after the list confirms the key.
            let missing = CompleteMiss::Collection(key.clone());
            match self
                .fetch_complete(
                    |start| collection_top_path(&self.scope, &key, start),
                    &mut seen,
                    missing,
                )
                .await?
            {
                Attempt::Ready(rows) => {
                    return Ok(rows.iter().filter_map(map_member).collect());
                }
                Attempt::Inconsistent => {}
            }
        }
        Err(inconsistent())
    }

    async fn resolve_listed_collection(
        &self,
        name: &str,
        seen: &mut Option<u64>,
    ) -> ZotResult<Attempt<String>> {
        let rows = match self
            .fetch_complete(
                |start| complete_collections_path(&self.scope, start),
                seen,
                CompleteMiss::Http,
            )
            .await?
        {
            Attempt::Inconsistent => return Ok(Attempt::Inconsistent),
            Attempt::Ready(rows) => rows,
        };
        Ok(Attempt::Ready(resolve_named_collection(name, &rows)?))
    }

    async fn fetch_complete<F>(
        &self,
        mut path_at: F,
        seen: &mut Option<u64>,
        missing: CompleteMiss,
    ) -> ZotResult<Attempt<Vec<Value>>>
    where
        F: FnMut(u64) -> String,
    {
        let mut rows = Vec::new();
        let mut start = 0_u64;
        let mut declared_total: Option<u64> = None;
        for _page in 0..COMPLETE_MAX_PAGES {
            let exchange = self
                .client
                .read_exchange(&path_at(start), self.server())
                .await?;
            if !exchange.status.is_success() {
                if rows.is_empty() && exchange.status.as_u16() == 404 {
                    if let CompleteMiss::Collection(key) = missing {
                        return Err(collection_missing(&key, true));
                    }
                }
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
                if total > COMPLETE_MAX_ROWS {
                    return Err(read_incomplete());
                }
                if declared_total.is_some_and(|previous| previous != total) {
                    return Ok(Attempt::Inconsistent);
                }
                declared_total = Some(total);
            }
            let page_rows = parse_array(&exchange.body)?;
            let count = u64::try_from(page_rows.len()).unwrap_or(u64::MAX);
            let next_len = rows.len().saturating_add(page_rows.len());
            if u64::try_from(next_len).unwrap_or(u64::MAX) > COMPLETE_MAX_ROWS {
                return Err(read_incomplete());
            }
            rows.extend(page_rows);
            let collected = u64::try_from(rows.len()).unwrap_or(u64::MAX);
            if count < COMPLETE_PAGE_SIZE {
                if declared_total.is_some_and(|total| total > collected) {
                    return Err(read_incomplete());
                }
                return Ok(Attempt::Ready(rows));
            }
            match next_start(exchange.link.as_deref(), start)? {
                Some(next) => {
                    if collected >= COMPLETE_MAX_ROWS {
                        return Err(read_incomplete());
                    }
                    start = next;
                }
                None => {
                    if declared_total.is_some_and(|total| total > collected) {
                        return Err(read_incomplete());
                    }
                    return Ok(Attempt::Ready(rows));
                }
            }
        }
        Err(read_incomplete())
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

fn complete_items_path(scope: &LibraryScope, start: u64, include_trashed: bool) -> String {
    items_path(scope, start, COMPLETE_PAGE_SIZE, include_trashed)
}

fn bibliographic_items_path(
    scope: &LibraryScope,
    start: u64,
    include_trashed: bool,
    collection: Option<&str>,
) -> String {
    let root = match collection {
        Some(key) => format!("{}/collections/{key}/items/top", prefix(scope)),
        None => format!("{}/items/top", prefix(scope)),
    };
    let mut path = format!(
        "{root}?limit={COMPLETE_PAGE_SIZE}&start={start}&itemType=-attachment&itemType=-note&itemType=-annotation"
    );
    if include_trashed {
        path.push_str("&includeTrashed=1");
    }
    path
}

fn complete_collections_path(scope: &LibraryScope, start: u64) -> String {
    format!(
        "{}/collections?limit={COMPLETE_PAGE_SIZE}&start={start}",
        prefix(scope)
    )
}

fn collection_top_path(scope: &LibraryScope, key: &str, start: u64) -> String {
    format!(
        "{}/collections/{key}/items/top?includeTrashed=1&limit={COMPLETE_PAGE_SIZE}&start={start}",
        prefix(scope)
    )
}

fn finish_library_list(
    rows: Vec<Value>,
    collection: Option<&str>,
    limit: usize,
    offset: usize,
    exclude_trashed: bool,
) -> SearchResult {
    let mut items = rows
        .iter()
        .filter_map(map_inventory)
        .filter(|row| is_bibliographic(&row.item_type))
        .filter(|row| !exclude_trashed || !row.deleted)
        .filter(|row| {
            collection.is_none_or(|key| row.collections.iter().any(|member| member == key))
        })
        .map(|row| row.item)
        .collect::<Vec<_>>();
    items.sort_by(|left, right| right.key.cmp(&left.key));
    let total = items.len();
    let items = items.into_iter().skip(offset).take(limit).collect();
    SearchResult {
        items,
        total,
        query: String::new(),
        fulltext_index: None,
    }
}

fn resolve_named_collection(name: &str, rows: &[Value]) -> ZotResult<String> {
    let flat = rows
        .iter()
        .filter_map(map_flat_collection)
        .collect::<Vec<_>>();
    if flat.iter().any(|row| row.key == name) {
        return Ok(name.to_string());
    }
    let mut named = flat
        .iter()
        .filter(|row| row.name == name)
        .map(|row| row.key.clone())
        .collect::<Vec<_>>();
    named.sort();
    named.dedup();
    match named.as_slice() {
        [key] => Ok(key.clone()),
        [] => Err(collection_missing(name, true)),
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

fn assemble_collections(rows: &[Value]) -> Vec<Collection> {
    let mut nodes = BTreeMap::<String, Collection>::new();
    let mut parent_of = BTreeMap::<String, Option<String>>::new();
    for row in rows.iter().filter_map(map_flat_collection) {
        if nodes.contains_key(&row.key) {
            continue;
        }
        parent_of.insert(row.key.clone(), row.parent.clone());
        nodes.insert(
            row.key.clone(),
            Collection {
                key: row.key,
                name: row.name,
                parent_key: None,
                children: Vec::new(),
            },
        );
    }
    let mut children_of = BTreeMap::<String, Vec<String>>::new();
    for (key, parent) in &parent_of {
        let Some(parent_key) = parent else {
            continue;
        };
        if !nodes.contains_key(parent_key) {
            continue;
        }
        if let Some(node) = nodes.get_mut(key) {
            node.parent_key = Some(parent_key.clone());
        }
        children_of
            .entry(parent_key.clone())
            .or_default()
            .push(key.clone());
    }
    for children in children_of.values_mut() {
        children.sort();
    }
    parent_of
        .iter()
        .filter(|(_, parent)| parent.is_none())
        .filter_map(|(key, _)| build_collection(key, &nodes, &children_of, &mut BTreeSet::new()))
        .collect()
}

fn build_collection(
    key: &str,
    nodes: &BTreeMap<String, Collection>,
    children_of: &BTreeMap<String, Vec<String>>,
    stack: &mut BTreeSet<String>,
) -> Option<Collection> {
    if !stack.insert(key.to_string()) {
        return None;
    }
    let mut node = nodes.get(key)?.clone();
    if let Some(children) = children_of.get(key) {
        node.children = children
            .iter()
            .filter_map(|child| build_collection(child, nodes, children_of, stack))
            .collect();
    }
    stack.remove(key);
    Some(node)
}

fn flatten_collection(collection: &Collection, out: &mut Vec<Collection>) {
    out.push(collection.clone());
    for child in &collection.children {
        flatten_collection(child, out);
    }
}

fn find_collection(collections: &[Collection], key: &str) -> Option<Collection> {
    for collection in collections {
        if collection.key == key {
            return Some(collection.clone());
        }
        if let Some(found) = find_collection(&collection.children, key) {
            return Some(found);
        }
    }
    None
}

fn find_children(collections: &[Collection], key: &str) -> Option<Vec<Collection>> {
    for collection in collections {
        if collection.key == key {
            return Some(collection.children.clone());
        }
        if let Some(found) = find_children(&collection.children, key) {
            return Some(found);
        }
    }
    None
}

fn map_flat_collection(value: &Value) -> Option<FlatCollection> {
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
    let parent = match data.get("parentCollection") {
        Some(Value::String(parent)) if !parent.is_empty() => Some(parent.clone()),
        _ => None,
    };
    Some(FlatCollection { key, name, parent })
}

fn map_member(value: &Value) -> Option<Item> {
    let data = data_map(value)?;
    if data.get("itemType").and_then(Value::as_str) == Some("annotation") {
        return None;
    }
    map_primary_item(value)
}

fn map_inventory(value: &Value) -> Option<InventoryRow> {
    let item = map_primary_item(value)?;
    let data = data_map(value)?;
    Some(InventoryRow {
        key: item.key.clone(),
        item_type: item.item_type.clone(),
        deleted: is_deleted(data),
        parent: parent_item_key(data),
        content_type: data
            .get("contentType")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        tags: item.tags.clone(),
        collections: item.collections.clone(),
        item,
    })
}

fn parent_item_key(data: &Map<String, Value>) -> Option<String> {
    match data.get("parentItem") {
        Some(Value::String(key)) if !key.is_empty() => Some(key.clone()),
        _ => None,
    }
}

fn is_bibliographic(item_type: &str) -> bool {
    !CHILD_TYPES.contains(&item_type)
}

fn summarize_tags(items: &[Item]) -> Vec<TagSummary> {
    let mut counts = BTreeMap::<String, usize>::new();
    for item in items {
        for tag in &item.tags {
            *counts.entry(tag.clone()).or_default() += 1;
        }
    }
    sort_tag_counts(counts)
}

fn aggregate_all_tags(rows: &[InventoryRow]) -> Vec<TagSummary> {
    let mut counts = BTreeMap::<String, usize>::new();
    for row in rows {
        for tag in &row.tags {
            *counts.entry(tag.clone()).or_default() += 1;
        }
    }
    sort_tag_counts(counts)
}

fn sort_tag_counts(counts: BTreeMap<String, usize>) -> Vec<TagSummary> {
    let mut tags = counts
        .into_iter()
        .map(|(name, count)| TagSummary { name, count })
        .collect::<Vec<_>>();
    tags.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.name.cmp(&right.name))
    });
    tags
}

fn aggregate_stats(rows: &[InventoryRow], include_trashed: bool) -> LibraryStats {
    let mut by_key = BTreeMap::<&str, &InventoryRow>::new();
    for row in rows {
        by_key.insert(row.key.as_str(), row);
    }
    let parent_visible = |key: &str| {
        by_key.get(key).is_some_and(|parent| {
            is_bibliographic(&parent.item_type) && (include_trashed || !parent.deleted)
        })
    };
    let mut total_items = 0;
    let mut by_type = BTreeMap::<String, usize>::new();
    let mut top_tags = BTreeMap::<String, usize>::new();
    let mut collections = BTreeMap::<String, usize>::new();
    for row in rows {
        if !is_bibliographic(&row.item_type) || (!include_trashed && row.deleted) {
            continue;
        }
        total_items += 1;
        *by_type.entry(row.item_type.clone()).or_default() += 1;
        for tag in &row.tags {
            *top_tags.entry(tag.clone()).or_default() += 1;
        }
        for key in &row.collections {
            *collections.entry(key.clone()).or_default() += 1;
        }
    }
    let mut pdf_parents = BTreeSet::new();
    let mut notes = 0_usize;
    for row in rows {
        let Some(parent) = row.parent.as_deref() else {
            continue;
        };
        if !parent_visible(parent) {
            continue;
        }
        if row.item_type == "attachment" && row.content_type == "application/pdf" {
            pdf_parents.insert(parent.to_string());
        } else if row.item_type == "note" {
            notes += 1;
        }
    }
    LibraryStats {
        total_items,
        by_type,
        top_tags,
        collections,
        pdf_attachments: pdf_parents.len(),
        notes,
    }
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
        date_added: optional_string(data, "dateAdded").map(|value| snapshot_timestamp(&value)),
        date_modified: optional_string(data, "dateModified")
            .map(|value| snapshot_timestamp(&value)),
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

/// Zotero 10.0.8 `sqlToISO8601` inserts `T` and `Z` without shifting the clock.
/// Snapshot rows keep the SQLite text, so reverse that spelling here.
fn snapshot_timestamp(value: &str) -> String {
    let Some((date, zoned)) = value.split_once('T') else {
        return value.to_string();
    };
    if !is_calendar_date(date) {
        return value.to_string();
    }
    let Some(time) = zoned.strip_suffix('Z') else {
        return value.to_string();
    };
    let time = time
        .split_once('.')
        .map(|(seconds, _)| seconds)
        .unwrap_or(time);
    if is_clock_time(time) {
        format!("{date} {time}")
    } else {
        value.to_string()
    }
}

fn is_calendar_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| matches!(index, 4 | 7) || byte.is_ascii_digit())
}

fn is_clock_time(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 8
        && bytes[2] == b':'
        && bytes[5] == b':'
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| matches!(index, 2 | 5) || byte.is_ascii_digit())
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

fn read_incomplete() -> ZotError {
    ZotError::LocalApi {
        code: "desktop-read-incomplete".to_string(),
        message: "The complete read is larger than 10000 rows".to_string(),
        hint: Some("No partial list is returned.".to_string()),
        status: None,
    }
}

fn collection_missing(key: &str, names: bool) -> ZotError {
    invalid(
        "collection-not-found",
        format!("Collection '{key}' not found"),
        Some(
            if names {
                "Use 'zot collection list' to inspect collection names"
            } else {
                "Use 'zot collection list' to inspect collection keys"
            }
            .to_string(),
        ),
    )
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
        serve_with(
            responses,
            limits(1, 5),
            Some("server-abc".to_string()),
            LibraryScope::User,
        )
    }

    fn owned_server(
        responses: Vec<Reply>,
    ) -> (DesktopRead, std::thread::JoinHandle<Vec<CapturedHit>>) {
        serve_with(
            responses,
            limits(1, 1),
            Some("server-abc".to_string()),
            LibraryScope::User,
        )
    }

    fn serve_with(
        responses: Vec<Reply>,
        scan: DesktopScanLimits,
        server_id: Option<String>,
        scope: LibraryScope,
    ) -> (DesktopRead, std::thread::JoinHandle<Vec<CapturedHit>>) {
        let http = Server::http("127.0.0.1:0").expect("server");
        let client = LocalApiClient::with_base_url_for_tests(
            &format!("http://{}/", http.server_addr()),
            Duration::from_secs(2),
        )
        .expect("client");
        let reader = DesktopRead::new(client, server_id, scope).with_limits(scan);
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

    fn serve_routed<F>(reply: F) -> (DesktopRead, std::thread::JoinHandle<Vec<CapturedHit>>)
    where
        F: Fn(&str) -> (u16, String, Vec<(String, String)>) + Send + 'static,
    {
        let http = Server::http("127.0.0.1:0").expect("server");
        let client = LocalApiClient::with_base_url_for_tests(
            &format!("http://{}/", http.server_addr()),
            Duration::from_secs(2),
        )
        .expect("client");
        let reader = DesktopRead::new(client, Some("server-abc".to_string()), LibraryScope::User)
            .with_limits(limits(1, 1));
        let handle = std::thread::spawn(move || {
            let mut captured = Vec::new();
            loop {
                let Ok(Some(request)) = http.recv_timeout(Duration::from_millis(500)) else {
                    break;
                };
                let url = request.url().to_string();
                captured.push((
                    request.method().to_string(),
                    url.clone(),
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
                let (status, body, headers) = reply(&url);
                let mut response = FakeResponse::from_string(body).with_status_code(status);
                for (name, value) in headers {
                    response.add_header(
                        Header::from_bytes(name.as_bytes(), value.as_bytes()).expect("header"),
                    );
                }
                request.respond(response).expect("respond");
            }
            captured
        });
        (reader, handle)
    }

    fn owned_headers(
        version: &str,
        total: Option<&str>,
        next: Option<String>,
    ) -> Vec<(String, String)> {
        let mut headers = vec![
            ("Zotero-API-Version".to_string(), "3".to_string()),
            ("Zotero-Server-ID".to_string(), "server-abc".to_string()),
            ("Last-Modified-Version".to_string(), version.to_string()),
        ];
        if let Some(total) = total {
            headers.push(("Total-Results".to_string(), total.to_string()));
        }
        if let Some(next) = next {
            headers.push(("Link".to_string(), next));
        }
        headers
    }

    fn query_start(url: &str) -> u64 {
        url.split("start=")
            .nth(1)
            .and_then(|value| value.split(['&', '#']).next())
            .and_then(|value| value.parse().ok())
            .unwrap_or(0)
    }

    fn page_headers(
        total: &'static str,
        next: Option<&'static str>,
    ) -> Vec<(&'static str, &'static str)> {
        let mut page = headers("4");
        page.push(("Total-Results", total));
        if let Some(next) = next {
            page.push(("Link", next));
        }
        page
    }

    fn bibliographic_page_at(start: usize, count: usize) -> String {
        let mut body = String::from("[");
        for offset in 0..count {
            if offset > 0 {
                body.push(',');
            }
            let key = format!("K{:07}", start + offset);
            body.push_str(&format!(
                r#"{{"key":"{key}","data":{{"key":"{key}","itemType":"journalArticle","title":"{key}","tags":[],"collections":[],"dateAdded":"2024-01-01T12:30:45Z","dateModified":"2024-01-02T03:04:05Z"}}}}"#
            ));
        }
        body.push(']');
        body
    }

    fn bibliographic_page(count: usize, prefix: &str) -> String {
        let mut body = String::from("[");
        for index in 0..count {
            if index > 0 {
                body.push(',');
            }
            let key = format!("{prefix}{index:07}");
            body.push_str(&format!(
                r#"{{"key":"{key}","data":{{"key":"{key}","itemType":"journalArticle","title":"{key}","tags":[],"collections":[]}}}}"#
            ));
        }
        body.push(']');
        body
    }

    fn assert_no_total(error: &ZotError) {
        let payload = serde_json::to_value(error.payload()).expect("payload");
        assert!(payload.get("total").is_none());
        assert!(!error.payload().message.contains("10001"));
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
        assert_eq!(item.date_added.as_deref(), Some("2024-01-01"));
    }

    #[test]
    fn iso_timestamps_use_the_snapshot_spelling() {
        let mut value = article("TIME0030", "Timed");
        let data = value
            .get_mut("data")
            .and_then(Value::as_object_mut)
            .expect("data");
        data.insert("dateAdded".to_string(), json!("2024-01-01T12:30:45Z"));
        data.insert(
            "dateModified".to_string(),
            json!("2024-01-02T03:04:05.000Z"),
        );
        let item = map_primary_item(&value).expect("item");
        assert_eq!(item.date_added.as_deref(), Some("2024-01-01 12:30:45"));
        assert_eq!(item.date_modified.as_deref(), Some("2024-01-02 03:04:05"));
        assert_eq!(item.date.as_deref(), Some("2017"));
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

    #[test]
    fn scan_limits_keep_the_search_page_cap() {
        assert_eq!(DesktopScanLimits::default().max_pages, 20);
        assert_eq!(DesktopScanLimits::default().page_size, 100);
    }

    #[tokio::test]
    async fn complete_read_pages_with_limit_100_even_when_scan_max_pages_is_one() {
        let first = bibliographic_page(100, "A");
        let second = bibliographic_page(1, "Z");
        let (reader, handle) = owned_server(vec![
            (
                200,
                first,
                page_headers(
                    "101",
                    Some("</api/users/0/items?limit=100&start=100>; rel=\"next\""),
                ),
            ),
            (200, second, page_headers("101", None)),
        ]);
        let listed = reader
            .library_list(None, 101, 0, true)
            .await
            .expect("two pages");
        assert_eq!(listed.total, 101);
        assert_eq!(listed.items[0].key, "Z0000000");
        let captured = handle.join().expect("join");
        assert_eq!(captured.len(), 2);
        assert!(captured.iter().all(|(method, url, _)| {
            method == "GET"
                && url.contains("/items/top?")
                && url.contains("itemType=-attachment")
                && url.contains("itemType=-note")
                && url.contains("itemType=-annotation")
                && url.contains("limit=100")
                && !url.contains("sort=")
        }));
        assert!(captured[0].1.contains("start=0"));
        assert!(captured[1].1.contains("start=100"));
        assert!(
            captured
                .iter()
                .all(|(_, url, _)| !url.contains("includeTrashed"))
        );
    }

    #[tokio::test]
    async fn complete_read_accepts_exactly_10000_rows() {
        let (reader, handle) = owned_server(vec![(
            200,
            bibliographic_page(10_000, "K"),
            page_headers("10000", None),
        )]);
        let stats = reader.library_stats(false).await.expect("full page");
        assert_eq!(stats.total_items, 10_000);
        assert_eq!(handle.join().expect("join").len(), 1);
    }

    #[tokio::test]
    async fn complete_read_rejects_10001_without_a_total_or_a_partial_list() {
        let (reader, handle) =
            owned_server(vec![(200, "[]".to_string(), page_headers("10001", None))]);
        let error = reader.library_stats(false).await.expect_err("over cap");
        assert_eq!(error.payload().code, "desktop-read-incomplete");
        assert_no_total(&error);
        assert_eq!(handle.join().expect("join").len(), 1);
    }

    #[tokio::test]
    async fn complete_read_rejects_a_next_link_after_10000_rows() {
        let (reader, handle) = owned_server(vec![(
            200,
            bibliographic_page(10_000, "K"),
            page_headers(
                "10000",
                Some("</api/users/0/items?limit=100&start=10000>; rel=\"next\""),
            ),
        )]);
        let error = reader.library_tags().await.expect_err("next past cap");
        assert_eq!(error.payload().code, "desktop-read-incomplete");
        assert_no_total(&error);
        assert_eq!(handle.join().expect("join").len(), 1);
    }

    #[tokio::test]
    async fn complete_read_rejects_a_short_page_below_its_declared_total() {
        let (reader, handle) = owned_server(vec![(
            200,
            bibliographic_page(1, "S"),
            page_headers("5", None),
        )]);
        let error = reader
            .library_list(None, 10, 0, true)
            .await
            .expect_err("short");
        assert_eq!(error.payload().code, "desktop-read-incomplete");
        assert_no_total(&error);
        handle.join().expect("join");
    }

    #[tokio::test]
    async fn complete_read_retries_version_drift_then_reports_inconsistent() {
        let full = bibliographic_page(100, "A");
        let tail = bibliographic_page(1, "B");
        let mut responses = Vec::new();
        for (first, second) in [("1", "2"), ("3", "4"), ("5", "6")] {
            let mut page = headers(first);
            page.push(("Total-Results", "101"));
            page.push((
                "Link",
                "</api/users/0/items?limit=100&start=100>; rel=\"next\"",
            ));
            responses.push((200, full.clone(), page));
            responses.push((200, tail.clone(), headers(second)));
        }
        let (reader, handle) = owned_server(responses);
        let error = reader.library_tags().await.expect_err("drift");
        assert_eq!(error.payload().code, "desktop-read-inconsistent");
        assert_ne!(error.payload().code, "local-api-conflict");
        assert_eq!(handle.join().expect("join").len(), 6);
    }

    #[tokio::test]
    async fn group_inventory_uses_the_group_prefix() {
        let (reader, handle) = serve_with(
            vec![(200, bibliographic_page(1, "G"), page_headers("1", None))],
            limits(1, 1),
            Some("server-abc".to_string()),
            LibraryScope::Group { group_id: 42 },
        );
        let tags = reader.library_tags().await.expect("group tags");
        assert!(tags.is_empty());
        let captured = handle.join().expect("join");
        assert!(captured[0].1.contains("/api/groups/42/"));
        assert!(captured[0].1.contains("limit=100"));
    }

    #[tokio::test]
    async fn unbound_inventory_omits_the_server_id_header() {
        let (reader, handle) = serve_with(
            vec![(200, bibliographic_page(1, "U"), page_headers("1", None))],
            limits(1, 1),
            None,
            LibraryScope::User,
        );
        reader.library_stats(false).await.expect("unbound");
        let captured = handle.join().expect("join");
        assert!(captured.iter().all(|(_, _, headers)| {
            headers
                .iter()
                .all(|(name, _)| name != "zotero-server-id" && name != "zotero-api-key")
        }));
    }

    #[tokio::test]
    async fn collection_members_come_from_items_top_and_drop_annotations() {
        let body = json!([
            {"key":"ART00030","data":{"key":"ART00030","itemType":"journalArticle","title":"Paper","tags":[{"tag":"paper"}],"collections":["COLMEM01"]}},
            {"key":"ATT00032","data":{"key":"ATT00032","itemType":"attachment","title":"Loose","contentType":"application/pdf","tags":[],"collections":["COLMEM01"]}},
            {"key":"ANN00033","data":{"key":"ANN00033","itemType":"annotation","title":"","parentItem":"ATT00032","tags":[{"tag":"note-only"}],"collections":["COLMEM01"]}}
        ])
        .to_string();
        let listed = json!([
            {"key":"COLMEM01","data":{"key":"COLMEM01","name":"Members","parentCollection":false}}
        ])
        .to_string();
        let (reader, handle) = owned_server(vec![
            (200, listed.clone(), page_headers("1", None)),
            (200, body.clone(), page_headers("3", None)),
            (200, listed.clone(), page_headers("1", None)),
            (200, body.clone(), page_headers("3", None)),
            (200, listed, page_headers("1", None)),
            (200, body, page_headers("3", None)),
        ]);
        let items = reader.collection_items("COLMEM01").await.expect("items");
        assert_eq!(
            items
                .iter()
                .map(|item| item.key.as_str())
                .collect::<Vec<_>>(),
            vec!["ART00030", "ATT00032"]
        );
        assert_eq!(
            reader
                .collection_item_count("COLMEM01")
                .await
                .expect("count"),
            2
        );
        let tags = reader.collection_tags("COLMEM01").await.expect("tags");
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].name, "paper");
        assert_eq!(tags[0].count, 1);
        let captured = handle.join().expect("join");
        let member_urls = captured
            .iter()
            .filter(|(_, url, _)| url.contains("items/top"))
            .map(|(_, url, _)| url.as_str())
            .collect::<Vec<_>>();
        assert_eq!(member_urls.len(), 3);
        assert!(member_urls.iter().all(|url| {
            url.contains("/collections/COLMEM01/items/top")
                && url.contains("includeTrashed=1")
                && url.contains("limit=100")
                && !url.contains("/tags")
        }));
    }

    #[tokio::test]
    async fn library_list_sorts_filtered_keys_locally_and_never_sends_sort() {
        let body = json!([
            {"key":"AAAA1111","data":{"key":"AAAA1111","itemType":"journalArticle","title":"New","dateModified":"2024-12-01","tags":[],"collections":[]}},
            {"key":"ZZZZ9999","data":{"key":"ZZZZ9999","itemType":"journalArticle","title":"Old","dateModified":"2010-01-01","tags":[],"collections":[]}},
            {"key":"ATT00032","data":{"key":"ATT00032","itemType":"attachment","title":"","tags":[],"collections":[]}},
            {"key":"NOTE0031","data":{"key":"NOTE0031","itemType":"note","title":"","parentItem":"AAAA1111","tags":[],"collections":[]}},
            {"key":"ANN00033","data":{"key":"ANN00033","itemType":"annotation","title":"","tags":[],"collections":[]}},
            {"key":"TRSH0001","data":{"key":"TRSH0001","itemType":"journalArticle","title":"Gone","deleted":true,"tags":[],"collections":[]}}
        ])
        .to_string();
        let (reader, handle) = owned_server(vec![
            (200, body.clone(), page_headers("6", None)),
            (200, body, page_headers("6", None)),
        ]);
        let page = reader.library_list(None, 1, 0, true).await.expect("page");
        assert_eq!(page.total, 2);
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].key, "ZZZZ9999");
        let with_trash = reader
            .library_list(None, 10, 0, false)
            .await
            .expect("trash");
        assert_eq!(
            with_trash
                .items
                .iter()
                .map(|item| item.key.as_str())
                .collect::<Vec<_>>(),
            vec!["ZZZZ9999", "TRSH0001", "AAAA1111"]
        );
        let captured = handle.join().expect("join");
        assert!(!captured[0].1.contains("includeTrashed"));
        assert!(captured[1].1.contains("includeTrashed=1"));
        assert!(captured.iter().all(|(_, url, _)| {
            url.contains("/items/top?")
                && url.contains("itemType=-attachment")
                && !url.contains("sort=")
        }));
    }

    #[tokio::test]
    async fn library_list_resolves_collection_keys_names_and_ambiguous_names() {
        let shared = json!([
            {"key":"ZZZZZZZZ","data":{"key":"ZZZZZZZZ","name":"Shared","parentCollection":false}},
            {"key":"AAAAAAAA","data":{"key":"AAAAAAAA","name":"Shared","parentCollection":false}}
        ])
        .to_string();
        let (reader, handle) = owned_server(vec![(200, shared, page_headers("2", None))]);
        let error = reader
            .library_list(Some("Shared"), 10, 0, true)
            .await
            .expect_err("ambiguous");
        assert_eq!(error.payload().code, "collection-ambiguous");
        assert_eq!(
            error.payload().hint.as_deref(),
            Some("Use one of these collection keys: AAAAAAAA, ZZZZZZZZ")
        );
        handle.join().expect("join");

        let named = json!([
            {"key":"AAAAAAAA","data":{"key":"AAAAAAAA","name":"Only","parentCollection":false}},
            {"key":"BBBBBBBB","data":{"key":"BBBBBBBB","name":"AAAAAAAA","parentCollection":false}},
            {"key":"short","data":{"key":"short","name":"Tiny","parentCollection":false}},
            {"key":"ORPHAN01","data":{"key":"ORPHAN01","name":"Loose","parentCollection":"MISSING1"}}
        ])
        .to_string();
        let items = json!([
            {"key":"KEYAAAAA","data":{"key":"KEYAAAAA","itemType":"journalArticle","title":"A","tags":[],"collections":["AAAAAAAA"]}},
            {"key":"KEYBBBBB","data":{"key":"KEYBBBBB","itemType":"journalArticle","title":"B","tags":[],"collections":["BBBBBBBB"]}},
            {"key":"KEYSHORT","data":{"key":"KEYSHORT","itemType":"journalArticle","title":"S","tags":[],"collections":["short"]}},
            {"key":"KEYORPHN","data":{"key":"KEYORPHN","itemType":"journalArticle","title":"O","tags":[],"collections":["ORPHAN01"]}}
        ])
        .to_string();
        let (reader, handle) = owned_server(vec![
            (200, named.clone(), page_headers("4", None)),
            (200, items.clone(), page_headers("4", None)),
            (200, named.clone(), page_headers("4", None)),
            (200, items.clone(), page_headers("4", None)),
            (200, named.clone(), page_headers("4", None)),
            (200, items.clone(), page_headers("4", None)),
            (200, named.clone(), page_headers("4", None)),
            (200, named, page_headers("4", None)),
        ]);
        let by_key = reader
            .library_list(Some("AAAAAAAA"), 10, 0, true)
            .await
            .expect("key wins over the same name");
        assert_eq!(by_key.items[0].key, "KEYAAAAA");
        let by_short = reader
            .library_list(Some("short"), 10, 0, true)
            .await
            .expect("short key");
        assert_eq!(by_short.items[0].key, "KEYSHORT");
        let by_orphan = reader
            .library_list(Some("Loose"), 10, 0, true)
            .await
            .expect("orphan name");
        assert_eq!(by_orphan.items[0].key, "KEYORPHN");
        let missing = reader
            .library_list(Some("Nope"), 10, 0, true)
            .await
            .expect_err("missing name");
        assert_eq!(missing.payload().code, "collection-not-found");
        assert_eq!(
            missing.payload().hint.as_deref(),
            Some("Use 'zot collection list' to inspect collection names")
        );
        let tree = reader.collection_list().await.expect("tree");
        assert!(tree.iter().all(|collection| collection.key != "ORPHAN01"));
        handle.join().expect("join");
    }

    #[tokio::test]
    async fn collection_object_404_and_list_404_use_different_codes() {
        let (reader, handle) = owned_server(vec![(200, "[]".to_string(), page_headers("0", None))]);
        let missing = reader.collection_get("NOPEKEY1").await.expect_err("get");
        assert_eq!(missing.payload().code, "collection-not-found");
        assert_eq!(missing.payload().message, "Collection 'NOPEKEY1' not found");
        assert_eq!(
            missing.payload().hint.as_deref(),
            Some("Use 'zot collection list' to inspect collection keys")
        );
        assert_eq!(handle.join().expect("join").len(), 1);

        let (reader, handle) = owned_server(vec![(200, "[]".to_string(), page_headers("0", None))]);
        let items = reader
            .collection_items("NOPEKEY1")
            .await
            .expect_err("items");
        assert_eq!(items.payload().code, "collection-not-found");
        assert_eq!(
            items.payload().hint.as_deref(),
            Some("Use 'zot collection list' to inspect collection names")
        );
        handle.join().expect("join");

        let present = json!([
            {"key":"COLMEM01","data":{"key":"COLMEM01","name":"Members","parentCollection":false}}
        ])
        .to_string();
        let (reader, handle) = owned_server(vec![(200, present, page_headers("1", None))]);
        let children = reader
            .collection_subcollections("MISSING1")
            .await
            .expect_err("subcollections");
        assert_eq!(children.payload().code, "collection-not-found");
        assert_eq!(
            children.payload().hint.as_deref(),
            Some("Use 'zot collection list' to inspect collection keys")
        );
        handle.join().expect("join");

        let (reader, handle) = owned_server(vec![(404, "{}".to_string(), headers("4"))]);
        let list = reader.collection_list().await.expect_err("list 404");
        assert_eq!(list.payload().code, "local-api-http");
        handle.join().expect("join");
    }

    #[tokio::test]
    async fn stats_and_tags_follow_parent_trash_and_count_duplicate_tags() {
        let body = json!([
            {"key":"BIB00001","data":{"key":"BIB00001","itemType":"journalArticle","title":"Paper","tags":[{"tag":"attention"},{"tag":"attention"}],"collections":["COL1"]}},
            {"key":"PDF00001","data":{"key":"PDF00001","itemType":"attachment","contentType":"application/pdf","parentItem":"BIB00001","tags":[],"collections":[]}},
            {"key":"PDF00002","data":{"key":"PDF00002","itemType":"attachment","contentType":"application/pdf","parentItem":"BIB00001","tags":[],"collections":[]}},
            {"key":"PDF0GONE","data":{"key":"PDF0GONE","itemType":"attachment","contentType":"application/pdf","parentItem":"GONE0001","tags":[],"collections":[]}},
            {"key":"NOT00001","data":{"key":"NOT00001","itemType":"note","parentItem":"BIB00001","tags":[{"tag":"note-tag"}],"collections":[]}},
            {"key":"NOT00002","data":{"key":"NOT00002","itemType":"note","parentItem":"BIB00001","tags":[{"tag":"note-tag"}],"collections":[]}},
            {"key":"ALONE001","data":{"key":"ALONE001","itemType":"note","tags":[{"tag":"alone"}],"collections":[]}},
            {"key":"TRSH0001","data":{"key":"TRSH0001","itemType":"journalArticle","title":"Old","deleted":true,"tags":[{"tag":"trashed"}],"collections":[]}},
            {"key":"PDF00003","data":{"key":"PDF00003","itemType":"attachment","contentType":"application/pdf","parentItem":"TRSH0001","tags":[],"collections":[]}},
            {"key":"NOT00003","data":{"key":"NOT00003","itemType":"note","parentItem":"TRSH0001","tags":[{"tag":"trash-note"}],"collections":[]}},
            {"key":"ANNO0001","data":{"key":"ANNO0001","itemType":"annotation","parentItem":"PDF00001","tags":[{"tag":"anno-tag"}],"collections":[]}}
        ])
        .to_string();
        let (reader, handle) = owned_server(vec![
            (200, body.clone(), page_headers("11", None)),
            (200, body.clone(), page_headers("11", None)),
            (200, body, page_headers("11", None)),
        ]);
        let stats = reader.library_stats(false).await.expect("stats");
        assert_eq!(stats.total_items, 1);
        assert_eq!(stats.by_type.get("journalArticle"), Some(&1));
        assert_eq!(stats.top_tags.get("attention"), Some(&2));
        assert!(!stats.top_tags.contains_key("note-tag"));
        assert!(!stats.top_tags.contains_key("trashed"));
        assert_eq!(stats.collections.get("COL1"), Some(&1));
        assert_eq!(stats.pdf_attachments, 1);
        assert_eq!(stats.notes, 2);
        let with_trash = reader.library_stats(true).await.expect("trash stats");
        assert_eq!(with_trash.total_items, 2);
        assert_eq!(with_trash.top_tags.get("attention"), Some(&2));
        assert_eq!(with_trash.top_tags.get("trashed"), Some(&1));
        assert_eq!(with_trash.pdf_attachments, 2);
        assert_eq!(with_trash.notes, 3);
        let tags = reader.library_tags().await.expect("tags");
        assert_eq!(
            tags.iter()
                .map(|tag| (tag.name.as_str(), tag.count))
                .collect::<Vec<_>>(),
            vec![
                ("attention", 2),
                ("note-tag", 2),
                ("alone", 1),
                ("anno-tag", 1),
                ("trash-note", 1),
                ("trashed", 1),
            ]
        );
        let captured = handle.join().expect("join");
        assert!(captured.iter().all(|(_, url, _)| {
            url.contains("includeTrashed=1") && url.contains("limit=100") && !url.contains("sort=")
        }));
    }

    #[tokio::test]
    async fn read_412_and_redirect_stay_immediate_desktop_errors() {
        let (reader, handle) = owned_server(vec![(412, String::new(), headers("4"))]);
        let conflict = reader.library_stats(false).await.expect_err("412");
        assert_eq!(conflict.payload().code, "local-api-conflict");
        assert_eq!(handle.join().expect("join").len(), 1);

        let (reader, handle) = owned_server(vec![(
            302,
            String::new(),
            vec![("Location", "http://example.invalid/api/users/0/items")],
        )]);
        let redirected = reader.collection_list().await.expect_err("redirect");
        assert_eq!(redirected.payload().code, "local-api-redirect");
        assert_eq!(handle.join().expect("join").len(), 1);
    }

    #[tokio::test]
    async fn unknown_collection_member_url_returning_200_is_not_a_count() {
        let library = json!([
            {"key":"WHOLE001","data":{"key":"WHOLE001","itemType":"journalArticle","title":"Whole","tags":[{"tag":"library"}],"collections":[]}}
        ])
        .to_string();
        let (reader, handle) = serve_routed(move |url| {
            if url.contains("/collections?") && !url.contains("items") {
                return (200, "[]".to_string(), owned_headers("45", Some("0"), None));
            }
            (200, library.clone(), owned_headers("45", Some("1"), None))
        });
        let items = reader
            .collection_items("NOPEKEY1")
            .await
            .expect_err("items");
        let count = reader
            .collection_item_count("NOPEKEY1")
            .await
            .expect_err("count");
        let tags = reader.collection_tags("NOPEKEY1").await.expect_err("tags");
        for error in [&items, &count, &tags] {
            assert_eq!(error.payload().code, "collection-not-found");
        }
        let captured = handle.join().expect("join");
        assert!(
            captured
                .iter()
                .all(|(_, url, _)| !url.contains("items/top"))
        );
    }

    #[tokio::test]
    async fn collection_members_resolve_a_unique_name_and_reject_duplicates() {
        let only = json!([
            {"key":"ONLYKEY1","data":{"key":"ONLYKEY1","name":"Only","parentCollection":false}}
        ])
        .to_string();
        let article = json!([
            {"key":"ART00030","data":{"key":"ART00030","itemType":"journalArticle","title":"Paper","tags":[],"collections":["ONLYKEY1"]}}
        ])
        .to_string();
        let (reader, handle) = owned_server(vec![
            (200, only.clone(), page_headers("1", None)),
            (200, article, page_headers("1", None)),
        ]);
        let items = reader.collection_items("Only").await.expect("unique name");
        assert_eq!(items[0].key, "ART00030");
        let captured = handle.join().expect("join");
        assert!(captured[1].1.contains("/collections/ONLYKEY1/items/top"));
        assert!(!captured[1].1.contains("/collections/Only/"));

        let shared = json!([
            {"key":"ZZZZZZZZ","data":{"key":"ZZZZZZZZ","name":"Shared","parentCollection":false}},
            {"key":"AAAAAAAA","data":{"key":"AAAAAAAA","name":"Shared","parentCollection":false}}
        ])
        .to_string();
        let (reader, handle) = owned_server(vec![(200, shared, page_headers("2", None))]);
        let error = reader
            .collection_item_count("Shared")
            .await
            .expect_err("ambiguous");
        assert_eq!(error.payload().code, "collection-ambiguous");
        assert_eq!(
            error.payload().hint.as_deref(),
            Some("Use one of these collection keys: AAAAAAAA, ZZZZZZZZ")
        );
        assert_eq!(handle.join().expect("join").len(), 1);
    }

    #[tokio::test]
    async fn collection_get_ignores_the_object_version() {
        let list = json!([
            {"key":"COLGET01","data":{"key":"COLGET01","name":"Parent","parentCollection":false}},
            {"key":"COLGET02","data":{"key":"COLGET02","name":"Child","parentCollection":"COLGET01"}}
        ])
        .to_string();
        let object = json!({
            "key": "COLGET01",
            "version": 7,
            "data": {"key": "COLGET01", "name": "Parent", "parentCollection": false}
        })
        .to_string();
        let (reader, handle) = serve_routed(move |url| {
            if url.contains("/collections?") {
                return (200, list.clone(), owned_headers("45", Some("2"), None));
            }
            (200, object.clone(), owned_headers("7", Some("1"), None))
        });
        let collection = reader.collection_get("COLGET01").await.expect("stable");
        assert_eq!(collection.name, "Parent");
        assert_eq!(collection.children.len(), 1);
        assert_eq!(collection.children[0].key, "COLGET02");
        let captured = handle.join().expect("join");
        assert!(
            captured
                .iter()
                .all(|(_, url, _)| url.contains("/collections?"))
        );
        assert!(captured.iter().all(|(_, url, _)| !url.contains("COLGET01")));
    }

    #[tokio::test]
    async fn library_list_accepts_10000_filtered_rows_across_100_pages() {
        let (reader, handle) = serve_routed(|url| {
            if !url.contains("/items/top?")
                || !url.contains("itemType=-attachment")
                || !url.contains("itemType=-note")
                || !url.contains("itemType=-annotation")
                || url.contains("sort=")
            {
                return (
                    500,
                    "unfiltered".to_string(),
                    owned_headers("4", None, None),
                );
            }
            let start = usize::try_from(query_start(url)).unwrap_or(usize::MAX);
            let next = (start + 100 < 10_000).then(|| {
                format!(
                    "</api/users/0/items/top?limit=100&start={}>; rel=\"next\"",
                    start + 100
                )
            });
            (
                200,
                bibliographic_page_at(start, 100),
                owned_headers("4", Some("10000"), next),
            )
        });
        let page = reader.library_list(None, 1, 0, true).await.expect("full");
        assert_eq!(page.total, 10_000);
        assert_eq!(page.items[0].key, "K0009999");
        assert_eq!(
            page.items[0].date_added.as_deref(),
            Some("2024-01-01 12:30:45")
        );
        let captured = handle.join().expect("join");
        assert_eq!(captured.len(), 100);
        assert!(captured[99].1.contains("start=9900"));
    }

    #[tokio::test]
    async fn library_list_collection_cap_ignores_the_rest_of_the_library() {
        let list = json!([
            {"key":"COLONLY1","data":{"key":"COLONLY1","name":"Only","parentCollection":false}}
        ])
        .to_string();
        let member = json!([
            {"key":"INCOL001","data":{"key":"INCOL001","itemType":"journalArticle","title":"Inside","tags":[],"collections":["COLONLY1"],"dateAdded":"2024-05-01T00:00:00Z","dateModified":"2024-05-01T00:00:00Z"}},
            {"key":"ATT00099","data":{"key":"ATT00099","itemType":"attachment","title":"Loose","tags":[],"collections":["COLONLY1"]}}
        ])
        .to_string();
        let (reader, handle) = serve_routed(move |url| {
            if url.contains("/collections/COLONLY1/items/top")
                && url.contains("itemType=-attachment")
            {
                return (200, member.clone(), owned_headers("8", Some("1"), None));
            }
            if url.contains("/collections?") {
                return (200, list.clone(), owned_headers("8", Some("1"), None));
            }
            (
                200,
                "[]".to_string(),
                owned_headers("8", Some("10001"), None),
            )
        });
        let page = reader
            .library_list(Some("Only"), 10, 0, true)
            .await
            .expect("one article");
        assert_eq!(page.total, 1);
        assert_eq!(page.items[0].key, "INCOL001");
        assert_eq!(
            page.items[0].date_added.as_deref(),
            Some("2024-05-01 00:00:00")
        );
        let captured = handle.join().expect("join");
        assert!(
            captured
                .iter()
                .any(|(_, url, _)| url.contains("/collections/COLONLY1/items/top"))
        );
        assert!(
            captured
                .iter()
                .all(|(_, url, _)| !url.contains("/items?") || url.contains("/items/top"))
        );
    }
}
