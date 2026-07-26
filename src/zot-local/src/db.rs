use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use regex::Regex;
use rusqlite::{
    Connection, ErrorCode, OpenFlags, OptionalExtension,
    backup::{Backup, StepResult},
    params, params_from_iter,
};
use serde::Serialize;
use strsim::normalized_levenshtein;
use tempfile::TempDir;
use zot_core::{
    AnnotationRecord, Attachment, ChildAnnotation, ChildAttachment, ChildItem, ChildNote,
    CitationKeyMatch, Collection, Creator, DuplicateGroup, DuplicateScanResult, FeedInfo,
    GraphOptions, Item, KnowledgeGraph, LibraryInfo, LibraryScope, LibraryStats, Note,
    NoteSearchResult, SearchResult, TagSummary, ZotError, ZotResult,
};

use crate::citation::export_item;
use crate::graph::{PairAccum, score_pair};

const EXCLUDED_TYPE_NAMES: &[&str] = &["attachment", "note", "annotation"];
const ZOTERO_DB_BUSY_TIMEOUT: Duration = Duration::from_secs(5);
const SNAPSHOT_STEP_PAUSE: Duration = Duration::from_millis(5);
const SNAPSHOT_PAGES_PER_STEP: i32 = 256;
const DUPLICATE_TITLE_THRESHOLD: f64 = 0.92;
const DUPLICATE_TITLE_PREFIX_CHARS: usize = 12;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LibrarySnapshotMeta {
    pub source_modified_at: Option<String>,
    pub snapshot_created_at: String,
    pub schema_version: Option<i64>,
}

#[derive(Debug, Clone, Copy)]
struct SnapshotPolicy {
    busy_timeout: Duration,
    busy_retry_limit: Duration,
    step_pause: Duration,
    pages_per_step: i32,
}

impl Default for SnapshotPolicy {
    fn default() -> Self {
        Self {
            busy_timeout: ZOTERO_DB_BUSY_TIMEOUT,
            busy_retry_limit: ZOTERO_DB_BUSY_TIMEOUT,
            step_pause: SNAPSHOT_STEP_PAUSE,
            pages_per_step: SNAPSHOT_PAGES_PER_STEP,
        }
    }
}

/// Escape SQLite `LIKE` wildcards in user-provided text so that `%` and `_`
/// are matched literally. Pair with `LIKE ? ESCAPE '\\'` in SQL.
fn escape_like(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '\\' | '%' | '_' => {
                out.push('\\');
                out.push(ch);
            }
            other => out.push(other),
        }
    }
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortField {
    DateAdded,
    DateModified,
    Title,
    Creator,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
    Asc,
    Desc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DuplicateMatchMethod {
    Title,
    Doi,
    Both,
}

#[derive(Debug, Clone)]
pub struct SearchOptions {
    pub query: String,
    pub collection: Option<String>,
    pub item_type: Option<String>,
    pub tag: Option<String>,
    pub creator: Option<String>,
    pub year: Option<String>,
    pub sort: Option<SortField>,
    pub direction: SortDirection,
    pub limit: usize,
    pub offset: usize,
    /// Drop items sitting in Zotero's trash (`deletedItems`). Defaults to
    /// `true`; callers must explicitly opt into returning trashed items.
    pub exclude_trashed: bool,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            query: String::new(),
            collection: None,
            item_type: None,
            tag: None,
            creator: None,
            year: None,
            sort: None,
            direction: SortDirection::Desc,
            limit: 50,
            offset: 0,
            exclude_trashed: true,
        }
    }
}

pub struct LocalLibrary {
    db_path: PathBuf,
    pub data_dir: PathBuf,
    library_scope: LibraryScope,
    library_id: i64,
    conn: Connection,
    _temp_dir: TempDir,
    snapshot_meta: LibrarySnapshotMeta,
    collections_cache: std::cell::OnceCell<Vec<Collection>>,
}

impl LocalLibrary {
    pub fn open(data_dir: impl AsRef<Path>, scope: LibraryScope) -> ZotResult<Self> {
        let data_dir = data_dir.as_ref().to_path_buf();
        let db_path = data_dir.join("zotero.sqlite");
        if !db_path.exists() {
            return Err(ZotError::Database {
                code: "db-not-found".to_string(),
                message: format!("Zotero database not found: {}", db_path.display()),
                hint: Some("Set ZOT_DATA_DIR or update ~/.config/zot/config.toml".to_string()),
            });
        }

        let (conn, temp_dir, snapshot_meta) = Self::connect(&db_path)?;
        let mut instance = Self {
            db_path,
            data_dir,
            library_scope: scope,
            library_id: 1,
            conn,
            _temp_dir: temp_dir,
            snapshot_meta,
            collections_cache: std::cell::OnceCell::new(),
        };
        instance.library_id = instance.resolve_library_id()?;
        Ok(instance)
    }

    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    pub fn library_id(&self) -> i64 {
        self.library_id
    }

    pub fn snapshot_meta(&self) -> &LibrarySnapshotMeta {
        &self.snapshot_meta
    }

    pub fn resolve_group_library_id(&self, group_id: i64) -> ZotResult<Option<i64>> {
        self.conn
            .query_row(
                "SELECT libraryID FROM groups WHERE groupID = ?1",
                params![group_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map_err(sql_err("resolve-group-library"))
    }

    pub fn check_schema_compatibility(&self) -> ZotResult<Option<i64>> {
        Ok(self.snapshot_meta.schema_version)
    }

    pub fn list_items(
        &self,
        collection: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> ZotResult<Vec<Item>> {
        let result = self.search(SearchOptions {
            query: String::new(),
            collection: collection.map(ToOwned::to_owned),
            limit,
            offset,
            ..SearchOptions::default()
        })?;
        Ok(result.items)
    }

    pub fn search(&self, options: SearchOptions) -> ZotResult<SearchResult> {
        let mut predicates = vec![
            "i.libraryID = ?".to_string(),
            "it.typeName NOT IN ('attachment','note','annotation')".to_string(),
        ];
        let mut values = vec![rusqlite::types::Value::from(self.library_id)];

        if options.exclude_trashed {
            predicates.push(
                "NOT EXISTS (SELECT 1 FROM deletedItems d WHERE d.itemID = i.itemID)".to_string(),
            );
        }
        if !options.query.is_empty() {
            let like = format!("%{}%", escape_like(&options.query));
            predicates.push(
                "(EXISTS (SELECT 1 FROM itemData id JOIN itemDataValues iv ON id.valueID = iv.valueID WHERE id.itemID = i.itemID AND iv.value LIKE ? ESCAPE '\\')
                  OR EXISTS (SELECT 1 FROM itemCreators ic JOIN creators c ON ic.creatorID = c.creatorID WHERE ic.itemID = i.itemID AND (c.firstName LIKE ? ESCAPE '\\' OR c.lastName LIKE ? ESCAPE '\\'))
                  OR EXISTS (SELECT 1 FROM itemTags itq JOIN tags tq ON itq.tagID = tq.tagID WHERE itq.itemID = i.itemID AND tq.name LIKE ? ESCAPE '\\')
                  OR EXISTS (SELECT 1 FROM itemAttachments ia JOIN fulltextItemWords fw ON ia.itemID = fw.itemID JOIN fulltextWords w ON fw.wordID = w.wordID WHERE ia.parentItemID = i.itemID AND w.word LIKE ? ESCAPE '\\'))"
                    .to_string(),
            );
            values.extend(std::iter::repeat_n(rusqlite::types::Value::from(like), 5));
        }
        if let Some(collection) = options.collection.as_deref() {
            let collection_id = self.resolve_collection_id(collection)?;
            predicates.push(
                "EXISTS (SELECT 1 FROM collectionItems ci WHERE ci.itemID = i.itemID AND ci.collectionID = ?)"
                    .to_string(),
            );
            values.push(rusqlite::types::Value::from(collection_id));
        }
        if let Some(item_type) = options.item_type.as_deref() {
            predicates.push("it.typeName = ?".to_string());
            values.push(rusqlite::types::Value::from(item_type.to_string()));
        }
        if let Some(tag) = options.tag.as_deref() {
            predicates.push(
                "EXISTS (SELECT 1 FROM itemTags itf JOIN tags tf ON itf.tagID = tf.tagID WHERE itf.itemID = i.itemID AND LOWER(tf.name) = ?)"
                    .to_string(),
            );
            values.push(rusqlite::types::Value::from(tag.to_lowercase()));
        }
        if let Some(creator) = options.creator.as_deref() {
            predicates.push(
                "EXISTS (SELECT 1 FROM itemCreators icf JOIN creators cf ON icf.creatorID = cf.creatorID WHERE icf.itemID = i.itemID AND LOWER(TRIM(COALESCE(cf.firstName, '') || ' ' || cf.lastName)) LIKE ? ESCAPE '\\')"
                    .to_string(),
            );
            values.push(rusqlite::types::Value::from(format!(
                "%{}%",
                escape_like(&creator.to_lowercase())
            )));
        }
        if let Some(year) = options.year.as_deref() {
            predicates.push(
                "EXISTS (SELECT 1 FROM itemData idy JOIN fields fy ON idy.fieldID = fy.fieldID JOIN itemDataValues ivy ON idy.valueID = ivy.valueID WHERE idy.itemID = i.itemID AND fy.fieldName = 'date' AND ivy.value LIKE ? ESCAPE '\\')"
                    .to_string(),
            );
            values.push(rusqlite::types::Value::from(format!(
                "{}%",
                escape_like(year)
            )));
        }

        let from_where = format!(
            "FROM items i JOIN itemTypes it ON i.itemTypeID = it.itemTypeID WHERE {}",
            predicates.join(" AND ")
        );
        let total = self
            .conn
            .prepare_cached(&format!("SELECT COUNT(*) {from_where}"))
            .map_err(sql_err("search-count"))?
            .query_row(params_from_iter(values.iter()), |row| row.get::<_, i64>(0))
            .map_err(sql_err("search-count"))? as usize;

        let sort_expression = match options.sort {
            Some(SortField::Title) => {
                "COALESCE((SELECT LOWER(iv.value) FROM itemData id JOIN fields f ON id.fieldID = f.fieldID JOIN itemDataValues iv ON id.valueID = iv.valueID WHERE id.itemID = i.itemID AND f.fieldName = 'title' LIMIT 1), '')"
            }
            Some(SortField::Creator) => {
                "COALESCE((SELECT LOWER(TRIM(COALESCE(c.firstName, '') || ' ' || c.lastName)) FROM itemCreators ic JOIN creators c ON ic.creatorID = c.creatorID WHERE ic.itemID = i.itemID ORDER BY ic.orderIndex LIMIT 1), '')"
            }
            Some(SortField::DateAdded) => "COALESCE(i.dateAdded, '')",
            Some(SortField::DateModified) => "COALESCE(i.dateModified, '')",
            None => "i.key",
        };
        let direction = match options.direction {
            SortDirection::Asc => "ASC",
            SortDirection::Desc => "DESC",
        };
        let limit = i64::try_from(options.limit).map_err(|_| ZotError::InvalidInput {
            code: "search-limit".to_string(),
            message: "Search limit is too large".to_string(),
            hint: Some("Use a smaller --limit value".to_string()),
        })?;
        let offset = i64::try_from(options.offset).map_err(|_| ZotError::InvalidInput {
            code: "search-offset".to_string(),
            message: "Search offset is too large".to_string(),
            hint: Some("Use a smaller --offset value".to_string()),
        })?;
        let page_sql = format!(
            "SELECT i.itemID {from_where} ORDER BY {sort_expression} {direction}, i.key {direction} LIMIT ? OFFSET ?"
        );
        let mut page_values = values;
        page_values.push(rusqlite::types::Value::from(limit));
        page_values.push(rusqlite::types::Value::from(offset));
        let mut stmt = self
            .conn
            .prepare_cached(&page_sql)
            .map_err(sql_err("search-page"))?;
        let rows = stmt
            .query_map(params_from_iter(page_values.iter()), |row| {
                row.get::<_, i64>(0)
            })
            .map_err(sql_err("search-page"))?;
        let item_ids = rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(sql_err("search-page"))?;
        let items = self.get_items_batch(&item_ids)?;

        Ok(SearchResult {
            items,
            total,
            query: options.query,
        })
    }

    pub fn get_item(&self, key: &str) -> ZotResult<Option<Item>> {
        let item_id = self
            .conn
            .query_row(
                "SELECT i.itemID FROM items i
                 JOIN itemTypes it ON i.itemTypeID = it.itemTypeID
                 WHERE i.key = ?1 AND i.libraryID = ?2
                 AND it.typeName NOT IN ('attachment','note','annotation')",
                params![key, self.library_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map_err(sql_err("get-item"))?;
        match item_id {
            Some(id) => self.get_item_by_id(id).map(Some),
            None => Ok(None),
        }
    }

    pub fn get_notes(&self, key: &str) -> ZotResult<Vec<Note>> {
        let parent_id = self.parent_item_id(key)?;
        let Some(parent_id) = parent_id else {
            return Ok(Vec::new());
        };

        let mut stmt = self.conn.prepare_cached(
            "SELECT i.itemID, i.key, n.note FROM itemNotes n JOIN items i ON n.itemID = i.itemID WHERE n.parentItemID = ?1",
        )
        .map_err(sql_err("get-notes"))?;
        let rows = stmt
            .query_map(params![parent_id], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                ))
            })
            .map_err(sql_err("get-notes"))?;

        let rows = rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(sql_err("get-notes"))?;
        let note_item_ids = rows.iter().map(|row| row.0).collect::<Vec<_>>();
        let tags_by_id = self.load_item_tags_batch(&note_item_ids)?;
        let mut notes = Vec::with_capacity(rows.len());
        for (note_item_id, note_key, note_html) in rows {
            let tags = tags_by_id.get(&note_item_id).cloned().unwrap_or_default();
            notes.push(Note {
                key: note_key,
                parent_key: key.to_string(),
                content: html_to_text(&note_html),
                tags,
            });
        }
        Ok(notes)
    }

    pub fn search_notes(&self, query: &str, limit: usize) -> ZotResult<Vec<NoteSearchResult>> {
        let pattern = format!("%{}%", escape_like(query));
        let title_field_id = self.field_id("title")?.unwrap_or(4);
        let sql = format!(
            "SELECT i.key, n.note, n.title, pi.key, pdv.value
             FROM itemNotes n
             JOIN items i ON n.itemID = i.itemID
             LEFT JOIN items pi ON n.parentItemID = pi.itemID
             LEFT JOIN itemData pd ON pi.itemID = pd.itemID AND pd.fieldID = {title_field_id}
             LEFT JOIN itemDataValues pdv ON pd.valueID = pdv.valueID
             WHERE n.note LIKE ?1 ESCAPE '\\'
             AND i.libraryID = ?2
             AND i.itemID NOT IN (SELECT itemID FROM deletedItems)
             LIMIT ?3"
        );
        let mut stmt = self
            .conn
            .prepare_cached(&sql)
            .map_err(sql_err("search-notes"))?;
        let rows = stmt
            .query_map(params![pattern, self.library_id, limit as i64], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                ))
            })
            .map_err(sql_err("search-notes"))?;

        let mut results = Vec::new();
        for row in rows {
            let (key, note_html, title, parent_key, parent_title) =
                row.map_err(sql_err("search-notes"))?;
            let clean = html_to_text(&note_html);
            if !clean.to_lowercase().contains(&query.to_lowercase()) {
                continue;
            }
            let tags = if let Some(item_id) = self.item_id_by_key(&key)? {
                self.get_item_tags(item_id)?
            } else {
                Vec::new()
            };
            results.push(NoteSearchResult {
                key,
                parent_key,
                parent_title,
                title,
                content: clean,
                tags,
            });
        }
        Ok(results)
    }

    pub fn get_tags(&self) -> ZotResult<Vec<TagSummary>> {
        let mut stmt = self
            .conn
            .prepare_cached(
                "SELECT t.name, COUNT(*) as cnt
                 FROM itemTags it
                 JOIN tags t ON it.tagID = t.tagID
                 JOIN items i ON it.itemID = i.itemID
                 WHERE i.libraryID = ?1
                 GROUP BY t.tagID, t.name
                 ORDER BY cnt DESC, t.name ASC",
            )
            .map_err(sql_err("get-tags"))?;
        let rows = stmt
            .query_map(params![self.library_id], |row| {
                Ok(TagSummary {
                    name: row.get::<_, String>(0)?,
                    count: row.get::<_, i64>(1)? as usize,
                })
            })
            .map_err(sql_err("get-tags"))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(sql_err("get-tags"))
    }

    pub fn search_by_citation_key(&self, citekey: &str) -> ZotResult<Option<CitationKeyMatch>> {
        let field_id = self.field_id("extra")?;
        let Some(field_id) = field_id else {
            return Ok(None);
        };
        let mut stmt = self
            .conn
            .prepare_cached(
                "SELECT i.key, iv.value
                 FROM items i
                 JOIN itemData id ON i.itemID = id.itemID
                 JOIN itemDataValues iv ON id.valueID = iv.valueID
                 WHERE i.libraryID = ?1 AND id.fieldID = ?2",
            )
            .map_err(sql_err("search-citation-key"))?;
        let rows = stmt
            .query_map(params![self.library_id, field_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(sql_err("search-citation-key"))?;
        for row in rows {
            let (item_key, extra) = row.map_err(sql_err("search-citation-key"))?;
            for line in extra.lines() {
                let normalized = line.trim().to_lowercase();
                if (normalized.starts_with("citation key:")
                    || normalized.starts_with("citationkey:"))
                    && line
                        .split_once(':')
                        .map(|(_, value)| value.trim() == citekey)
                        .unwrap_or(false)
                    && let Some(item) = self.get_item(&item_key)?
                {
                    return Ok(Some(CitationKeyMatch {
                        citekey: citekey.to_string(),
                        source: "extra".to_string(),
                        item,
                    }));
                }
            }
        }
        Ok(None)
    }

    pub fn get_item_children(&self, key: &str) -> ZotResult<Vec<ChildItem>> {
        let mut children = Vec::new();
        children.extend(
            self.get_note_children(key)?
                .into_iter()
                .map(ChildItem::Note),
        );
        children.extend(
            self.get_attachment_children(key)?
                .into_iter()
                .map(ChildItem::Attachment),
        );
        children.extend(
            self.get_annotation_children(key)?
                .into_iter()
                .map(ChildItem::Annotation),
        );
        Ok(children)
    }

    pub fn get_items_children(
        &self,
        keys: &[String],
    ) -> ZotResult<BTreeMap<String, Vec<ChildItem>>> {
        let mut grouped = BTreeMap::new();
        for key in keys {
            grouped.insert(key.clone(), self.get_item_children(key)?);
        }
        Ok(grouped)
    }

    pub fn get_annotations(
        &self,
        item_key: Option<&str>,
        limit: usize,
    ) -> ZotResult<Vec<AnnotationRecord>> {
        if !self.table_exists("itemAnnotations")? {
            return Ok(Vec::new());
        }
        let mut results = if let Some(item_key) = item_key {
            self.get_annotation_children(item_key)?
                .into_iter()
                .map(|child| AnnotationRecord {
                    key: child.key,
                    parent_key: None,
                    parent_title: None,
                    attachment_key: child.parent_key,
                    attachment_title: None,
                    annotation_type: child.annotation_type,
                    text: child.text,
                    comment: child.comment,
                    color: child.color,
                    page_label: child.page_label,
                    tags: child.tags,
                })
                .collect::<Vec<_>>()
        } else {
            let title_field_id = self.field_id("title")?.unwrap_or(4);
            let sql = format!(
                "SELECT i.key, ia.text, ia.comment, ia.color, ia.pageLabel, ia.type,
                        att.key, gpi.key, gpdv.value
                 FROM itemAnnotations ia
                 JOIN items i ON ia.itemID = i.itemID
                 LEFT JOIN items att ON ia.parentItemID = att.itemID
                 LEFT JOIN itemAttachments iatt ON ia.parentItemID = iatt.itemID
                 LEFT JOIN items gpi ON iatt.parentItemID = gpi.itemID
                 LEFT JOIN itemData gpd ON gpi.itemID = gpd.itemID AND gpd.fieldID = {title_field_id}
                 LEFT JOIN itemDataValues gpdv ON gpd.valueID = gpdv.valueID
                 WHERE i.libraryID = ?1
                 AND i.itemID NOT IN (SELECT itemID FROM deletedItems)
                 ORDER BY i.key ASC
                 LIMIT ?2"
            );
            let mut stmt = self
                .conn
                .prepare_cached(&sql)
                .map_err(sql_err("get-annotations"))?;
            let rows = stmt
                .query_map(params![self.library_id, limit as i64], |row| {
                    Ok(AnnotationRecord {
                        key: row.get::<_, String>(0)?,
                        parent_key: row.get::<_, Option<String>>(7)?,
                        parent_title: row.get::<_, Option<String>>(8)?,
                        attachment_key: row.get::<_, Option<String>>(6)?,
                        attachment_title: None,
                        annotation_type: annotation_type_name(row.get::<_, i64>(5)?),
                        text: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                        comment: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                        color: row.get::<_, Option<String>>(3)?,
                        page_label: row.get::<_, Option<String>>(4)?,
                        tags: Vec::new(),
                    })
                })
                .map_err(sql_err("get-annotations"))?;
            rows.collect::<Result<Vec<_>, _>>()
                .map_err(sql_err("get-annotations"))?
        };
        results.truncate(limit);
        Ok(results)
    }

    pub fn search_annotations(
        &self,
        query: &str,
        limit: usize,
    ) -> ZotResult<Vec<AnnotationRecord>> {
        if !self.table_exists("itemAnnotations")? {
            return Ok(Vec::new());
        }
        let pattern = format!("%{}%", escape_like(query));
        let title_field_id = self.field_id("title")?.unwrap_or(4);
        let sql = format!(
            "SELECT i.key, ia.text, ia.comment, ia.color, ia.pageLabel, ia.type,
                    att.key, gpi.key, gpdv.value
             FROM itemAnnotations ia
             JOIN items i ON ia.itemID = i.itemID
             LEFT JOIN items att ON ia.parentItemID = att.itemID
             LEFT JOIN itemAttachments iatt ON ia.parentItemID = iatt.itemID
             LEFT JOIN items gpi ON iatt.parentItemID = gpi.itemID
             LEFT JOIN itemData gpd ON gpi.itemID = gpd.itemID AND gpd.fieldID = {title_field_id}
             LEFT JOIN itemDataValues gpdv ON gpd.valueID = gpdv.valueID
             WHERE (ia.text LIKE ?1 ESCAPE '\\' OR ia.comment LIKE ?1 ESCAPE '\\')
             AND i.libraryID = ?2
             AND i.itemID NOT IN (SELECT itemID FROM deletedItems)
             LIMIT ?3"
        );
        let mut stmt = self
            .conn
            .prepare_cached(&sql)
            .map_err(sql_err("search-annotations"))?;
        let rows = stmt
            .query_map(params![pattern, self.library_id, limit as i64], |row| {
                Ok(AnnotationRecord {
                    key: row.get::<_, String>(0)?,
                    parent_key: row.get::<_, Option<String>>(7)?,
                    parent_title: row.get::<_, Option<String>>(8)?,
                    attachment_key: row.get::<_, Option<String>>(6)?,
                    attachment_title: None,
                    annotation_type: annotation_type_name(row.get::<_, i64>(5)?),
                    text: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                    comment: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                    color: row.get::<_, Option<String>>(3)?,
                    page_label: row.get::<_, Option<String>>(4)?,
                    tags: Vec::new(),
                })
            })
            .map_err(sql_err("search-annotations"))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(sql_err("search-annotations"))
    }

    pub fn get_collections(&self) -> ZotResult<Vec<Collection>> {
        if let Some(cached) = self.collections_cache.get() {
            return Ok(cached.clone());
        }
        let fresh = self.load_collections_tree()?;
        let _ = self.collections_cache.set(fresh.clone());
        Ok(fresh)
    }

    fn load_collections_tree(&self) -> ZotResult<Vec<Collection>> {
        let mut stmt = self
            .conn
            .prepare_cached("SELECT collectionID, collectionName, parentCollectionID, key FROM collections WHERE libraryID = ?1")
            .map_err(sql_err("get-collections"))?;
        let rows = stmt
            .query_map(params![self.library_id], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<i64>>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })
            .map_err(sql_err("get-collections"))?;

        let mut collection_map: HashMap<i64, Collection> = HashMap::new();
        let mut parent_map: HashMap<i64, Option<i64>> = HashMap::new();
        for row in rows {
            let (collection_id, name, parent_collection_id, key) =
                row.map_err(sql_err("get-collections"))?;
            collection_map.insert(
                collection_id,
                Collection {
                    key,
                    name,
                    parent_key: None,
                    children: Vec::new(),
                },
            );
            parent_map.insert(collection_id, parent_collection_id);
        }

        let ids = collection_map.keys().copied().collect::<Vec<_>>();
        for id in ids {
            if let Some(Some(parent_id)) = parent_map.get(&id).copied() {
                let parent_key = collection_map
                    .get(&parent_id)
                    .map(|parent| parent.key.clone());
                if let Some(child) = collection_map.get_mut(&id) {
                    child.parent_key = parent_key;
                }
            }
        }

        let mut children_by_parent: HashMap<i64, Vec<i64>> = HashMap::new();
        for (collection_id, parent_id) in &parent_map {
            if let Some(parent_id) = parent_id {
                children_by_parent
                    .entry(*parent_id)
                    .or_default()
                    .push(*collection_id);
            }
        }

        fn build_tree(
            root_id: i64,
            collection_map: &HashMap<i64, Collection>,
            children_by_parent: &HashMap<i64, Vec<i64>>,
        ) -> Option<Collection> {
            let mut root = collection_map.get(&root_id)?.clone();
            if let Some(children) = children_by_parent.get(&root_id) {
                root.children = children
                    .iter()
                    .filter_map(|child_id| {
                        build_tree(*child_id, collection_map, children_by_parent)
                    })
                    .collect();
            }
            Some(root)
        }

        Ok(parent_map
            .iter()
            .filter_map(|(collection_id, parent_id)| {
                if parent_id.is_none() {
                    build_tree(*collection_id, &collection_map, &children_by_parent)
                } else {
                    None
                }
            })
            .collect())
    }

    pub fn search_collections(&self, query: &str, limit: usize) -> ZotResult<Vec<Collection>> {
        let query_lc = query.to_lowercase();
        let mut flattened = Vec::new();
        for collection in self.get_collections()? {
            flatten_collection_tree(&collection, &mut flattened);
        }
        Ok(flattened
            .into_iter()
            .filter(|collection| collection.name.to_lowercase().contains(&query_lc))
            .take(limit)
            .collect())
    }

    pub fn get_collection(&self, collection_key: &str) -> ZotResult<Option<Collection>> {
        let mut flattened = Vec::new();
        for collection in self.get_collections()? {
            flatten_collection_tree(&collection, &mut flattened);
        }
        Ok(flattened
            .into_iter()
            .find(|collection| collection.key == collection_key))
    }

    pub fn get_subcollections(&self, collection_key: &str) -> ZotResult<Vec<Collection>> {
        fn find_children(collection: &Collection, key: &str) -> Option<Vec<Collection>> {
            if collection.key == key {
                return Some(collection.children.clone());
            }
            for child in &collection.children {
                if let Some(found) = find_children(child, key) {
                    return Some(found);
                }
            }
            None
        }

        for collection in self.get_collections()? {
            if let Some(found) = find_children(&collection, collection_key) {
                return Ok(found);
            }
        }
        Err(ZotError::InvalidInput {
            code: "collection-not-found".to_string(),
            message: format!("Collection '{collection_key}' not found"),
            hint: Some("Use 'zot collection list' to inspect collection keys".to_string()),
        })
    }

    pub fn get_collection_items(&self, collection_key: &str) -> ZotResult<Vec<Item>> {
        let collection_id = self.resolve_collection_id(collection_key)?;
        let mut stmt = self
            .conn
            .prepare_cached("SELECT itemID FROM collectionItems WHERE collectionID = ?1 ORDER BY orderIndex ASC")
            .map_err(sql_err("get-collection-items"))?;
        let rows = stmt
            .query_map(params![collection_id], |row| row.get::<_, i64>(0))
            .map_err(sql_err("get-collection-items"))?;
        let item_ids = rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(sql_err("get-collection-items"))?;
        self.get_items_batch(&item_ids)
    }

    pub fn get_collection_item_count(&self, collection_key: &str) -> ZotResult<usize> {
        let collection_id = self.resolve_collection_id(collection_key)?;
        self.conn
            .query_row(
                "SELECT COUNT(*) FROM collectionItems WHERE collectionID = ?1",
                params![collection_id],
                |row| row.get::<_, i64>(0),
            )
            .map(|count| count as usize)
            .map_err(sql_err("get-collection-item-count"))
    }

    pub fn get_collection_tags(&self, collection_key: &str) -> ZotResult<Vec<TagSummary>> {
        let collection_id = self.resolve_collection_id(collection_key)?;
        let mut stmt = self
            .conn
            .prepare_cached(
                "SELECT t.name, COUNT(*) as cnt
                 FROM collectionItems ci
                 JOIN itemTags it ON ci.itemID = it.itemID
                 JOIN tags t ON it.tagID = t.tagID
                 WHERE ci.collectionID = ?1
                 GROUP BY t.tagID, t.name
                 ORDER BY cnt DESC, t.name ASC",
            )
            .map_err(sql_err("get-collection-tags"))?;
        let rows = stmt
            .query_map(params![collection_id], |row| {
                Ok(TagSummary {
                    name: row.get::<_, String>(0)?,
                    count: row.get::<_, i64>(1)? as usize,
                })
            })
            .map_err(sql_err("get-collection-tags"))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(sql_err("get-collection-tags"))
    }

    pub fn get_libraries(&self) -> ZotResult<Vec<LibraryInfo>> {
        if !self.table_exists("libraries")? {
            return Ok(Vec::new());
        }
        let feeds_available = self.table_exists("feeds")?;
        let feed_join = if feeds_available {
            "LEFT JOIN feeds f ON l.libraryID = f.libraryID"
        } else {
            ""
        };
        let query = format!(
            "SELECT l.libraryID, l.type, l.editable, l.filesEditable,
                    g.groupID, g.name, g.description,
                    {} AS feedName, {} AS feedUrl,
                    (SELECT COUNT(*)
                     FROM items i
                     JOIN itemTypes it ON i.itemTypeID = it.itemTypeID
                     WHERE i.libraryID = l.libraryID
                     AND it.typeName NOT IN ('attachment', 'note', 'annotation')) as itemCount
             FROM libraries l
             LEFT JOIN groups g ON l.libraryID = g.libraryID
             {}
             ORDER BY l.type, l.libraryID",
            if feeds_available { "f.name" } else { "NULL" },
            if feeds_available { "f.url" } else { "NULL" },
            feed_join
        );
        let mut stmt = self
            .conn
            .prepare_cached(&query)
            .map_err(sql_err("get-libraries"))?;
        let rows = stmt
            .query_map([], |row| {
                Ok(LibraryInfo {
                    library_id: row.get::<_, i64>(0)?,
                    library_type: row.get::<_, String>(1)?,
                    editable: row.get::<_, i64>(2)? != 0,
                    files_editable: row.get::<_, i64>(3)? != 0,
                    group_id: row.get::<_, Option<i64>>(4)?,
                    group_name: row.get::<_, Option<String>>(5)?,
                    group_description: row.get::<_, Option<String>>(6)?,
                    feed_name: row.get::<_, Option<String>>(7)?,
                    feed_url: row.get::<_, Option<String>>(8)?,
                    item_count: row.get::<_, i64>(9)? as usize,
                })
            })
            .map_err(sql_err("get-libraries"))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(sql_err("get-libraries"))
    }

    pub fn get_feeds(&self) -> ZotResult<Vec<FeedInfo>> {
        if !self.table_exists("feeds")? {
            return Ok(Vec::new());
        }
        let mut stmt = self
            .conn
            .prepare_cached(
                "SELECT f.libraryID, f.name, f.url, f.lastCheck, f.lastUpdate,
                        f.lastCheckError, f.refreshInterval,
                        (SELECT COUNT(*)
                         FROM feedItems fi
                         JOIN items i ON fi.itemID = i.itemID
                         WHERE i.libraryID = f.libraryID) as itemCount
                 FROM feeds f
                 ORDER BY f.name",
            )
            .map_err(sql_err("get-feeds"))?;
        let rows = stmt
            .query_map([], |row| {
                Ok(FeedInfo {
                    library_id: row.get::<_, i64>(0)?,
                    name: row.get::<_, String>(1)?,
                    url: row.get::<_, String>(2)?,
                    last_check: row.get::<_, Option<String>>(3)?,
                    last_update: row.get::<_, Option<String>>(4)?,
                    last_check_error: row.get::<_, Option<String>>(5)?,
                    refresh_interval: row.get::<_, Option<i64>>(6)?,
                    item_count: row.get::<_, i64>(7)? as usize,
                })
            })
            .map_err(sql_err("get-feeds"))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(sql_err("get-feeds"))
    }

    pub fn get_feed_items(&self, library_id: i64, limit: usize) -> ZotResult<Vec<Item>> {
        if !self.table_exists("feeds")? || !self.table_exists("feedItems")? {
            return Ok(Vec::new());
        }
        let title_field_id = self.field_id("title")?.unwrap_or(1);
        let abstract_field_id = self.field_id("abstractNote")?.unwrap_or(2);
        let url_field_id = self.field_id("url")?.unwrap_or(0);
        let query = format!(
            "SELECT i.itemID, i.key, it.typeName, i.dateAdded,
                    title_val.value, abstract_val.value, url_val.value
             FROM feedItems fi
             JOIN items i ON fi.itemID = i.itemID
             JOIN itemTypes it ON i.itemTypeID = it.itemTypeID
             LEFT JOIN itemData title_data ON i.itemID = title_data.itemID AND title_data.fieldID = {title_field_id}
             LEFT JOIN itemDataValues title_val ON title_data.valueID = title_val.valueID
             LEFT JOIN itemData abstract_data ON i.itemID = abstract_data.itemID AND abstract_data.fieldID = {abstract_field_id}
             LEFT JOIN itemDataValues abstract_val ON abstract_data.valueID = abstract_val.valueID
             LEFT JOIN itemData url_data ON i.itemID = url_data.itemID AND url_data.fieldID = {url_field_id}
             LEFT JOIN itemDataValues url_val ON url_data.valueID = url_val.valueID
             WHERE i.libraryID = ?1
             ORDER BY i.dateAdded DESC
             LIMIT ?2"
        );
        let mut stmt = self
            .conn
            .prepare_cached(&query)
            .map_err(sql_err("get-feed-items"))?;
        let rows = stmt
            .query_map(params![library_id, limit as i64], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<String>>(4)?.unwrap_or_default(),
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, Option<String>>(6)?,
                ))
            })
            .map_err(sql_err("get-feed-items"))?;
        let raw = rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(sql_err("get-feed-items"))?;
        raw.into_iter()
            .map(
                |(item_id, key, item_type, date_added, title, abstract_note, url)| {
                    Ok(Item {
                        key,
                        item_type,
                        title,
                        creators: self.get_item_creators(item_id)?,
                        abstract_note,
                        date: None,
                        url,
                        doi: None,
                        tags: Vec::new(),
                        collections: Vec::new(),
                        date_added,
                        date_modified: None,
                        extra: BTreeMap::new(),
                    })
                },
            )
            .collect()
    }

    pub fn get_attachments(&self, key: &str) -> ZotResult<Vec<Attachment>> {
        let parent_id = self.parent_item_id(key)?;
        let Some(parent_id) = parent_id else {
            return Ok(Vec::new());
        };

        let mut stmt = self.conn.prepare_cached(
            "SELECT i.key, ia.contentType, ia.path FROM itemAttachments ia JOIN items i ON ia.itemID = i.itemID WHERE ia.parentItemID = ?1",
        )
        .map_err(sql_err("get-attachments"))?;
        let rows = stmt
            .query_map(params![parent_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                    row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                ))
            })
            .map_err(sql_err("get-attachments"))?;

        let mut attachments = Vec::new();
        for row in rows {
            let (attachment_key, content_type, raw_path) =
                row.map_err(sql_err("get-attachments"))?;
            let filename = raw_path
                .strip_prefix("storage:")
                .unwrap_or(&raw_path)
                .to_string();
            attachments.push(Attachment {
                key: attachment_key,
                parent_key: key.to_string(),
                filename,
                content_type,
            });
        }
        Ok(attachments)
    }

    pub fn get_attachment_by_key(&self, key: &str) -> ZotResult<Option<Attachment>> {
        let mut stmt = self
            .conn
            .prepare_cached(
                "SELECT ia.parentItemID, ia.contentType, ia.path, parent.key
                 FROM itemAttachments ia
                 JOIN items i ON ia.itemID = i.itemID
                 LEFT JOIN items parent ON ia.parentItemID = parent.itemID
                 WHERE i.key = ?1 AND i.libraryID = ?2",
            )
            .map_err(sql_err("get-attachment-by-key"))?;
        let row = stmt
            .query_row(params![key, self.library_id], |row| {
                Ok((
                    row.get::<_, Option<i64>>(0)?,
                    row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                    row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                    row.get::<_, Option<String>>(3)?,
                ))
            })
            .optional()
            .map_err(sql_err("get-attachment-by-key"))?;
        Ok(
            row.map(|(_, content_type, raw_path, parent_key)| Attachment {
                key: key.to_string(),
                parent_key: parent_key.unwrap_or_default(),
                filename: raw_path
                    .strip_prefix("storage:")
                    .unwrap_or(&raw_path)
                    .to_string(),
                content_type,
            }),
        )
    }

    pub fn get_pdf_attachment(&self, key: &str) -> ZotResult<Option<Attachment>> {
        Ok(self
            .get_attachments(key)?
            .into_iter()
            .find(|attachment| attachment.content_type == "application/pdf"))
    }

    pub fn attachment_path(&self, attachment: &Attachment) -> PathBuf {
        self.data_dir
            .join("storage")
            .join(&attachment.key)
            .join(&attachment.filename)
    }

    pub fn pdf_path(&self, attachment: &Attachment) -> PathBuf {
        self.attachment_path(attachment)
    }

    pub fn get_recent_items(
        &self,
        since: &str,
        sort: SortField,
        limit: usize,
    ) -> ZotResult<Vec<Item>> {
        let column = match sort {
            SortField::DateModified => "dateModified",
            _ => "dateAdded",
        };
        let sql = format!(
            "SELECT itemID FROM items WHERE libraryID = ?1 AND {} >= ?2 ORDER BY {} DESC LIMIT ?3",
            column, column
        );
        let mut stmt = self
            .conn
            .prepare_cached(&sql)
            .map_err(sql_err("recent-items"))?;
        let rows = stmt
            .query_map(params![self.library_id, since, limit as i64], |row| {
                row.get::<_, i64>(0)
            })
            .map_err(sql_err("recent-items"))?;
        let item_ids = rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(sql_err("recent-items"))?;
        self.get_items_batch(&item_ids)
    }

    pub fn get_recent_items_by_count(&self, count: usize) -> ZotResult<Vec<Item>> {
        let mut stmt = self
            .conn
            .prepare_cached(
                "SELECT itemID FROM items
                 WHERE libraryID = ?1
                 ORDER BY dateAdded DESC
                 LIMIT ?2",
            )
            .map_err(sql_err("recent-items-by-count"))?;
        let rows = stmt
            .query_map(params![self.library_id, count as i64], |row| {
                row.get::<_, i64>(0)
            })
            .map_err(sql_err("recent-items-by-count"))?;
        let item_ids = rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(sql_err("recent-items-by-count"))?;
        self.get_items_batch(&item_ids)
    }

    pub fn get_trash_items(&self, limit: usize) -> ZotResult<Vec<Item>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT i.itemID FROM items i JOIN deletedItems d ON i.itemID = d.itemID WHERE i.libraryID = ?1 ORDER BY d.dateDeleted DESC LIMIT ?2",
        )
        .map_err(sql_err("trash-items"))?;
        let rows = stmt
            .query_map(params![self.library_id, limit as i64], |row| {
                row.get::<_, i64>(0)
            })
            .map_err(sql_err("trash-items"))?;
        let item_ids = rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(sql_err("trash-items"))?;
        self.get_items_batch(&item_ids)
    }

    pub fn find_duplicates(
        &self,
        method: DuplicateMatchMethod,
        collection: Option<&str>,
        limit: usize,
        candidate_budget: usize,
    ) -> ZotResult<DuplicateScanResult> {
        if candidate_budget == 0 {
            return Err(ZotError::InvalidInput {
                code: "duplicate-candidate-budget".to_string(),
                message: "Duplicate candidate budget must be greater than zero".to_string(),
                hint: Some("Pass --candidate-budget with a positive integer".to_string()),
            });
        }

        let collection_id = collection
            .map(|value| self.resolve_collection_id(value))
            .transpose()?;
        let (scope_filter, scope_values) = duplicate_scope(collection_id, self.library_id);
        let mut raw_groups = Vec::<(&'static str, f32, Vec<i64>)>::new();

        if matches!(
            method,
            DuplicateMatchMethod::Doi | DuplicateMatchMethod::Both
        ) {
            let sql = format!(
                "SELECT LOWER(TRIM(iv.value)), GROUP_CONCAT(i.itemID, ',')
                 FROM items i
                 JOIN itemTypes it ON i.itemTypeID = it.itemTypeID
                 JOIN itemData id ON i.itemID = id.itemID
                 JOIN fields f ON id.fieldID = f.fieldID
                 JOIN itemDataValues iv ON id.valueID = iv.valueID
                 WHERE {scope_filter} AND f.fieldName = 'DOI' AND TRIM(iv.value) <> ''
                 GROUP BY LOWER(TRIM(iv.value)) HAVING COUNT(*) > 1
                 ORDER BY LOWER(TRIM(iv.value))"
            );
            let mut stmt = self
                .conn
                .prepare_cached(&sql)
                .map_err(sql_err("duplicates-doi-groups"))?;
            let rows = stmt
                .query_map(params_from_iter(scope_values.iter()), |row| {
                    row.get::<_, String>(1)
                })
                .map_err(sql_err("duplicates-doi-groups"))?;
            for row in rows {
                let ids = row
                    .map_err(sql_err("duplicates-doi-groups"))?
                    .split(',')
                    .filter_map(|value| value.parse::<i64>().ok())
                    .collect::<Vec<_>>();
                if ids.len() > 1 {
                    raw_groups.push(("doi", 1.0, ids));
                }
            }
        }

        let title_candidates =
            self.load_duplicate_title_candidates(&scope_filter, &scope_values)?;
        let mut candidate_pair_count = 0usize;
        let mut skipped_oversize_blocks = 0usize;
        let mut truncated = false;
        if matches!(
            method,
            DuplicateMatchMethod::Title | DuplicateMatchMethod::Both
        ) {
            let (components, scan_meta) = title_duplicate_components(
                &title_candidates,
                candidate_budget,
                DUPLICATE_TITLE_THRESHOLD,
            );
            candidate_pair_count = scan_meta.candidate_pair_count;
            skipped_oversize_blocks = scan_meta.skipped_oversize_blocks;
            truncated = scan_meta.truncated;
            raw_groups.extend(
                components
                    .into_iter()
                    .map(|ids| ("title", DUPLICATE_TITLE_THRESHOLD as f32, ids)),
            );
        }

        raw_groups.truncate(limit);
        let all_ids = raw_groups
            .iter()
            .flat_map(|(_, _, ids)| ids.iter().copied())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let items = self.get_items_batch(&all_ids)?;
        let by_id = all_ids.into_iter().zip(items).collect::<HashMap<_, _>>();
        let groups = raw_groups
            .into_iter()
            .filter_map(|(match_type, score, ids)| {
                let mut items = ids
                    .into_iter()
                    .filter_map(|item_id| by_id.get(&item_id).cloned())
                    .collect::<Vec<_>>();
                items.sort_by(|left, right| left.key.cmp(&right.key));
                (items.len() > 1).then_some(DuplicateGroup {
                    match_type: match_type.to_string(),
                    score,
                    items,
                })
            })
            .collect();

        Ok(DuplicateScanResult {
            groups,
            scanned_count: title_candidates.len(),
            candidate_pair_count,
            skipped_oversize_blocks,
            threshold: DUPLICATE_TITLE_THRESHOLD as f32,
            candidate_budget,
            truncated,
        })
    }

    fn load_duplicate_title_candidates(
        &self,
        scope_filter: &str,
        scope_values: &[rusqlite::types::Value],
    ) -> ZotResult<Vec<DuplicateTitleCandidate>> {
        let sql = format!(
            "SELECT i.itemID,
                    COALESCE((SELECT iv.value FROM itemData id JOIN fields f ON id.fieldID = f.fieldID JOIN itemDataValues iv ON id.valueID = iv.valueID WHERE id.itemID = i.itemID AND f.fieldName = 'title' LIMIT 1), ''),
                    COALESCE((SELECT iv.value FROM itemData id JOIN fields f ON id.fieldID = f.fieldID JOIN itemDataValues iv ON id.valueID = iv.valueID WHERE id.itemID = i.itemID AND f.fieldName = 'date' LIMIT 1), ''),
                    COALESCE((SELECT c.lastName FROM itemCreators ic JOIN creators c ON ic.creatorID = c.creatorID WHERE ic.itemID = i.itemID ORDER BY ic.orderIndex LIMIT 1), '')
             FROM items i JOIN itemTypes it ON i.itemTypeID = it.itemTypeID
             WHERE {scope_filter} ORDER BY i.key"
        );
        let mut stmt = self
            .conn
            .prepare_cached(&sql)
            .map_err(sql_err("duplicates-title-candidates"))?;
        let rows = stmt
            .query_map(params_from_iter(scope_values.iter()), |row| {
                Ok(DuplicateTitleCandidate {
                    item_id: row.get(0)?,
                    title: row.get(1)?,
                    year: year_bucket(&row.get::<_, String>(2)?),
                    author: normalize_block_value(&row.get::<_, String>(3)?),
                })
            })
            .map_err(sql_err("duplicates-title-candidates"))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(sql_err("duplicates-title-candidates"))
    }

    /// Rank items related to `key`. This method only fetches raw signals
    /// (explicit relation pairs plus shared creator/tag/collection counts);
    /// the weights and the ranking itself live in [`crate::graph::score_pair`]
    /// so `zot related` and `zot graph` share one relatedness definition.
    pub fn get_related_items(&self, key: &str, limit: usize) -> ZotResult<Vec<Item>> {
        let parent_id = self.parent_item_id(key)?;
        let Some(parent_id) = parent_id else {
            return Ok(Vec::new());
        };
        let mut signals: HashMap<i64, PairAccum> = HashMap::new();

        let mut stmt = self
            .conn
            .prepare_cached(
                "SELECT object FROM itemRelations WHERE itemID = ?1 AND predicateID = 1",
            )
            .map_err(sql_err("related-explicit"))?;
        let rows = stmt
            .query_map(params![parent_id], |row| row.get::<_, String>(0))
            .map_err(sql_err("related-explicit"))?;
        for row in rows {
            let object = row.map_err(sql_err("related-explicit"))?;
            let related_key = object.rsplit('/').next().unwrap_or_default();
            if !related_key.is_empty()
                && let Some(item_id) = self.item_id_by_key(related_key)?
            {
                signals.entry(item_id).or_default().related = true;
            }
        }

        let my_creator_ids = self.get_item_creator_ids(parent_id)?;
        for (item_id, count) in self.count_shared_ids(
            "itemCreators",
            "creatorID",
            parent_id,
            &my_creator_ids,
            "related-coauthors",
        )? {
            signals.entry(item_id).or_default().coauthor = count;
        }

        let my_collections = self.get_item_collection_ids(parent_id)?;
        for (item_id, count) in self.count_shared_ids(
            "collectionItems",
            "collectionID",
            parent_id,
            &my_collections,
            "related-collections",
        )? {
            signals.entry(item_id).or_default().collection = count;
        }

        // No `HAVING cnt >= 2` here any more: thresholds are scoring policy,
        // and scoring is owned by `graph::score_pair` (a single shared tag now
        // scores TAG_WEIGHT instead of being dropped at fetch time).
        let my_tag_ids = self.get_item_tag_ids(parent_id)?;
        for (item_id, count) in
            self.count_shared_ids("itemTags", "tagID", parent_id, &my_tag_ids, "related-tags")?
        {
            signals.entry(item_id).or_default().tag = count;
        }

        let mut ordered = signals
            .into_iter()
            .map(|(item_id, pair)| (item_id, score_pair(&pair)))
            .collect::<Vec<_>>();
        // Score desc, then itemID asc so equal-score candidates rank
        // deterministically (HashMap iteration order is arbitrary).
        ordered.sort_by_key(|&(item_id, score)| (Reverse(score), item_id));
        let item_ids = ordered
            .into_iter()
            .take(limit)
            .map(|(item_id, _)| item_id)
            .collect::<Vec<_>>();
        self.get_items_batch(&item_ids)
    }

    /// Build a local relationship knowledge graph for the current scope (whole
    /// library, or a single collection when `opts.collection` is set). Node and
    /// edge data come from existing batched loaders; assembly and structural
    /// analysis live in [`crate::graph`].
    pub fn build_knowledge_graph(&self, opts: &GraphOptions) -> ZotResult<KnowledgeGraph> {
        if opts.edge_budget == 0 {
            return Err(ZotError::InvalidInput {
                code: "graph-edge-budget".to_string(),
                message: "Graph edge budget must be greater than zero".to_string(),
                hint: Some("Pass --edge-budget with a positive integer".to_string()),
            });
        }
        let unbounded_limit = usize::try_from(i64::MAX).unwrap_or(usize::MAX);
        let items = self.list_items(opts.collection.as_deref(), unbounded_limit, 0)?;
        let explicit = if opts.relations.related {
            self.load_explicit_relations()?
        } else {
            Vec::new()
        };
        let scope = match &opts.collection {
            Some(key) => format!("collection:{key}"),
            None => format!("library:{}", self.scope_label()),
        };
        Ok(crate::graph::assemble_graph(&items, &explicit, opts, scope))
    }

    fn scope_label(&self) -> String {
        match &self.library_scope {
            LibraryScope::User => "user".to_string(),
            LibraryScope::Group { group_id } => format!("group-{group_id}"),
        }
    }

    /// Load explicit Zotero relations (`dc:relation`, predicateID 1) for the
    /// current library as `(source_key, target_key)` pairs. The related item's
    /// key is the final path segment of the stored relation URI.
    fn load_explicit_relations(&self) -> ZotResult<Vec<(String, String)>> {
        let mut stmt = self
            .conn
            .prepare_cached(
                "SELECT i.key, ir.object FROM itemRelations ir
                 JOIN items i ON ir.itemID = i.itemID
                 WHERE ir.predicateID = 1 AND i.libraryID = ?1",
            )
            .map_err(sql_err("graph-relations"))?;
        let rows = stmt
            .query_map(params![self.library_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(sql_err("graph-relations"))?;
        let mut relations = Vec::new();
        for row in rows {
            let (source_key, object) = row.map_err(sql_err("graph-relations"))?;
            let target_key = object.rsplit('/').next().unwrap_or_default();
            if !target_key.is_empty() {
                relations.push((source_key, target_key.to_string()));
            }
        }
        Ok(relations)
    }

    pub fn get_stats(&self) -> ZotResult<LibraryStats> {
        self.get_stats_with_trashed(false)
    }

    pub fn get_stats_with_trashed(&self, include_trashed: bool) -> ZotResult<LibraryStats> {
        let item_trash = if include_trashed {
            ""
        } else {
            " AND NOT EXISTS (SELECT 1 FROM deletedItems d WHERE d.itemID = i.itemID)"
        };
        let parent_trash = if include_trashed {
            ""
        } else {
            " AND NOT EXISTS (SELECT 1 FROM deletedItems d WHERE d.itemID = pi.itemID)"
        };
        let total_items = self.stats_total_items(include_trashed)?;
        let by_type = self.stats_grouped(
            &format!(
                "SELECT it.typeName, COUNT(*) FROM items i
             JOIN itemTypes it ON i.itemTypeID = it.itemTypeID
             WHERE i.libraryID = ?1
             AND it.typeName NOT IN ('attachment','note','annotation'){item_trash}
             GROUP BY it.typeName"
            ),
            "stats-by-type",
        )?;
        let top_tags = self.stats_grouped(
            &format!(
                "SELECT t.name, COUNT(*) FROM itemTags itg
             JOIN tags t ON itg.tagID = t.tagID
             JOIN items i ON itg.itemID = i.itemID
             JOIN itemTypes it ON i.itemTypeID = it.itemTypeID
             WHERE i.libraryID = ?1
             AND it.typeName NOT IN ('attachment','note','annotation'){item_trash}
             GROUP BY t.tagID, t.name"
            ),
            "stats-tags",
        )?;
        let collections = self.stats_grouped(
            &format!(
                "SELECT c.key, COUNT(*) FROM collectionItems ci
             JOIN collections c ON ci.collectionID = c.collectionID
             JOIN items i ON ci.itemID = i.itemID
             JOIN itemTypes it ON i.itemTypeID = it.itemTypeID
             WHERE i.libraryID = ?1
             AND it.typeName NOT IN ('attachment','note','annotation'){item_trash}
             GROUP BY c.key"
            ),
            "stats-collections",
        )?;
        let pdf_attachments = self.stats_count(
            &format!(
                "SELECT COUNT(DISTINCT ia.parentItemID) FROM itemAttachments ia
             JOIN items pi ON ia.parentItemID = pi.itemID
             JOIN itemTypes it ON pi.itemTypeID = it.itemTypeID
             WHERE pi.libraryID = ?1
             AND ia.contentType = 'application/pdf'
             AND it.typeName NOT IN ('attachment','note','annotation'){parent_trash}"
            ),
            "stats-pdf-attachments",
        )?;
        let notes = self.stats_count(
            &format!(
                "SELECT COUNT(*) FROM itemNotes n
             JOIN items pi ON n.parentItemID = pi.itemID
             JOIN itemTypes it ON pi.itemTypeID = it.itemTypeID
             WHERE pi.libraryID = ?1
             AND it.typeName NOT IN ('attachment','note','annotation'){parent_trash}"
            ),
            "stats-notes",
        )?;
        Ok(LibraryStats {
            total_items,
            by_type,
            top_tags,
            collections,
            pdf_attachments,
            notes,
        })
    }

    fn stats_total_items(&self, include_trashed: bool) -> ZotResult<usize> {
        let trash = if include_trashed {
            ""
        } else {
            " AND NOT EXISTS (SELECT 1 FROM deletedItems d WHERE d.itemID = i.itemID)"
        };
        self.stats_count(
            &format!(
                "SELECT COUNT(*) FROM items i
             JOIN itemTypes it ON i.itemTypeID = it.itemTypeID
             WHERE i.libraryID = ?1
             AND it.typeName NOT IN ('attachment','note','annotation'){trash}"
            ),
            "stats-total",
        )
    }

    fn stats_count(&self, sql: &str, context: &'static str) -> ZotResult<usize> {
        self.conn
            .prepare_cached(sql)
            .map_err(sql_err(context))?
            .query_row(params![self.library_id], |row| row.get::<_, i64>(0))
            .map(|n| n as usize)
            .map_err(sql_err(context))
    }

    fn stats_grouped(
        &self,
        sql: &str,
        context: &'static str,
    ) -> ZotResult<BTreeMap<String, usize>> {
        let mut stmt = self.conn.prepare_cached(sql).map_err(sql_err(context))?;
        let rows = stmt
            .query_map(params![self.library_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as usize))
            })
            .map_err(sql_err(context))?;
        let mut map = BTreeMap::new();
        for row in rows {
            let (name, count) = row.map_err(sql_err(context))?;
            map.insert(name, count);
        }
        Ok(map)
    }

    pub fn export_citation(&self, key: &str, format: &str) -> ZotResult<Option<String>> {
        let Some(item) = self.get_item(key)? else {
            return Ok(None);
        };
        Ok(Some(export_item(&item, format)?))
    }

    pub fn get_arxiv_preprints(
        &self,
        collection: Option<&str>,
        limit: usize,
    ) -> ZotResult<Vec<Item>> {
        let mut items = self.list_items(collection, 10_000, 0)?;
        items.retain(|item| {
            let candidate = [
                item.url.as_deref().unwrap_or_default(),
                item.doi.as_deref().unwrap_or_default(),
                item.extra
                    .get("extra")
                    .map(String::as_str)
                    .unwrap_or_default(),
            ]
            .join(" ")
            .to_lowercase();
            item.item_type == "preprint"
                || candidate.contains("arxiv")
                || candidate.contains("biorxiv")
                || candidate.contains("medrxiv")
                || candidate.contains("10.1101/")
        });
        items.truncate(limit);
        Ok(items)
    }

    fn connect(db_path: &Path) -> ZotResult<(Connection, TempDir, LibrarySnapshotMeta)> {
        Self::connect_with_policy(db_path, SnapshotPolicy::default())
    }

    fn connect_with_policy(
        db_path: &Path,
        policy: SnapshotPolicy,
    ) -> ZotResult<(Connection, TempDir, LibrarySnapshotMeta)> {
        let source_modified_at = fs::metadata(db_path)
            .ok()
            .and_then(|metadata| metadata.modified().ok())
            .map(|modified| DateTime::<Utc>::from(modified).to_rfc3339());
        let source = Connection::open_with_flags(db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(snapshot_sql_err("open-zotero-db"))?;
        source
            .busy_timeout(policy.busy_timeout)
            .map_err(snapshot_sql_err("open-zotero-db"))?;

        let temp_dir = tempfile::tempdir().map_err(|source| ZotError::Io {
            path: db_path.to_path_buf(),
            source,
        })?;
        let snapshot_path = temp_dir.path().join("zotero.sqlite");
        let mut destination =
            Connection::open(&snapshot_path).map_err(snapshot_sql_err("snapshot-zotero-db"))?;
        run_snapshot_backup(&source, &mut destination, policy)?;
        drop(destination);
        drop(source);

        let snapshot =
            Connection::open_with_flags(&snapshot_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
                .map_err(snapshot_sql_err("open-zotero-snapshot"))?;
        validate_snapshot(&snapshot)?;
        let schema_version = snapshot_schema_version(&snapshot)?;
        let snapshot_meta = LibrarySnapshotMeta {
            source_modified_at,
            snapshot_created_at: Utc::now().to_rfc3339(),
            schema_version,
        };
        Ok((snapshot, temp_dir, snapshot_meta))
    }

    fn resolve_library_id(&self) -> ZotResult<i64> {
        match self.library_scope {
            LibraryScope::User => Ok(1),
            LibraryScope::Group { group_id } => self
                .resolve_group_library_id(group_id)?
                .ok_or_else(|| ZotError::Database {
                    code: "group-not-found".to_string(),
                    message: format!("Group '{group_id}' not found in local database"),
                    hint: None,
                }),
        }
    }

    fn resolve_collection_id(&self, collection: &str) -> ZotResult<i64> {
        if let Some(collection_id) = self
            .conn
            .query_row(
                "SELECT collectionID FROM collections WHERE libraryID = ?1 AND key = ?2",
                params![self.library_id, collection],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map_err(sql_err("resolve-collection-key"))?
        {
            return Ok(collection_id);
        }

        let mut stmt = self
            .conn
            .prepare_cached(
                "SELECT collectionID, key FROM collections WHERE libraryID = ?1 AND collectionName = ?2 ORDER BY key",
            )
            .map_err(sql_err("resolve-collection-name"))?;
        let matches = stmt
            .query_map(params![self.library_id, collection], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(sql_err("resolve-collection-name"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(sql_err("resolve-collection-name"))?;
        match matches.as_slice() {
            [(collection_id, _)] => Ok(*collection_id),
            [] => Err(ZotError::InvalidInput {
                code: "collection-not-found".to_string(),
                message: format!("Collection '{collection}' not found"),
                hint: Some("Use 'zot collection list' to inspect collection names".to_string()),
            }),
            candidates => Err(ZotError::InvalidInput {
                code: "collection-ambiguous".to_string(),
                message: format!("Collection name '{collection}' matches multiple collections"),
                hint: Some(format!(
                    "Use one of these collection keys: {}",
                    candidates
                        .iter()
                        .map(|(_, key)| key.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                )),
            }),
        }
    }

    fn item_id_by_key(&self, key: &str) -> ZotResult<Option<i64>> {
        self.conn
            .query_row(
                "SELECT itemID FROM items WHERE key = ?1 AND libraryID = ?2",
                params![key, self.library_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map_err(sql_err("item-id-by-key"))
    }

    fn parent_item_id(&self, key: &str) -> ZotResult<Option<i64>> {
        self.item_id_by_key(key)
    }

    fn get_item_tag_ids(&self, item_id: i64) -> ZotResult<Vec<i64>> {
        let mut stmt = self
            .conn
            .prepare_cached("SELECT tagID FROM itemTags WHERE itemID = ?1")
            .map_err(sql_err("item-tag-ids"))?;
        let rows = stmt
            .query_map(params![item_id], |row| row.get::<_, i64>(0))
            .map_err(sql_err("item-tag-ids"))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(sql_err("item-tag-ids"))
    }

    fn get_item_collection_ids(&self, item_id: i64) -> ZotResult<Vec<i64>> {
        let mut stmt = self
            .conn
            .prepare_cached("SELECT collectionID FROM collectionItems WHERE itemID = ?1")
            .map_err(sql_err("item-collection-ids"))?;
        let rows = stmt
            .query_map(params![item_id], |row| row.get::<_, i64>(0))
            .map_err(sql_err("item-collection-ids"))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(sql_err("item-collection-ids"))
    }

    fn get_item_creator_ids(&self, item_id: i64) -> ZotResult<Vec<i64>> {
        let mut stmt = self
            .conn
            .prepare_cached("SELECT creatorID FROM itemCreators WHERE itemID = ?1")
            .map_err(sql_err("item-creator-ids"))?;
        let rows = stmt
            .query_map(params![item_id], |row| row.get::<_, i64>(0))
            .map_err(sql_err("item-creator-ids"))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(sql_err("item-creator-ids"))
    }

    /// Shared-signal counting for `get_related_items`: given the parent
    /// item's own `ids` in `table`.`id_column` (its tagIDs, collectionIDs, or
    /// creatorIDs), count per other primary item how many of those ids it
    /// shares. `table` and `id_column` are code constants, never user input.
    fn count_shared_ids(
        &self,
        table: &str,
        id_column: &str,
        parent_id: i64,
        ids: &[i64],
        label: &'static str,
    ) -> ZotResult<Vec<(i64, usize)>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        // `?1` (parent) must precede the plain-`?` IN list: SQLite numbers a
        // plain `?` as one-greater-than-the-largest index seen so far, so an
        // `IN (?,...)`-first clause would make a trailing `?1` alias the
        // first IN slot and the bind count would no longer match.
        // `COUNT(DISTINCT ...)` mirrors graph.rs, where a creator credited
        // twice on one item (e.g. author and editor) counts once.
        // Child rows (attachment/note/annotation) are excluded up front:
        // `get_items_batch` can never return them, so letting them into the
        // candidate list would only burn `limit` slots — and the graph never
        // scores them either, since its nodes exclude child types.
        let sql = format!(
            "SELECT ti.itemID, COUNT(DISTINCT ti.{id_column}) as cnt FROM {table} ti
             JOIN items i ON ti.itemID = i.itemID
             JOIN itemTypes it ON i.itemTypeID = it.itemTypeID
             WHERE ti.itemID != ?1
             AND it.typeName NOT IN ('attachment','note','annotation')
             AND ti.{id_column} IN ({})
             GROUP BY ti.itemID",
            repeat_placeholders(ids.len())
        );
        let mut params_vec = vec![rusqlite::types::Value::from(parent_id)];
        params_vec.extend(ids.iter().copied().map(rusqlite::types::Value::from));
        let mut stmt = self.conn.prepare_cached(&sql).map_err(sql_err(label))?;
        let rows = stmt
            .query_map(params_from_iter(params_vec), |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
            })
            .map_err(sql_err(label))?;
        let mut counts = Vec::new();
        for row in rows {
            let (item_id, count) = row.map_err(sql_err(label))?;
            // COUNT(...) is never negative; saturate defensively.
            counts.push((item_id, usize::try_from(count).unwrap_or(0)));
        }
        Ok(counts)
    }

    fn get_items_batch(&self, item_ids: &[i64]) -> ZotResult<Vec<Item>> {
        if item_ids.is_empty() {
            return Ok(Vec::new());
        }

        const CHUNK: usize = 500;

        let mut items_by_id = HashMap::new();
        for chunk in item_ids.chunks(CHUNK) {
            let base_rows = self.load_item_rows_batch(chunk)?;
            let chunk_ids = base_rows.keys().copied().collect::<Vec<_>>();
            let fields_by_id = self.load_item_fields_batch(&chunk_ids)?;
            let creators_by_id = self.load_item_creators_batch(&chunk_ids)?;
            let tags_by_id = self.load_item_tags_batch(&chunk_ids)?;
            let collections_by_id = self.load_item_collection_keys_batch(&chunk_ids)?;

            for (item_id, base) in base_rows {
                let fields = fields_by_id.get(&item_id).cloned().unwrap_or_default();
                let creators = creators_by_id.get(&item_id).cloned().unwrap_or_default();
                let tags = tags_by_id.get(&item_id).cloned().unwrap_or_default();
                let collections = collections_by_id.get(&item_id).cloned().unwrap_or_default();

                items_by_id.insert(
                    item_id,
                    Item {
                        key: base.key,
                        item_type: base.item_type,
                        title: fields.get("title").cloned().unwrap_or_default(),
                        creators,
                        abstract_note: fields.get("abstractNote").cloned(),
                        date: fields.get("date").cloned(),
                        url: fields.get("url").cloned(),
                        doi: fields.get("DOI").cloned(),
                        tags,
                        collections,
                        date_added: base.date_added,
                        date_modified: base.date_modified,
                        extra: fields
                            .into_iter()
                            .filter(|(field, _)| {
                                !matches!(
                                    field.as_str(),
                                    "title" | "abstractNote" | "date" | "url" | "DOI"
                                )
                            })
                            .collect(),
                    },
                );
            }
        }

        let mut seen = HashSet::new();
        let mut items = Vec::with_capacity(items_by_id.len());
        for item_id in item_ids {
            if seen.insert(*item_id)
                && let Some(item) = items_by_id.remove(item_id)
            {
                items.push(item);
            }
        }
        Ok(items)
    }

    fn load_item_rows_batch(&self, item_ids: &[i64]) -> ZotResult<HashMap<i64, BatchItemRow>> {
        if item_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let placeholders = repeat_placeholders(item_ids.len());
        let sql = format!(
            "SELECT i.itemID, i.key, it.typeName, i.dateAdded, i.dateModified
             FROM items i
             JOIN itemTypes it ON i.itemTypeID = it.itemTypeID
             WHERE i.libraryID = ? AND i.itemID IN ({placeholders})"
        );
        let mut params = Vec::with_capacity(item_ids.len() + 1);
        params.push(rusqlite::types::Value::from(self.library_id));
        params.extend(item_ids.iter().copied().map(rusqlite::types::Value::from));

        let mut stmt = self
            .conn
            .prepare_cached(&sql)
            .map_err(sql_err("item-rows-batch"))?;
        let rows = stmt
            .query_map(params_from_iter(params.iter()), |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    BatchItemRow {
                        key: row.get::<_, String>(1)?,
                        item_type: row.get::<_, String>(2)?,
                        date_added: row.get::<_, Option<String>>(3)?,
                        date_modified: row.get::<_, Option<String>>(4)?,
                    },
                ))
            })
            .map_err(sql_err("item-rows-batch"))?;

        let mut batch = HashMap::new();
        for row in rows {
            let (item_id, entry) = row.map_err(sql_err("item-rows-batch"))?;
            if EXCLUDED_TYPE_NAMES.contains(&entry.item_type.as_str()) {
                continue;
            }
            batch.insert(item_id, entry);
        }
        Ok(batch)
    }

    fn load_item_fields_batch(
        &self,
        item_ids: &[i64],
    ) -> ZotResult<HashMap<i64, BTreeMap<String, String>>> {
        if item_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let placeholders = repeat_placeholders(item_ids.len());
        let sql = format!(
            "SELECT id.itemID, f.fieldName, iv.value
             FROM itemData id
             JOIN fields f ON id.fieldID = f.fieldID
             JOIN itemDataValues iv ON id.valueID = iv.valueID
             WHERE id.itemID IN ({placeholders})"
        );
        let mut stmt = self
            .conn
            .prepare_cached(&sql)
            .map_err(sql_err("item-fields-batch"))?;
        let rows = stmt
            .query_map(params_from_iter(item_ids.iter()), |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(sql_err("item-fields-batch"))?;

        let mut fields = HashMap::<i64, BTreeMap<String, String>>::new();
        for row in rows {
            let (item_id, field, value) = row.map_err(sql_err("item-fields-batch"))?;
            fields.entry(item_id).or_default().insert(field, value);
        }
        Ok(fields)
    }

    fn load_item_creators_batch(&self, item_ids: &[i64]) -> ZotResult<HashMap<i64, Vec<Creator>>> {
        if item_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let placeholders = repeat_placeholders(item_ids.len());
        let sql = format!(
            "SELECT ic.itemID, c.firstName, c.lastName, ct.creatorType
             FROM itemCreators ic
             JOIN creators c ON ic.creatorID = c.creatorID
             JOIN creatorTypes ct ON ic.creatorTypeID = ct.creatorTypeID
             WHERE ic.itemID IN ({placeholders})
             ORDER BY ic.itemID, ic.orderIndex"
        );
        let mut stmt = self
            .conn
            .prepare_cached(&sql)
            .map_err(sql_err("item-creators-batch"))?;
        let rows = stmt
            .query_map(params_from_iter(item_ids.iter()), |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    Creator {
                        first_name: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                        last_name: row.get::<_, String>(2)?,
                        creator_type: row.get::<_, String>(3)?,
                    },
                ))
            })
            .map_err(sql_err("item-creators-batch"))?;

        let mut creators = HashMap::<i64, Vec<Creator>>::new();
        for row in rows {
            let (item_id, creator) = row.map_err(sql_err("item-creators-batch"))?;
            creators.entry(item_id).or_default().push(creator);
        }
        Ok(creators)
    }

    fn load_item_tags_batch(&self, item_ids: &[i64]) -> ZotResult<HashMap<i64, Vec<String>>> {
        if item_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let placeholders = repeat_placeholders(item_ids.len());
        let sql = format!(
            "SELECT it.itemID, t.name
             FROM itemTags it
             JOIN tags t ON it.tagID = t.tagID
             WHERE it.itemID IN ({placeholders})
             ORDER BY it.itemID, t.name ASC"
        );
        let mut stmt = self
            .conn
            .prepare_cached(&sql)
            .map_err(sql_err("item-tags-batch"))?;
        let rows = stmt
            .query_map(params_from_iter(item_ids.iter()), |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(sql_err("item-tags-batch"))?;

        let mut tags = HashMap::<i64, Vec<String>>::new();
        for row in rows {
            let (item_id, tag) = row.map_err(sql_err("item-tags-batch"))?;
            tags.entry(item_id).or_default().push(tag);
        }
        Ok(tags)
    }

    fn load_item_collection_keys_batch(
        &self,
        item_ids: &[i64],
    ) -> ZotResult<HashMap<i64, Vec<String>>> {
        if item_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let placeholders = repeat_placeholders(item_ids.len());
        let sql = format!(
            "SELECT ci.itemID, c.key
             FROM collectionItems ci
             JOIN collections c ON ci.collectionID = c.collectionID
             WHERE ci.itemID IN ({placeholders})
             ORDER BY ci.itemID, c.collectionName ASC"
        );
        let mut stmt = self
            .conn
            .prepare_cached(&sql)
            .map_err(sql_err("item-collection-keys-batch"))?;
        let rows = stmt
            .query_map(params_from_iter(item_ids.iter()), |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(sql_err("item-collection-keys-batch"))?;

        let mut collections = HashMap::<i64, Vec<String>>::new();
        for row in rows {
            let (item_id, collection_key) = row.map_err(sql_err("item-collection-keys-batch"))?;
            collections.entry(item_id).or_default().push(collection_key);
        }
        Ok(collections)
    }

    fn get_item_by_id(&self, item_id: i64) -> ZotResult<Item> {
        let row = self
            .conn
            .query_row(
                "SELECT key, itemTypeID, dateAdded, dateModified FROM items WHERE itemID = ?1 AND libraryID = ?2",
                params![item_id, self.library_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, Option<String>>(3)?,
                    ))
                },
            )
            .map_err(sql_err("get-item-by-id"))?;

        let (key, item_type_id, date_added, date_modified) = row;
        let item_type = self
            .conn
            .query_row(
                "SELECT typeName FROM itemTypes WHERE itemTypeID = ?1",
                params![item_type_id],
                |row| row.get::<_, String>(0),
            )
            .map_err(sql_err("item-type-name"))?;
        let fields = self.get_item_fields(item_id)?;
        let creators = self.get_item_creators(item_id)?;
        let tags = self.get_item_tags(item_id)?;
        let collections = self.get_item_collection_keys(item_id)?;
        Ok(Item {
            key,
            item_type,
            title: fields.get("title").cloned().unwrap_or_default(),
            creators,
            abstract_note: fields.get("abstractNote").cloned(),
            date: fields.get("date").cloned(),
            url: fields.get("url").cloned(),
            doi: fields.get("DOI").cloned(),
            tags,
            collections,
            date_added,
            date_modified,
            extra: fields
                .into_iter()
                .filter(|(field, _)| {
                    !matches!(
                        field.as_str(),
                        "title" | "abstractNote" | "date" | "url" | "DOI"
                    )
                })
                .collect(),
        })
    }

    fn get_item_fields(&self, item_id: i64) -> ZotResult<BTreeMap<String, String>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT f.fieldName, iv.value FROM itemData id JOIN fields f ON id.fieldID = f.fieldID JOIN itemDataValues iv ON id.valueID = iv.valueID WHERE id.itemID = ?1",
        )
        .map_err(sql_err("item-fields"))?;
        let rows = stmt
            .query_map(params![item_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(sql_err("item-fields"))?;
        let mut fields = BTreeMap::new();
        for row in rows {
            let (field, value) = row.map_err(sql_err("item-fields"))?;
            fields.insert(field, value);
        }
        Ok(fields)
    }

    fn get_item_creators(&self, item_id: i64) -> ZotResult<Vec<Creator>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT c.firstName, c.lastName, ct.creatorType FROM itemCreators ic JOIN creators c ON ic.creatorID = c.creatorID JOIN creatorTypes ct ON ic.creatorTypeID = ct.creatorTypeID WHERE ic.itemID = ?1 ORDER BY ic.orderIndex",
        )
        .map_err(sql_err("item-creators"))?;
        let rows = stmt
            .query_map(params![item_id], |row| {
                Ok(Creator {
                    first_name: row.get::<_, Option<String>>(0)?.unwrap_or_default(),
                    last_name: row.get::<_, String>(1)?,
                    creator_type: row.get::<_, String>(2)?,
                })
            })
            .map_err(sql_err("item-creators"))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(sql_err("item-creators"))
    }

    fn get_item_tags(&self, item_id: i64) -> ZotResult<Vec<String>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT t.name FROM itemTags it JOIN tags t ON it.tagID = t.tagID WHERE it.itemID = ?1 ORDER BY t.name ASC",
        )
        .map_err(sql_err("item-tags"))?;
        let rows = stmt
            .query_map(params![item_id], |row| row.get::<_, String>(0))
            .map_err(sql_err("item-tags"))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(sql_err("item-tags"))
    }

    fn get_item_collection_keys(&self, item_id: i64) -> ZotResult<Vec<String>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT c.key FROM collectionItems ci JOIN collections c ON ci.collectionID = c.collectionID WHERE ci.itemID = ?1 ORDER BY c.collectionName ASC",
        )
        .map_err(sql_err("item-collection-keys"))?;
        let rows = stmt
            .query_map(params![item_id], |row| row.get::<_, String>(0))
            .map_err(sql_err("item-collection-keys"))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(sql_err("item-collection-keys"))
    }

    fn table_exists(&self, table: &str) -> ZotResult<bool> {
        self.conn
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1",
                params![table],
                |_| Ok(()),
            )
            .optional()
            .map(|value| value.is_some())
            .map_err(sql_err("table-exists"))
    }

    fn field_id(&self, field_name: &str) -> ZotResult<Option<i64>> {
        self.conn
            .query_row(
                "SELECT fieldID FROM fields WHERE fieldName = ?1",
                params![field_name],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map_err(sql_err("field-id"))
    }

    fn get_note_children(&self, key: &str) -> ZotResult<Vec<ChildNote>> {
        Ok(self
            .get_notes(key)?
            .into_iter()
            .map(|note| ChildNote {
                key: note.key,
                parent_key: Some(note.parent_key),
                content: note.content,
                tags: note.tags,
            })
            .collect())
    }

    fn get_attachment_children(&self, key: &str) -> ZotResult<Vec<ChildAttachment>> {
        Ok(self
            .get_attachments(key)?
            .into_iter()
            .map(|attachment| ChildAttachment {
                key: attachment.key,
                parent_key: Some(attachment.parent_key),
                filename: attachment.filename,
                content_type: attachment.content_type,
                tags: Vec::new(),
            })
            .collect())
    }

    fn get_annotation_children(&self, key: &str) -> ZotResult<Vec<ChildAnnotation>> {
        if !self.table_exists("itemAnnotations")? {
            return Ok(Vec::new());
        }

        let Some(item) = self.get_item(key)? else {
            return Ok(Vec::new());
        };
        let Some(item_id) = self.item_id_by_key(&item.key)? else {
            return Ok(Vec::new());
        };

        let attachment_ids = if item.item_type == "attachment" {
            vec![item_id]
        } else {
            let mut stmt = self
                .conn
                .prepare_cached(
                    "SELECT itemID
                     FROM itemAttachments
                     WHERE parentItemID = ?1
                     AND contentType IN ('application/pdf', 'application/epub+zip', 'text/html')",
                )
                .map_err(sql_err("annotation-attachment-ids"))?;
            let rows = stmt
                .query_map(params![item_id], |row| row.get::<_, i64>(0))
                .map_err(sql_err("annotation-attachment-ids"))?;
            rows.collect::<Result<Vec<_>, _>>()
                .map_err(sql_err("annotation-attachment-ids"))?
        };

        let mut children = Vec::new();
        for attachment_id in attachment_ids {
            let attachment_key = self
                .conn
                .query_row(
                    "SELECT key FROM items WHERE itemID = ?1",
                    params![attachment_id],
                    |row| row.get::<_, String>(0),
                )
                .map_err(sql_err("annotation-attachment-key"))?;
            let mut stmt = self
                .conn
                .prepare_cached(
                    "SELECT i.key, ia.text, ia.comment, ia.color, ia.pageLabel, ia.type
                     FROM itemAnnotations ia
                     JOIN items i ON ia.itemID = i.itemID
                     WHERE ia.parentItemID = ?1
                     AND i.libraryID = ?2
                     AND i.itemID NOT IN (SELECT itemID FROM deletedItems)
                     ORDER BY i.key ASC",
                )
                .map_err(sql_err("annotation-children"))?;
            let rows = stmt
                .query_map(params![attachment_id, self.library_id], |row| {
                    Ok(ChildAnnotation {
                        key: row.get::<_, String>(0)?,
                        parent_key: Some(attachment_key.clone()),
                        annotation_type: annotation_type_name(row.get::<_, i64>(5)?),
                        text: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                        comment: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                        color: row.get::<_, Option<String>>(3)?,
                        page_label: row.get::<_, Option<String>>(4)?,
                        tags: Vec::new(),
                    })
                })
                .map_err(sql_err("annotation-children"))?;
            children.extend(
                rows.collect::<Result<Vec<_>, _>>()
                    .map_err(sql_err("annotation-children"))?,
            );
        }
        Ok(children)
    }
}

#[derive(Debug)]
struct BatchItemRow {
    key: String,
    item_type: String,
    date_added: Option<String>,
    date_modified: Option<String>,
}

#[derive(Debug, Clone)]
struct DuplicateTitleCandidate {
    item_id: i64,
    title: String,
    year: String,
    author: String,
}

#[derive(Debug, Default, PartialEq, Eq)]
struct DuplicateTitleScanMeta {
    candidate_pair_count: usize,
    skipped_oversize_blocks: usize,
    truncated: bool,
}

fn duplicate_scope(
    collection_id: Option<i64>,
    library_id: i64,
) -> (String, Vec<rusqlite::types::Value>) {
    let mut filter = "i.libraryID = ? AND it.typeName NOT IN ('attachment','note','annotation') AND NOT EXISTS (SELECT 1 FROM deletedItems d WHERE d.itemID = i.itemID)".to_string();
    let mut values = vec![rusqlite::types::Value::from(library_id)];
    if let Some(collection_id) = collection_id {
        filter.push_str(" AND EXISTS (SELECT 1 FROM collectionItems ci WHERE ci.itemID = i.itemID AND ci.collectionID = ?)");
        values.push(rusqlite::types::Value::from(collection_id));
    }
    (filter, values)
}

fn title_duplicate_components(
    candidates: &[DuplicateTitleCandidate],
    candidate_budget: usize,
    threshold: f64,
) -> (Vec<Vec<i64>>, DuplicateTitleScanMeta) {
    let mut blocks = BTreeMap::<String, Vec<&DuplicateTitleCandidate>>::new();
    for candidate in candidates {
        let normalized = normalize_title(&candidate.title);
        if normalized.is_empty() {
            continue;
        }
        let prefix = normalized
            .chars()
            .filter(|ch| !ch.is_whitespace())
            .take(DUPLICATE_TITLE_PREFIX_CHARS)
            .collect::<String>();
        blocks
            .entry(format!("prefix:{prefix}"))
            .or_default()
            .push(candidate);
        if !candidate.year.is_empty() {
            blocks
                .entry(format!("prefix-year:{prefix}|{}", candidate.year))
                .or_default()
                .push(candidate);
        }
        if !candidate.author.is_empty() {
            blocks
                .entry(format!("prefix-author:{prefix}|{}", candidate.author))
                .or_default()
                .push(candidate);
        }
    }

    let mut adjacency = BTreeMap::<i64, BTreeSet<i64>>::new();
    let mut seen_pairs = BTreeSet::new();
    let mut meta = DuplicateTitleScanMeta::default();
    for block in blocks.values() {
        if block.len() < 2 {
            continue;
        }
        let mut block_truncated = false;
        'pairs: for left in 0..block.len() {
            let normalized_left = normalize_title(&block[left].title);
            for right in (left + 1)..block.len() {
                let pair = if block[left].item_id < block[right].item_id {
                    (block[left].item_id, block[right].item_id)
                } else {
                    (block[right].item_id, block[left].item_id)
                };
                if seen_pairs.contains(&pair) {
                    continue;
                }
                if meta.candidate_pair_count >= candidate_budget {
                    meta.truncated = true;
                    block_truncated = true;
                    break 'pairs;
                }
                seen_pairs.insert(pair);
                meta.candidate_pair_count += 1;
                let normalized_right = normalize_title(&block[right].title);
                if normalized_levenshtein(&normalized_left, &normalized_right) >= threshold {
                    adjacency
                        .entry(block[left].item_id)
                        .or_default()
                        .insert(block[right].item_id);
                    adjacency
                        .entry(block[right].item_id)
                        .or_default()
                        .insert(block[left].item_id);
                }
            }
        }
        if block_truncated {
            meta.skipped_oversize_blocks += 1;
        }
    }

    let mut visited = BTreeSet::new();
    let mut components = Vec::new();
    for &start in adjacency.keys() {
        if !visited.insert(start) {
            continue;
        }
        let mut pending = vec![start];
        let mut component = Vec::new();
        while let Some(item_id) = pending.pop() {
            component.push(item_id);
            if let Some(neighbours) = adjacency.get(&item_id) {
                for &neighbour in neighbours.iter().rev() {
                    if visited.insert(neighbour) {
                        pending.push(neighbour);
                    }
                }
            }
        }
        component.sort_unstable();
        if component.len() > 1 {
            components.push(component);
        }
    }
    components.sort();
    (components, meta)
}

fn normalize_block_value(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn year_bucket(value: &str) -> String {
    let mut digits = String::new();
    for ch in value.chars() {
        if ch.is_ascii_digit() {
            digits.push(ch);
            if digits.len() == 4 {
                return digits;
            }
        } else {
            digits.clear();
        }
    }
    String::new()
}

fn normalize_title(title: &str) -> String {
    title
        .chars()
        .map(|ch| {
            if ch.is_alphanumeric() || ch.is_whitespace() {
                ch.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn annotation_type_name(value: i64) -> String {
    match value {
        1 => "highlight",
        2 => "note",
        3 => "image",
        4 => "ink",
        5 => "underline",
        _ => "unknown",
    }
    .to_string()
}

fn flatten_collection_tree(collection: &Collection, flattened: &mut Vec<Collection>) {
    flattened.push(Collection {
        key: collection.key.clone(),
        name: collection.name.clone(),
        parent_key: collection.parent_key.clone(),
        children: Vec::new(),
    });
    for child in &collection.children {
        flatten_collection_tree(child, flattened);
    }
}

fn repeat_placeholders(count: usize) -> String {
    std::iter::repeat_n("?", count)
        .collect::<Vec<_>>()
        .join(",")
}

fn html_to_text(html: &str) -> String {
    let with_breaks = html
        .replace("<br>", "\n")
        .replace("<br/>", "\n")
        .replace("<br />", "\n")
        .replace("</p>", "\n\n")
        .replace("</li>", "\n");
    if let Ok(tag_re) = Regex::new(r"<[^>]+>") {
        tag_re
            .replace_all(&with_breaks, "")
            .replace("&nbsp;", " ")
            .replace("&amp;", "&")
            .trim()
            .to_string()
    } else {
        with_breaks.trim().to_string()
    }
}

fn sql_err(context: &'static str) -> impl Fn(rusqlite::Error) -> ZotError {
    move |source| ZotError::Database {
        code: context.to_string(),
        message: source.to_string(),
        hint: None,
    }
}

fn snapshot_schema_version(conn: &Connection) -> ZotResult<Option<i64>> {
    conn.query_row(
        "SELECT version FROM version WHERE schema = 'userdata'",
        [],
        |row| row.get::<_, i64>(0),
    )
    .optional()
    .map_err(snapshot_sql_err("schema-version"))
}

fn run_snapshot_backup(
    source: &Connection,
    destination: &mut Connection,
    policy: SnapshotPolicy,
) -> ZotResult<()> {
    let backup =
        Backup::new(source, destination).map_err(snapshot_sql_err("snapshot-zotero-db"))?;
    let mut busy_since = None;
    loop {
        match backup
            .step(policy.pages_per_step)
            .map_err(snapshot_sql_err("snapshot-zotero-db"))?
        {
            StepResult::Done => return Ok(()),
            StepResult::More => {
                busy_since = None;
                thread::sleep(policy.step_pause);
            }
            StepResult::Busy | StepResult::Locked => {
                let started = busy_since.get_or_insert_with(Instant::now);
                if started.elapsed() >= policy.busy_retry_limit {
                    return Err(zotero_db_busy_error(
                        "Timed out while creating a consistent Zotero database snapshot",
                    ));
                }
                thread::sleep(policy.step_pause);
            }
            _ => {
                return Err(ZotError::Database {
                    code: "snapshot-zotero-db".to_string(),
                    message: "SQLite backup returned an unsupported step result".to_string(),
                    hint: None,
                });
            }
        }
    }
}

fn validate_snapshot(conn: &Connection) -> ZotResult<()> {
    let result = conn
        .query_row("PRAGMA quick_check", [], |row| row.get::<_, String>(0))
        .map_err(snapshot_sql_err("zotero-db-snapshot-integrity"))?;
    if result.eq_ignore_ascii_case("ok") {
        return Ok(());
    }
    Err(ZotError::Database {
        code: "zotero-db-snapshot-integrity".to_string(),
        message: format!("SQLite quick_check rejected the Zotero snapshot: {result}"),
        hint: Some(
            "Close Zotero and retry; do not use this snapshot for write decisions".to_string(),
        ),
    })
}

fn snapshot_sql_err(context: &'static str) -> impl Fn(rusqlite::Error) -> ZotError {
    move |source| {
        if matches!(
            source.sqlite_error_code(),
            Some(ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked)
        ) {
            zotero_db_busy_error(&format!("Zotero database is busy: {source}"))
        } else {
            ZotError::Database {
                code: context.to_string(),
                message: source.to_string(),
                hint: None,
            }
        }
    }
}

fn zotero_db_busy_error(message: &str) -> ZotError {
    ZotError::Database {
        code: "zotero-db-busy".to_string(),
        message: message.to_string(),
        hint: Some("Close Zotero or wait for its database write to finish, then retry".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    };
    use std::thread;
    use std::time::{Duration, Instant};

    use chrono::DateTime;
    use rusqlite::Connection;
    use tempfile::TempDir;
    use zot_core::LibraryScope;

    use super::{
        DuplicateMatchMethod, DuplicateTitleCandidate, LocalLibrary, SearchOptions, SnapshotPolicy,
        escape_like, title_duplicate_components,
    };
    use zot_core::ChildItem;

    #[test]
    fn escape_like_quotes_percent_underscore_and_backslash() {
        // F-11: every LIKE wildcard plus the escape char itself must be
        // backslash-prefixed so users searching for `50%`, `foo_bar`, or paths
        // containing `\` get literal matches when the SQL pairs LIKE ? ESCAPE '\\'.
        assert_eq!(escape_like("plain"), "plain");
        assert_eq!(escape_like("50%"), "50\\%");
        assert_eq!(escape_like("foo_bar"), "foo\\_bar");
        assert_eq!(escape_like("path\\to"), "path\\\\to");
        assert_eq!(escape_like("a%b_c\\d"), "a\\%b\\_c\\\\d");
    }

    fn create_snapshot_test_schema(conn: &Connection) {
        conn.execute_batch(
            "CREATE TABLE version (schema TEXT PRIMARY KEY, version INTEGER NOT NULL);
             INSERT INTO version VALUES ('userdata', 42);
             CREATE TABLE snapshot_totals (id INTEGER PRIMARY KEY, total INTEGER NOT NULL);
             INSERT INTO snapshot_totals VALUES (1, 0);
             CREATE TABLE snapshot_events (id INTEGER PRIMARY KEY);",
        )
        .expect("create snapshot test schema");
    }

    #[test]
    fn snapshot_reads_committed_wal_and_preserves_metadata() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("zotero.sqlite");
        let writer = Connection::open(&db_path).expect("open writer");
        writer
            .execute_batch("PRAGMA journal_mode=WAL; PRAGMA wal_autocheckpoint=0;")
            .expect("enable wal");
        create_snapshot_test_schema(&writer);
        writer
            .execute_batch(
                "PRAGMA wal_checkpoint(TRUNCATE);
                 BEGIN IMMEDIATE;
                 UPDATE snapshot_totals SET total = 1 WHERE id = 1;
                 INSERT INTO snapshot_events VALUES (1);
                 COMMIT;",
            )
            .expect("commit wal-only change");
        let wal_path = db_path.with_extension("sqlite-wal");
        let wal_size_before = fs::metadata(&wal_path).expect("wal metadata").len();
        assert!(wal_size_before > 0);

        let library = LocalLibrary::open(dir.path(), LibraryScope::User).expect("open snapshot");
        let total: i64 = library
            .conn
            .query_row(
                "SELECT total FROM snapshot_totals WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .expect("read snapshot total");
        assert_eq!(total, 1, "committed WAL content must reach the snapshot");
        assert_eq!(library.db_path(), db_path);
        assert_eq!(library.snapshot_meta().schema_version, Some(42));
        assert!(library.snapshot_meta().source_modified_at.is_some());
        DateTime::parse_from_rfc3339(&library.snapshot_meta().snapshot_created_at)
            .expect("snapshot time is RFC 3339");
        let wal_size_after = fs::metadata(&wal_path).expect("wal still exists").len();
        assert_eq!(
            wal_size_after, wal_size_before,
            "snapshot reads must not checkpoint or truncate Zotero's WAL"
        );
    }

    #[test]
    fn snapshot_lock_contention_returns_stable_busy_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("zotero.sqlite");
        let writer = Connection::open(&db_path).expect("open writer");
        create_snapshot_test_schema(&writer);
        writer
            .execute_batch(
                "BEGIN EXCLUSIVE;
                 UPDATE snapshot_totals SET total = 1 WHERE id = 1;",
            )
            .expect("hold exclusive write lock");

        let result = LocalLibrary::connect_with_policy(
            &db_path,
            SnapshotPolicy {
                busy_timeout: Duration::from_millis(10),
                busy_retry_limit: Duration::from_millis(25),
                step_pause: Duration::from_millis(1),
                pages_per_step: 1,
            },
        );
        writer.execute_batch("ROLLBACK").expect("release lock");
        let error = match result {
            Ok(_) => panic!("exclusive source lock must not produce a snapshot"),
            Err(error) => error,
        };
        let payload = error.payload();
        assert_eq!(payload.code, "zotero-db-busy");
        assert!(
            payload
                .hint
                .as_deref()
                .is_some_and(|hint| hint.contains("Close Zotero"))
        );
    }

    #[test]
    fn concurrent_writer_snapshots_preserve_cross_table_invariant() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("zotero.sqlite");
        let setup = Connection::open(&db_path).expect("open setup");
        setup
            .execute_batch("PRAGMA journal_mode=WAL; PRAGMA wal_autocheckpoint=0;")
            .expect("enable wal");
        create_snapshot_test_schema(&setup);
        drop(setup);

        let stop = Arc::new(AtomicBool::new(false));
        let writes = Arc::new(AtomicUsize::new(0));
        let writer_stop = Arc::clone(&stop);
        let writer_count = Arc::clone(&writes);
        let writer_path = db_path.clone();
        let writer = thread::spawn(move || -> Result<(), String> {
            let mut conn = Connection::open(writer_path).map_err(|error| error.to_string())?;
            conn.busy_timeout(Duration::from_secs(1))
                .map_err(|error| error.to_string())?;
            while !writer_stop.load(Ordering::Acquire) {
                let tx = conn.transaction().map_err(|error| error.to_string())?;
                let next: i64 = tx
                    .query_row(
                        "SELECT total + 1 FROM snapshot_totals WHERE id = 1",
                        [],
                        |row| row.get(0),
                    )
                    .map_err(|error| error.to_string())?;
                tx.execute("UPDATE snapshot_totals SET total = ?1 WHERE id = 1", [next])
                    .map_err(|error| error.to_string())?;
                tx.execute("INSERT INTO snapshot_events VALUES (?1)", [next])
                    .map_err(|error| error.to_string())?;
                tx.commit().map_err(|error| error.to_string())?;
                writer_count.fetch_add(1, Ordering::Release);
                thread::sleep(Duration::from_millis(1));
            }
            Ok(())
        });

        let writer_ready_deadline = Instant::now() + Duration::from_secs(2);
        while writes.load(Ordering::Acquire) < 3 && Instant::now() < writer_ready_deadline {
            thread::yield_now();
        }
        if writes.load(Ordering::Acquire) < 3 {
            stop.store(true, Ordering::Release);
            let writer_result = writer.join().expect("join stalled writer");
            panic!("writer did not become ready: {writer_result:?}");
        }
        let mut failure = None;
        for _ in 0..32 {
            let (snapshot, _snapshot_dir, _) =
                match LocalLibrary::connect_with_policy(&db_path, SnapshotPolicy::default()) {
                    Ok(snapshot) => snapshot,
                    Err(error) => {
                        failure = Some(format!("snapshot failed: {error}"));
                        break;
                    }
                };
            let total = snapshot
                .query_row(
                    "SELECT total FROM snapshot_totals WHERE id = 1",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .expect("read snapshot total");
            let events = snapshot
                .query_row("SELECT COUNT(*) FROM snapshot_events", [], |row| {
                    row.get::<_, i64>(0)
                })
                .expect("count snapshot events");
            if total != events {
                failure = Some(format!(
                    "inconsistent snapshot: total={total}, events={events}"
                ));
                break;
            }
            let quick_check: String = snapshot
                .query_row("PRAGMA quick_check", [], |row| row.get(0))
                .expect("quick_check snapshot");
            if quick_check != "ok" {
                failure = Some(format!("quick_check failed: {quick_check}"));
                break;
            }
        }
        stop.store(true, Ordering::Release);
        writer
            .join()
            .expect("join writer")
            .expect("writer stays healthy");
        assert!(writes.load(Ordering::Acquire) >= 3);
        assert!(failure.is_none(), "{}", failure.unwrap_or_default());
    }

    #[test]
    fn search_with_percent_in_query_does_not_match_arbitrary_chars() {
        // F-11 regression: the escaped LIKE pattern must not let `%` act as a
        // wildcard. ATTN001's title is "Attention Is All You Need", so a
        // search for `tion%You` would historically match across "tion ... You"
        // through the unescaped wildcard. With ESCAPE '\' the literal phrase
        // is required and cannot land on ATTN001.
        let fixture = rich_fixture_library();
        let result = match fixture.lib.search(SearchOptions {
            query: "tion%You".to_string(),
            ..SearchOptions::default()
        }) {
            Ok(result) => result,
            Err(err) => panic!("search failed: {err}"),
        };
        assert!(
            result.items.iter().all(|item| item.key != "ATTN001"),
            "literal `%` must not behave as a wildcard between substrings"
        );
    }

    struct TestFixture {
        lib: LocalLibrary,
        _dir: TempDir,
    }

    fn rich_fixture_library() -> TestFixture {
        rich_fixture_library_with_extra_sql("")
    }

    /// Build the rich fixture, then apply `extra_sql` on top (e.g. moving a
    /// seeded item into `deletedItems`) before the read-only open.
    fn rich_fixture_library_with_extra_sql(extra_sql: &str) -> TestFixture {
        let dir = match tempfile::tempdir() {
            Ok(dir) => dir,
            Err(err) => panic!("tempdir failed: {err}"),
        };
        let db_path = dir.path().join("zotero.sqlite");
        let conn = match Connection::open(&db_path) {
            Ok(conn) => conn,
            Err(err) => panic!("open temp sqlite failed: {err}"),
        };
        if let Err(err) = conn.execute_batch(
            r#"
            CREATE TABLE libraries (libraryID INTEGER PRIMARY KEY, type TEXT NOT NULL, editable INT NOT NULL DEFAULT 1, filesEditable INT NOT NULL DEFAULT 1);
            INSERT INTO libraries VALUES (1, 'user', 1, 1);
            INSERT INTO libraries VALUES (2, 'group', 1, 1);
            INSERT INTO libraries VALUES (3, 'feed', 0, 0);

            CREATE TABLE groups (
                groupID INTEGER PRIMARY KEY,
                libraryID INT NOT NULL UNIQUE,
                name TEXT NOT NULL,
                description TEXT NOT NULL DEFAULT '',
                version INT NOT NULL DEFAULT 1
            );
            INSERT INTO groups VALUES (99999, 2, 'Lab Group', '', 1);

            CREATE TABLE itemTypes (itemTypeID INTEGER PRIMARY KEY, typeName TEXT NOT NULL);
            INSERT INTO itemTypes VALUES (2, 'journalArticle');
            INSERT INTO itemTypes VALUES (3, 'book');
            INSERT INTO itemTypes VALUES (14, 'attachment');
            INSERT INTO itemTypes VALUES (26, 'note');
            INSERT INTO itemTypes VALUES (37, 'preprint');
            INSERT INTO itemTypes VALUES (38, 'annotation');

            CREATE TABLE fields (fieldID INTEGER PRIMARY KEY, fieldName TEXT NOT NULL);
            INSERT INTO fields VALUES (1, 'url');
            INSERT INTO fields VALUES (4, 'title');
            INSERT INTO fields VALUES (6, 'abstractNote');
            INSERT INTO fields VALUES (14, 'date');
            INSERT INTO fields VALUES (26, 'DOI');
            INSERT INTO fields VALUES (90, 'extra');

            CREATE TABLE items (
                itemID INTEGER PRIMARY KEY,
                itemTypeID INT NOT NULL REFERENCES itemTypes(itemTypeID),
                dateAdded TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                dateModified TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                clientDateModified TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                libraryID INT NOT NULL REFERENCES libraries(libraryID),
                key TEXT NOT NULL UNIQUE
            );

            CREATE TABLE itemData (itemID INT NOT NULL, fieldID INT NOT NULL, valueID INT NOT NULL, PRIMARY KEY (itemID, fieldID));
            CREATE TABLE itemDataValues (valueID INTEGER PRIMARY KEY, value TEXT NOT NULL);

            CREATE TABLE creatorTypes (creatorTypeID INTEGER PRIMARY KEY, creatorType TEXT NOT NULL);
            INSERT INTO creatorTypes VALUES (1, 'author');
            INSERT INTO creatorTypes VALUES (2, 'editor');

            CREATE TABLE creators (creatorID INTEGER PRIMARY KEY, firstName TEXT, lastName TEXT NOT NULL);
            CREATE TABLE itemCreators (itemID INT NOT NULL, creatorID INT NOT NULL, creatorTypeID INT NOT NULL DEFAULT 1, orderIndex INT NOT NULL DEFAULT 0, PRIMARY KEY (itemID, creatorID, creatorTypeID, orderIndex));

            CREATE TABLE tags (tagID INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE);
            CREATE TABLE itemTags (itemID INT NOT NULL, tagID INT NOT NULL, type INT NOT NULL DEFAULT 0, PRIMARY KEY (itemID, tagID));

            CREATE TABLE collections (collectionID INTEGER PRIMARY KEY, collectionName TEXT NOT NULL, parentCollectionID INT, libraryID INT NOT NULL, key TEXT NOT NULL UNIQUE);
            CREATE TABLE collectionItems (collectionID INT NOT NULL, itemID INT NOT NULL, orderIndex INT NOT NULL DEFAULT 0, PRIMARY KEY (collectionID, itemID));

            CREATE TABLE itemNotes (itemID INT PRIMARY KEY, parentItemID INT, note TEXT, title TEXT);
            CREATE TABLE itemAnnotations (
                itemID INT PRIMARY KEY,
                parentItemID INT NOT NULL,
                type INT NOT NULL,
                text TEXT,
                comment TEXT,
                color TEXT,
                pageLabel TEXT
            );

            CREATE TABLE itemAttachments (
                itemID INT PRIMARY KEY,
                parentItemID INT,
                linkMode INT,
                contentType TEXT,
                charsetID INT,
                path TEXT
            );

            CREATE TABLE itemRelations (itemID INT NOT NULL, predicateID INT NOT NULL, object TEXT NOT NULL, PRIMARY KEY (itemID, predicateID, object));
            CREATE TABLE relationPredicates (predicateID INTEGER PRIMARY KEY, predicate TEXT NOT NULL UNIQUE);
            INSERT INTO relationPredicates VALUES (1, 'dc:relation');

            CREATE TABLE fulltextItemWords (wordID INT NOT NULL, itemID INT NOT NULL, PRIMARY KEY (wordID, itemID));
            CREATE TABLE fulltextWords (wordID INTEGER PRIMARY KEY, word TEXT NOT NULL UNIQUE);

            CREATE TABLE feeds (
                libraryID INT PRIMARY KEY,
                name TEXT NOT NULL,
                url TEXT NOT NULL,
                lastCheck TEXT,
                lastUpdate TEXT,
                lastCheckError TEXT,
                refreshInterval INT
            );
            CREATE TABLE feedItems (itemID INT PRIMARY KEY);

            CREATE TABLE deletedItems (
                itemID INTEGER PRIMARY KEY,
                dateDeleted DEFAULT CURRENT_TIMESTAMP NOT NULL,
                FOREIGN KEY (itemID) REFERENCES items(itemID) ON DELETE CASCADE
            );

            CREATE TABLE version (schema TEXT PRIMARY KEY, version INT NOT NULL);
            INSERT INTO version VALUES ('userdata', 120);

            INSERT INTO items VALUES (1, 2, '2024-01-01', '2024-01-02', '2024-01-02', 1, 'ATTN001');
            INSERT INTO itemDataValues VALUES (1, 'Attention Is All You Need');
            INSERT INTO itemDataValues VALUES (2, 'We propose a new architecture...');
            INSERT INTO itemDataValues VALUES (3, '2017');
            INSERT INTO itemDataValues VALUES (4, '10.5555/attention');
            INSERT INTO itemDataValues VALUES (21, 'Citation Key: Smith2024
Original Date: 2017');
            INSERT INTO itemData VALUES (1, 4, 1);
            INSERT INTO itemData VALUES (1, 6, 2);
            INSERT INTO itemData VALUES (1, 14, 3);
            INSERT INTO itemData VALUES (1, 26, 4);
            INSERT INTO itemData VALUES (1, 90, 21);

            INSERT INTO items VALUES (2, 2, '2024-02-01', '2024-02-02', '2024-02-02', 1, 'BERT002');
            INSERT INTO itemDataValues VALUES (5, 'BERT: Pre-training of Deep Bidirectional Transformers');
            INSERT INTO itemDataValues VALUES (6, 'We introduce BERT...');
            INSERT INTO itemDataValues VALUES (7, '2019');
            INSERT INTO itemDataValues VALUES (8, '10.5555/bert');
            INSERT INTO itemData VALUES (2, 4, 5);
            INSERT INTO itemData VALUES (2, 6, 6);
            INSERT INTO itemData VALUES (2, 14, 7);
            INSERT INTO itemData VALUES (2, 26, 8);

            INSERT INTO items VALUES (3, 3, '2024-03-01', '2024-03-02', '2024-03-02', 1, 'DEEP003');
            INSERT INTO itemDataValues VALUES (9, 'Deep Learning');
            INSERT INTO itemDataValues VALUES (10, 'An MIT Press book...');
            INSERT INTO itemDataValues VALUES (11, '2016');
            INSERT INTO itemData VALUES (3, 4, 9);
            INSERT INTO itemData VALUES (3, 6, 10);
            INSERT INTO itemData VALUES (3, 14, 11);

            INSERT INTO items VALUES (6, 37, '2024-04-01', '2024-04-02', '2024-04-02', 1, 'SCAL006');
            INSERT INTO itemDataValues VALUES (12, 'Scaling Laws for Neural Language Models');
            INSERT INTO itemDataValues VALUES (13, 'We study scaling laws...');
            INSERT INTO itemDataValues VALUES (14, '2020');
            INSERT INTO itemData VALUES (6, 4, 12);
            INSERT INTO itemData VALUES (6, 6, 13);
            INSERT INTO itemData VALUES (6, 14, 14);

            INSERT INTO items VALUES (4, 26, '2024-01-03', '2024-01-03', '2024-01-03', 1, 'NOTE004');
            INSERT INTO itemNotes VALUES (4, 1, '<p>This paper introduces the transformer architecture.</p>', 'Transformer note');

            INSERT INTO items VALUES (5, 14, '2024-01-01', '2024-01-01', '2024-01-01', 1, 'ATCH005');
            INSERT INTO itemAttachments VALUES (5, 1, 0, 'application/pdf', NULL, 'storage:attention.pdf');

            INSERT INTO items VALUES (11, 38, '2024-01-04', '2024-01-04', '2024-01-04', 1, 'ANNO011');
            INSERT INTO itemAnnotations VALUES (11, 5, 1, 'attention mechanisms are the core finding', 'important highlight', '#2ea043', '1');

            INSERT INTO items VALUES (7, 2, '2023-06-01', '2023-06-02', '2023-06-02', 1, 'TRSH007');
            INSERT INTO itemDataValues VALUES (15, 'Old Survey of Neural Networks');
            INSERT INTO itemDataValues VALUES (16, '2010');
            INSERT INTO itemData VALUES (7, 4, 15);
            INSERT INTO itemData VALUES (7, 14, 16);
            INSERT INTO deletedItems VALUES (7, '2024-03-01 12:00:00');

            INSERT INTO items VALUES (8, 2, '2024-05-01', '2024-05-02', '2024-05-02', 1, 'DUPE008');
            INSERT INTO itemDataValues VALUES (17, 'Attention Is All You Need');
            INSERT INTO itemDataValues VALUES (18, '10.5555/attention');
            INSERT INTO itemData VALUES (8, 4, 17);
            INSERT INTO itemData VALUES (8, 26, 18);

            INSERT INTO items VALUES (9, 2, '2024-06-01', '2024-06-02', '2024-06-02', 2, 'GRPITM09');
            INSERT INTO itemDataValues VALUES (19, 'Group Paper on Protein Folding');
            INSERT INTO itemDataValues VALUES (20, '2024');
            INSERT INTO itemData VALUES (9, 4, 19);
            INSERT INTO itemData VALUES (9, 14, 20);

            INSERT INTO items VALUES (12, 2, '2026-04-01', '2026-04-01', '2026-04-01', 3, 'FEED012');
            INSERT INTO itemDataValues VALUES (22, 'Feed Paper on Agents');
            INSERT INTO itemDataValues VALUES (23, 'A feed-imported paper about agent tooling.');
            INSERT INTO itemDataValues VALUES (24, 'https://example.com/feed-paper');
            INSERT INTO itemData VALUES (12, 4, 22);
            INSERT INTO itemData VALUES (12, 6, 23);
            INSERT INTO itemData VALUES (12, 1, 24);
            INSERT INTO feedItems VALUES (12);
            INSERT INTO feeds VALUES (3, 'ML Weekly', 'https://example.com/ml-weekly.xml', '2026-04-01', '2026-04-01', NULL, 60);

            INSERT INTO creators VALUES (1, 'Ashish', 'Vaswani');
            INSERT INTO creators VALUES (2, 'Noam', 'Shazeer');
            INSERT INTO creators VALUES (3, 'Jacob', 'Devlin');
            INSERT INTO creators VALUES (4, 'Ian', 'Goodfellow');
            INSERT INTO creators VALUES (5, 'Jared', 'Kaplan');
            INSERT INTO creators VALUES (6, 'John', 'Smith');
            INSERT INTO creators VALUES (7, 'Alice', 'Wong');
            INSERT INTO itemCreators VALUES (1, 1, 1, 0);
            INSERT INTO itemCreators VALUES (1, 2, 1, 1);
            INSERT INTO itemCreators VALUES (2, 3, 1, 0);
            INSERT INTO itemCreators VALUES (3, 4, 1, 0);
            INSERT INTO itemCreators VALUES (6, 5, 1, 0);
            INSERT INTO itemCreators VALUES (7, 6, 1, 0);
            INSERT INTO itemCreators VALUES (9, 7, 1, 0);

            INSERT INTO tags VALUES (1, 'transformer');
            INSERT INTO tags VALUES (2, 'attention');
            INSERT INTO tags VALUES (3, 'NLP');
            INSERT INTO tags VALUES (4, 'scaling');
            INSERT INTO itemTags VALUES (1, 1, 0);
            INSERT INTO itemTags VALUES (1, 2, 0);
            INSERT INTO itemTags VALUES (2, 1, 0);
            INSERT INTO itemTags VALUES (2, 3, 0);
            INSERT INTO itemTags VALUES (4, 2, 0);
            INSERT INTO itemTags VALUES (6, 4, 0);

            INSERT INTO collections VALUES (1, 'Machine Learning', NULL, 1, 'COLML01');
            INSERT INTO collections VALUES (2, 'Transformers', 1, 1, 'COLTR02');
            INSERT INTO collections VALUES (4, 'Attention Variants', 2, 1, 'COLSUB03');
            INSERT INTO collections VALUES (3, 'Group Papers', NULL, 2, 'GRPCOL03');
            INSERT INTO collectionItems VALUES (1, 1, 0);
            INSERT INTO collectionItems VALUES (1, 2, 0);
            INSERT INTO collectionItems VALUES (1, 3, 0);
            INSERT INTO collectionItems VALUES (1, 6, 0);
            INSERT INTO collectionItems VALUES (2, 1, 0);
            INSERT INTO collectionItems VALUES (4, 8, 0);
            INSERT INTO collectionItems VALUES (3, 9, 0);

            INSERT INTO itemRelations VALUES (1, 1, 'http://zotero.org/users/local/BERT002');

            -- Related-scorer bench (unified `related`/`graph` scoring tests):
            -- RELA013 is the probe; each partner shares exactly one signal
            -- class with it so every pair lands on a distinct score.
            INSERT INTO items VALUES (13, 2, '2023-01-01', '2023-01-01', '2023-01-01', 1, 'RELA013');
            INSERT INTO itemDataValues VALUES (30, 'Bench Probe Paper');
            INSERT INTO itemData VALUES (13, 4, 30);
            INSERT INTO items VALUES (14, 2, '2023-01-02', '2023-01-02', '2023-01-02', 1, 'RELB014');
            INSERT INTO itemDataValues VALUES (31, 'Bench Explicit Relation Partner');
            INSERT INTO itemData VALUES (14, 4, 31);
            INSERT INTO items VALUES (15, 2, '2023-01-03', '2023-01-03', '2023-01-03', 1, 'COAU015');
            INSERT INTO itemDataValues VALUES (32, 'Bench Coauthor Partner');
            INSERT INTO itemData VALUES (15, 4, 32);
            INSERT INTO items VALUES (16, 2, '2023-01-04', '2023-01-04', '2023-01-04', 1, 'TAG1016');
            INSERT INTO itemDataValues VALUES (33, 'Bench Single Tag Partner');
            INSERT INTO itemData VALUES (16, 4, 33);
            INSERT INTO items VALUES (17, 2, '2023-01-05', '2023-01-05', '2023-01-05', 1, 'TAG2017');
            INSERT INTO itemDataValues VALUES (34, 'Bench Double Tag Partner');
            INSERT INTO itemData VALUES (17, 4, 34);
            INSERT INTO items VALUES (18, 2, '2023-01-06', '2023-01-06', '2023-01-06', 1, 'COLL018');
            INSERT INTO itemDataValues VALUES (35, 'Bench Shelf Partner');
            INSERT INTO itemData VALUES (18, 4, 35);

            INSERT INTO creators VALUES (8, 'Grace', 'Hopper');
            INSERT INTO itemCreators VALUES (13, 8, 1, 0);
            INSERT INTO itemCreators VALUES (15, 8, 1, 0);

            INSERT INTO tags VALUES (5, 'bench-shared-a');
            INSERT INTO tags VALUES (6, 'bench-shared-b');
            INSERT INTO itemTags VALUES (13, 5, 0);
            INSERT INTO itemTags VALUES (13, 6, 0);
            INSERT INTO itemTags VALUES (16, 5, 0);
            INSERT INTO itemTags VALUES (17, 5, 0);
            INSERT INTO itemTags VALUES (17, 6, 0);

            INSERT INTO collections VALUES (5, 'Bench Shelf', NULL, 1, 'COLBEN05');
            INSERT INTO collectionItems VALUES (5, 13, 0);
            INSERT INTO collectionItems VALUES (5, 18, 0);

            INSERT INTO itemRelations VALUES (13, 1, 'http://zotero.org/users/local/RELB014');

            INSERT INTO fulltextWords VALUES (1, 'transformer');
            INSERT INTO fulltextWords VALUES (2, 'attention');
            INSERT INTO fulltextWords VALUES (3, 'mechanism');
            INSERT INTO fulltextItemWords VALUES (1, 5);
            INSERT INTO fulltextItemWords VALUES (2, 5);
            INSERT INTO fulltextItemWords VALUES (3, 5);
            "#,
        ) {
            panic!("seed rich fixture failed: {err}");
        }
        if !extra_sql.is_empty()
            && let Err(err) = conn.execute_batch(extra_sql)
        {
            panic!("seed extra fixture sql failed: {err}");
        }
        drop(conn);
        let lib = match LocalLibrary::open(dir.path(), LibraryScope::User) {
            Ok(lib) => lib,
            Err(err) => panic!("open rich fixture failed: {err}"),
        };
        TestFixture { lib, _dir: dir }
    }

    #[test]
    fn searches_titles_and_fulltext() {
        let fixture = rich_fixture_library();
        let result = match fixture.lib.search(SearchOptions {
            query: "attention".to_string(),
            ..SearchOptions::default()
        }) {
            Ok(result) => result,
            Err(err) => panic!("search failed: {err}"),
        };
        assert!(result.items.iter().any(|item| item.key == "ATTN001"));
    }

    #[test]
    fn builds_knowledge_graph_with_expected_edges_and_determinism() {
        let fixture = rich_fixture_library();
        let opts = zot_core::GraphOptions::default();
        let graph = match fixture.lib.build_knowledge_graph(&opts) {
            Ok(graph) => graph,
            Err(err) => panic!("build_knowledge_graph failed: {err}"),
        };

        // Nodes only cover primary items, never child types.
        for node in &graph.nodes {
            assert!(
                !["attachment", "note", "annotation"].contains(&node.item_type.as_str()),
                "unexpected child node type {}",
                node.item_type
            );
        }
        let keys: std::collections::HashSet<&str> =
            graph.nodes.iter().map(|node| node.key.as_str()).collect();
        assert!(keys.contains("ATTN001"));
        assert!(keys.contains("BERT002"));

        // ATTN001 shares a collection with BERT002 and has an explicit relation
        // to it: weight 100 (related) + 1 (one shared collection) = 101.
        let edge = graph
            .edges
            .iter()
            .find(|edge| {
                (edge.source == "ATTN001" && edge.target == "BERT002")
                    || (edge.source == "BERT002" && edge.target == "ATTN001")
            })
            .expect("expected an ATTN001<->BERT002 edge");
        assert!(edge.relations.contains(&zot_core::EdgeRelation::Related));
        assert!(edge.relations.contains(&zot_core::EdgeRelation::Collection));
        assert!(!edge.relations.contains(&zot_core::EdgeRelation::Tag));
        assert_eq!(edge.weight, 101);
        assert!(edge.source < edge.target, "edge endpoints must be ordered");

        // Each node's degree equals its incident edge count (handshake lemma).
        for node in &graph.nodes {
            let incident = graph
                .edges
                .iter()
                .filter(|edge| edge.source == node.key || edge.target == node.key)
                .count();
            assert_eq!(node.degree, incident, "degree mismatch for {}", node.key);
        }
        let degree_sum: usize = graph.nodes.iter().map(|node| node.degree).sum();
        assert_eq!(degree_sum, graph.edges.len() * 2);

        // Metrics stay self-consistent.
        assert_eq!(graph.metrics.node_count, graph.nodes.len());
        assert_eq!(graph.metrics.edge_count, graph.edges.len());
        assert!(graph.metrics.connected_components >= 1);
        assert!(graph.metrics.connected_components <= graph.nodes.len());

        // Deterministic: an identical build yields an identical graph.
        let again = fixture
            .lib
            .build_knowledge_graph(&opts)
            .expect("rebuild knowledge graph");
        assert_eq!(graph, again);
    }

    #[test]
    fn related_items_rank_by_unified_scorer_signals() {
        let fixture = rich_fixture_library();
        let related = fixture
            .lib
            .get_related_items("RELA013", 10)
            .expect("related items");
        let keys: Vec<&str> = related.iter().map(|item| item.key.as_str()).collect();
        // Unified `graph::score_pair` ordering: explicit relation 100 >
        // two shared tags 10 > one shared author 8 > one shared tag 5 >
        // one shared collection 1.
        //
        // Behavior changes vs the old SQL-inline scoring (07-07-related-scorer):
        // - COAU015 is new here: `related` previously had no coauthor signal.
        // - TAG1016 is new here: `HAVING cnt >= 2` previously dropped
        //   single-shared-tag pairs at fetch time.
        assert_eq!(
            keys,
            vec!["RELB014", "TAG2017", "COAU015", "TAG1016", "COLL018"]
        );

        // `limit` still truncates after ranking.
        let top2 = fixture
            .lib
            .get_related_items("RELA013", 2)
            .expect("related items with limit");
        let top2_keys: Vec<&str> = top2.iter().map(|item| item.key.as_str()).collect();
        assert_eq!(top2_keys, vec!["RELB014", "TAG2017"]);

        // Equal scores tie-break by itemID ascending for deterministic
        // output: DEEP003 (item 3) and SCAL006 (item 6) both score 1
        // (one shared collection) relative to ATTN001.
        let attn = fixture
            .lib
            .get_related_items("ATTN001", 10)
            .expect("related items for ATTN001");
        let attn_keys: Vec<&str> = attn.iter().map(|item| item.key.as_str()).collect();
        assert_eq!(attn_keys.first(), Some(&"BERT002"));
        let deep = attn_keys
            .iter()
            .position(|key| *key == "DEEP003")
            .expect("DEEP003 in related");
        let scal = attn_keys
            .iter()
            .position(|key| *key == "SCAL006")
            .expect("SCAL006 in related");
        assert!(deep < scal, "equal scores must tie-break by itemID");

        // Child items can share signals (NOTE004 carries the 'attention' tag,
        // like ATTN001) but are excluded at fetch: `get_items_batch` could
        // never print them, so they must not consume `limit` slots either.
        // Without the fetch-side type filter this would return only
        // ["BERT002"] — the note would occupy the second slot and then be
        // dropped at load time.
        let attn_top2 = fixture
            .lib
            .get_related_items("ATTN001", 2)
            .expect("related items for ATTN001 with limit");
        let attn_top2_keys: Vec<&str> = attn_top2.iter().map(|item| item.key.as_str()).collect();
        assert_eq!(attn_top2_keys, vec!["BERT002", "DEEP003"]);
    }

    #[test]
    fn related_and_graph_rank_the_same_pairs_consistently() {
        let fixture = rich_fixture_library();
        // `min_shared_tags` is a graph-only edge-emission gate; set it to 1 so
        // the graph exposes exactly the signal set `related` scores.
        let opts = zot_core::GraphOptions {
            min_shared_tags: 1,
            ..zot_core::GraphOptions::default()
        };
        let graph = fixture
            .lib
            .build_knowledge_graph(&opts)
            .expect("knowledge graph");
        let mut neighbors: Vec<(i64, String)> = graph
            .edges
            .iter()
            .filter_map(|edge| {
                if edge.source == "RELA013" {
                    Some((edge.weight, edge.target.clone()))
                } else if edge.target == "RELA013" {
                    Some((edge.weight, edge.source.clone()))
                } else {
                    None
                }
            })
            .collect();
        neighbors.sort_by_key(|&(weight, _)| std::cmp::Reverse(weight));
        // Edge weights come from the same `graph::score_pair` weight table
        // that ranks `zot related`.
        assert_eq!(
            neighbors,
            vec![
                (100, "RELB014".to_string()),
                (10, "TAG2017".to_string()),
                (8, "COAU015".to_string()),
                (5, "TAG1016".to_string()),
                (1, "COLL018".to_string()),
            ]
        );

        let related = fixture
            .lib
            .get_related_items("RELA013", 10)
            .expect("related items");
        let related_keys: Vec<&str> = related.iter().map(|item| item.key.as_str()).collect();
        let graph_order: Vec<&str> = neighbors.iter().map(|(_, key)| key.as_str()).collect();
        // Acceptance core: both commands rank the same pairs identically.
        assert_eq!(related_keys, graph_order);
    }

    #[test]
    fn collection_scoped_graph_only_includes_collection_items() {
        let fixture = rich_fixture_library();
        let opts = zot_core::GraphOptions {
            collection: Some("COLML01".to_string()),
            ..zot_core::GraphOptions::default()
        };
        let graph = fixture
            .lib
            .build_knowledge_graph(&opts)
            .expect("collection graph");
        // COLML01 holds items 1,2,3,6 (ATTN001, BERT002, DEEP003, SCAL006).
        let keys: std::collections::HashSet<&str> =
            graph.nodes.iter().map(|node| node.key.as_str()).collect();
        assert_eq!(keys.len(), graph.nodes.len());
        for expected in ["ATTN001", "BERT002", "DEEP003", "SCAL006"] {
            assert!(keys.contains(expected), "missing {expected}");
        }
        assert!(!keys.contains("DUPE008"), "DUPE008 is not in COLML01");
        assert_eq!(graph.scope, "collection:COLML01");
    }

    #[test]
    fn resolves_group_library() {
        let fixture = rich_fixture_library();
        let lib = match LocalLibrary::open(
            fixture._dir.path(),
            LibraryScope::Group { group_id: 99999 },
        ) {
            Ok(lib) => lib,
            Err(err) => panic!("group db failed: {err}"),
        };
        assert_eq!(lib.library_id(), 2);
        let group_item = match lib.get_item("GRPITM09") {
            Ok(item) => item,
            Err(err) => panic!("group item failed: {err}"),
        };
        assert!(group_item.is_some());
    }

    #[test]
    fn supports_structured_search_and_citation_key_lookup() {
        let fixture = rich_fixture_library();
        let lib = &fixture.lib;
        let result = match lib.search(SearchOptions {
            query: "attention".to_string(),
            tag: Some("attention".to_string()),
            creator: Some("Vaswani".to_string()),
            year: Some("2017".to_string()),
            ..SearchOptions::default()
        }) {
            Ok(result) => result,
            Err(err) => panic!("structured search failed: {err}"),
        };
        assert_eq!(result.total, 1);
        assert_eq!(result.items[0].key, "ATTN001");

        let preprint = match lib.search(SearchOptions {
            query: "scaling".to_string(),
            item_type: Some("preprint".to_string()),
            year: Some("2020".to_string()),
            ..SearchOptions::default()
        }) {
            Ok(result) => result,
            Err(err) => panic!("preprint search failed: {err}"),
        };
        assert_eq!(preprint.total, 1);
        assert_eq!(preprint.items[0].key, "SCAL006");

        let citekey = match lib.search_by_citation_key("Smith2024") {
            Ok(result) => result,
            Err(err) => panic!("citation key lookup failed: {err}"),
        };
        let citekey = match citekey {
            Some(result) => result,
            None => panic!("expected citation key result"),
        };
        assert_eq!(citekey.source, "extra");
        assert_eq!(citekey.item.key, "ATTN001");
    }

    #[test]
    fn returns_recent_items_by_count_in_date_added_order() {
        let fixture = rich_fixture_library();
        let lib = &fixture.lib;

        let items = match lib.get_recent_items_by_count(3) {
            Ok(items) => items,
            Err(err) => panic!("recent by count failed: {err}"),
        };

        let keys = items
            .iter()
            .map(|item| item.key.as_str())
            .collect::<Vec<_>>();
        assert_eq!(keys, vec!["DUPE008", "SCAL006", "DEEP003"]);
    }

    #[test]
    fn enumerates_tags_libraries_feeds_and_feed_items() {
        let fixture = rich_fixture_library();
        let lib = &fixture.lib;

        let tags = match lib.get_tags() {
            Ok(tags) => tags,
            Err(err) => panic!("get tags failed: {err}"),
        };
        assert!(
            tags.iter()
                .any(|tag| tag.name == "transformer" && tag.count == 2)
        );
        assert!(
            tags.iter()
                .any(|tag| tag.name == "attention" && tag.count == 2)
        );

        let libraries = match lib.get_libraries() {
            Ok(entries) => entries,
            Err(err) => panic!("get libraries failed: {err}"),
        };
        assert!(libraries.iter().any(|entry| entry.library_type == "user"));
        assert!(libraries.iter().any(|entry| entry.library_type == "group"));
        assert!(libraries.iter().any(|entry| entry.library_type == "feed"
            && entry.feed_name.as_deref() == Some("ML Weekly")
            && entry.item_count == 1));

        let feeds = match lib.get_feeds() {
            Ok(entries) => entries,
            Err(err) => panic!("get feeds failed: {err}"),
        };
        assert_eq!(feeds.len(), 1);
        assert_eq!(feeds[0].name, "ML Weekly");
        assert_eq!(feeds[0].item_count, 1);

        let feed_items = match lib.get_feed_items(3, 10) {
            Ok(items) => items,
            Err(err) => panic!("get feed items failed: {err}"),
        };
        assert_eq!(feed_items.len(), 1);
        assert_eq!(feed_items[0].key, "FEED012");
    }

    #[test]
    fn exposes_children_notes_annotations_and_collection_search() {
        let fixture = rich_fixture_library();
        let lib = &fixture.lib;

        let children = match lib.get_item_children("ATTN001") {
            Ok(children) => children,
            Err(err) => panic!("get item children failed: {err}"),
        };
        assert!(children.iter().any(|child| matches!(
            child,
            ChildItem::Note(note) if note.key == "NOTE004"
        )));
        assert!(children.iter().any(|child| matches!(
            child,
            ChildItem::Attachment(attachment) if attachment.key == "ATCH005"
        )));
        assert!(children.iter().any(|child| matches!(
            child,
            ChildItem::Annotation(annotation)
                if annotation.key == "ANNO011" && annotation.annotation_type == "highlight"
        )));

        let note_hits = match lib.search_notes("transformer", 10) {
            Ok(results) => results,
            Err(err) => panic!("search notes failed: {err}"),
        };
        assert_eq!(note_hits.len(), 1);
        assert_eq!(note_hits[0].key, "NOTE004");
        assert_eq!(
            note_hits[0].parent_title.as_deref(),
            Some("Attention Is All You Need")
        );

        let annotations = match lib.get_annotations(None, 10) {
            Ok(results) => results,
            Err(err) => panic!("get annotations failed: {err}"),
        };
        assert_eq!(annotations.len(), 1);
        assert_eq!(annotations[0].key, "ANNO011");
        assert_eq!(
            annotations[0].parent_title.as_deref(),
            Some("Attention Is All You Need")
        );

        let annotation_hits = match lib.search_annotations("core finding", 10) {
            Ok(results) => results,
            Err(err) => panic!("search annotations failed: {err}"),
        };
        assert_eq!(annotation_hits.len(), 1);
        assert_eq!(annotation_hits[0].key, "ANNO011");

        let collections = match lib.search_collections("transform", 10) {
            Ok(results) => results,
            Err(err) => panic!("search collections failed: {err}"),
        };
        assert!(
            collections
                .iter()
                .any(|collection| collection.key == "COLTR02")
        );

        let collection = match lib.get_collection("COLTR02") {
            Ok(Some(collection)) => collection,
            Ok(None) => panic!("expected collection details"),
            Err(err) => panic!("get collection failed: {err}"),
        };
        assert_eq!(collection.name, "Transformers");

        let subcollections = match lib.get_subcollections("COLTR02") {
            Ok(subcollections) => subcollections,
            Err(err) => panic!("get subcollections failed: {err}"),
        };
        assert_eq!(subcollections.len(), 1);
        assert_eq!(subcollections[0].key, "COLSUB03");

        let item_count = match lib.get_collection_item_count("COLTR02") {
            Ok(count) => count,
            Err(err) => panic!("get collection item count failed: {err}"),
        };
        assert_eq!(item_count, 1);

        let collection_tags = match lib.get_collection_tags("COLTR02") {
            Ok(tags) => tags,
            Err(err) => panic!("get collection tags failed: {err}"),
        };
        assert!(
            collection_tags
                .iter()
                .any(|tag| tag.name == "attention" && tag.count == 1)
        );
    }

    #[test]
    fn finds_duplicates_by_title_doi_and_both() {
        let fixture = rich_fixture_library();
        let lib = &fixture.lib;

        for method in [
            DuplicateMatchMethod::Title,
            DuplicateMatchMethod::Doi,
            DuplicateMatchMethod::Both,
        ] {
            let scan = match lib.find_duplicates(method, None, 10, 250_000) {
                Ok(groups) => groups,
                Err(err) => panic!("find duplicates failed for {:?}: {err}", method),
            };
            assert!(scan.groups.iter().any(|group| {
                let keys = group
                    .items
                    .iter()
                    .map(|item| item.key.as_str())
                    .collect::<Vec<_>>();
                keys.contains(&"ATTN001") && keys.contains(&"DUPE008")
            }));
        }
    }

    #[test]
    fn find_duplicates_excludes_trashed_items() {
        // R3 regression: DUPE008 duplicates ATTN001 by title and DOI. Once it
        // sits in the trash the pair must vanish from every duplicate group,
        // otherwise a completed cleanup would be re-reported on rescan.
        let fixture = rich_fixture_library_with_extra_sql(
            "INSERT INTO deletedItems VALUES (8, '2026-07-11 00:00:00');",
        );
        let lib = &fixture.lib;

        for method in [
            DuplicateMatchMethod::Title,
            DuplicateMatchMethod::Doi,
            DuplicateMatchMethod::Both,
        ] {
            let scan = match lib.find_duplicates(method, None, 10, 250_000) {
                Ok(groups) => groups,
                Err(err) => panic!("find duplicates failed for {:?}: {err}", method),
            };
            assert!(
                scan.groups
                    .iter()
                    .flat_map(|group| group.items.iter())
                    .all(|item| item.key != "DUPE008"),
                "trashed DUPE008 must not appear in any duplicate group ({method:?})"
            );
        }
    }

    #[test]
    fn collection_resolution_prefers_exact_key_and_rejects_ambiguous_names() {
        let fixture = rich_fixture_library_with_extra_sql(
            "INSERT INTO collections VALUES (6, 'Key Winner', NULL, 1, 'Transformers');
             INSERT INTO collectionItems VALUES (6, 2, 0);
             INSERT INTO collections VALUES (7, 'Shared Shelf', NULL, 1, 'ZZZZ0002');
             INSERT INTO collections VALUES (8, 'Shared Shelf', NULL, 1, 'AAAA0001');",
        );

        let keyed = fixture
            .lib
            .get_collection_items("Transformers")
            .expect("exact collection key");
        assert_eq!(
            keyed
                .iter()
                .map(|item| item.key.as_str())
                .collect::<Vec<_>>(),
            vec!["BERT002"]
        );

        let error = fixture
            .lib
            .get_collection_items("Shared Shelf")
            .expect_err("duplicate collection names must fail closed");
        let payload = error.payload();
        assert_eq!(payload.code, "collection-ambiguous");
        assert_eq!(
            payload.hint.as_deref(),
            Some("Use one of these collection keys: AAAA0001, ZZZZ0002")
        );
    }

    #[test]
    fn duplicate_title_scan_bounds_ten_thousand_candidates_by_budget() {
        let candidates = (0..10_000)
            .map(|index| DuplicateTitleCandidate {
                item_id: i64::from(index),
                title: format!("Deterministic duplicate title {index:05}"),
                year: "2026".to_string(),
                author: "auditor".to_string(),
            })
            .collect::<Vec<_>>();

        let (_, meta) = title_duplicate_components(&candidates, 1_000, 0.92);
        assert_eq!(meta.candidate_pair_count, 1_000);
        assert!(meta.truncated);
        assert!(meta.skipped_oversize_blocks >= 1);
    }

    #[test]
    fn search_excludes_trashed_items_by_default_and_allows_explicit_inclusion() {
        // TRSH007 is seeded in `deletedItems`. Default search excludes it;
        // callers can restore the legacy projection explicitly.
        let fixture = rich_fixture_library();
        let lib = &fixture.lib;

        let default_all = match lib.search(SearchOptions::default()) {
            Ok(result) => result,
            Err(err) => panic!("default full-scan search failed: {err}"),
        };
        assert!(default_all.items.iter().all(|item| item.key != "TRSH007"));

        let included_all = match lib.search(SearchOptions {
            exclude_trashed: false,
            ..SearchOptions::default()
        }) {
            Ok(result) => result,
            Err(err) => panic!("included full-scan search failed: {err}"),
        };
        assert!(included_all.items.iter().any(|item| item.key == "TRSH007"));

        let default_query = match lib.search(SearchOptions {
            query: "Survey".to_string(),
            ..SearchOptions::default()
        }) {
            Ok(result) => result,
            Err(err) => panic!("default LIKE search failed: {err}"),
        };
        assert!(default_query.items.iter().all(|item| item.key != "TRSH007"));

        let included_query = match lib.search(SearchOptions {
            query: "Survey".to_string(),
            exclude_trashed: false,
            ..SearchOptions::default()
        }) {
            Ok(result) => result,
            Err(err) => panic!("included LIKE search failed: {err}"),
        };
        assert!(
            included_query
                .items
                .iter()
                .any(|item| item.key == "TRSH007")
        );

        let excluded_stats = lib.get_stats().expect("excluded stats");
        let included_stats = lib.get_stats_with_trashed(true).expect("included stats");
        assert_eq!(included_stats.total_items, excluded_stats.total_items + 1);
    }
}
