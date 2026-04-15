use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use regex::Regex;
use rusqlite::{Connection, OptionalExtension, params, params_from_iter};
use strsim::normalized_levenshtein;
use tempfile::TempDir;
use zot_core::{
    Attachment, Collection, Creator, DuplicateGroup, Item, LibraryScope, LibraryStats, Note,
    SearchResult, ZotError, ZotResult,
};

use crate::citation::export_item;

const EXCLUDED_TYPE_NAMES: &[&str] = &["attachment", "note", "annotation"];

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

#[derive(Debug, Clone)]
pub struct SearchOptions {
    pub query: String,
    pub collection: Option<String>,
    pub item_type: Option<String>,
    pub sort: Option<SortField>,
    pub direction: SortDirection,
    pub limit: usize,
    pub offset: usize,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            query: String::new(),
            collection: None,
            item_type: None,
            sort: None,
            direction: SortDirection::Desc,
            limit: 50,
            offset: 0,
        }
    }
}

pub struct LocalLibrary {
    db_path: PathBuf,
    pub data_dir: PathBuf,
    library_scope: LibraryScope,
    library_id: i64,
    conn: Connection,
    _temp_dir: Option<TempDir>,
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

        let (conn, temp_dir) = Self::connect(&db_path)?;
        let mut instance = Self {
            db_path,
            data_dir,
            library_scope: scope,
            library_id: 1,
            conn,
            _temp_dir: temp_dir,
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
        self.conn
            .query_row(
                "SELECT version FROM version WHERE schema = 'userdata'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map_err(sql_err("schema-version"))
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
        let excluded_ids = self.excluded_type_ids()?;
        let mut item_ids: HashSet<i64> = HashSet::new();

        if options.query.is_empty() {
            let mut stmt = self
                .conn
                .prepare("SELECT itemID FROM items WHERE libraryID = ?1")
                .map_err(sql_err("search-all"))?;
            let rows = stmt
                .query_map(params![self.library_id], |row| row.get::<_, i64>(0))
                .map_err(sql_err("search-all"))?;
            for row in rows {
                let id = row.map_err(sql_err("search-all"))?;
                if !self.is_excluded_item(id, &excluded_ids)? {
                    item_ids.insert(id);
                }
            }
        } else {
            let like = format!("%{}%", options.query);
            self.collect_matching_item_ids_from_field_search(&like, &excluded_ids, &mut item_ids)?;
            self.collect_matching_item_ids_from_creator_search(
                &like,
                &excluded_ids,
                &mut item_ids,
            )?;
            self.collect_matching_item_ids_from_tag_search(&like, &excluded_ids, &mut item_ids)?;
            self.collect_matching_item_ids_from_fulltext_search(
                &like,
                &excluded_ids,
                &mut item_ids,
            )?;
        }

        if let Some(collection) = options.collection.as_deref() {
            let collection_id = self.resolve_collection_id(collection)?;
            let mut stmt = self
                .conn
                .prepare("SELECT itemID FROM collectionItems WHERE collectionID = ?1")
                .map_err(sql_err("collection-filter"))?;
            let rows = stmt
                .query_map(params![collection_id], |row| row.get::<_, i64>(0))
                .map_err(sql_err("collection-filter"))?;
            let collection_items = rows
                .collect::<Result<Vec<_>, _>>()
                .map_err(sql_err("collection-filter"))?
                .into_iter()
                .collect::<HashSet<_>>();
            item_ids.retain(|item_id| collection_items.contains(item_id));
        }

        if let Some(item_type) = options.item_type.as_deref() {
            let type_id = self
                .conn
                .query_row(
                    "SELECT itemTypeID FROM itemTypes WHERE typeName = ?1",
                    params![item_type],
                    |row| row.get::<_, i64>(0),
                )
                .optional()
                .map_err(sql_err("resolve-item-type"))?;
            if let Some(type_id) = type_id {
                item_ids.retain(|item_id| self.item_type_id(*item_id).ok() == Some(type_id));
            } else {
                item_ids.clear();
            }
        }

        let total = item_ids.len();
        let mut items = self.get_items_batch(&item_ids.into_iter().collect::<Vec<_>>())?;
        sort_items(&mut items, options.sort, options.direction);
        let items = items
            .into_iter()
            .skip(options.offset)
            .take(options.limit)
            .collect::<Vec<_>>();

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
                "SELECT itemID FROM items WHERE key = ?1 AND libraryID = ?2",
                params![key, self.library_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map_err(sql_err("get-item"))?;
        match item_id {
            Some(id) if !self.is_excluded_item(id, &self.excluded_type_ids()?)? => {
                self.get_item_by_id(id).map(Some)
            }
            _ => Ok(None),
        }
    }

    pub fn get_notes(&self, key: &str) -> ZotResult<Vec<Note>> {
        let parent_id = self.parent_item_id(key)?;
        let Some(parent_id) = parent_id else {
            return Ok(Vec::new());
        };

        let mut stmt = self.conn.prepare(
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

        let mut notes = Vec::new();
        for row in rows {
            let (note_item_id, note_key, note_html) = row.map_err(sql_err("get-notes"))?;
            let tags = self.get_item_tags(note_item_id)?;
            notes.push(Note {
                key: note_key,
                parent_key: key.to_string(),
                content: html_to_text(&note_html),
                tags,
            });
        }
        Ok(notes)
    }

    pub fn get_collections(&self) -> ZotResult<Vec<Collection>> {
        let mut stmt = self
            .conn
            .prepare("SELECT collectionID, collectionName, parentCollectionID, key FROM collections WHERE libraryID = ?1")
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

    pub fn get_collection_items(&self, collection_key: &str) -> ZotResult<Vec<Item>> {
        let collection_id = self.resolve_collection_id(collection_key)?;
        let mut stmt = self
            .conn
            .prepare("SELECT itemID FROM collectionItems WHERE collectionID = ?1 ORDER BY orderIndex ASC")
            .map_err(sql_err("get-collection-items"))?;
        let rows = stmt
            .query_map(params![collection_id], |row| row.get::<_, i64>(0))
            .map_err(sql_err("get-collection-items"))?;
        let item_ids = rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(sql_err("get-collection-items"))?;
        self.get_items_batch(&item_ids)
    }

    pub fn get_attachments(&self, key: &str) -> ZotResult<Vec<Attachment>> {
        let parent_id = self.parent_item_id(key)?;
        let Some(parent_id) = parent_id else {
            return Ok(Vec::new());
        };

        let mut stmt = self.conn.prepare(
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

    pub fn get_pdf_attachment(&self, key: &str) -> ZotResult<Option<Attachment>> {
        Ok(self
            .get_attachments(key)?
            .into_iter()
            .find(|attachment| attachment.content_type == "application/pdf"))
    }

    pub fn pdf_path(&self, attachment: &Attachment) -> PathBuf {
        self.data_dir
            .join("storage")
            .join(&attachment.key)
            .join(&attachment.filename)
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
        let mut stmt = self.conn.prepare(&sql).map_err(sql_err("recent-items"))?;
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

    pub fn get_trash_items(&self, limit: usize) -> ZotResult<Vec<Item>> {
        let mut stmt = self.conn.prepare(
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

    pub fn find_duplicates(&self, limit: usize) -> ZotResult<Vec<DuplicateGroup>> {
        let items = self.list_items(None, 10_000, 0)?;
        let mut groups = Vec::new();
        let mut seen: BTreeSet<String> = BTreeSet::new();

        let mut doi_map: HashMap<String, Vec<Item>> = HashMap::new();
        for item in &items {
            if let Some(doi) = item.doi.as_deref() {
                doi_map
                    .entry(doi.trim().to_lowercase())
                    .or_default()
                    .push(item.clone());
            }
        }
        for items in doi_map.into_values() {
            if items.len() > 1 {
                let group_key = items
                    .iter()
                    .map(|item| item.key.clone())
                    .collect::<Vec<_>>()
                    .join(",");
                if seen.insert(group_key) {
                    groups.push(DuplicateGroup {
                        match_type: "doi".to_string(),
                        score: 1.0,
                        items,
                    });
                }
            }
        }

        let mut used = HashSet::new();
        for index in 0..items.len() {
            if used.contains(&items[index].key) {
                continue;
            }
            let mut cluster = vec![items[index].clone()];
            let left = normalize_title(&items[index].title);
            for other in items.iter().skip(index + 1) {
                if used.contains(&other.key) {
                    continue;
                }
                let right = normalize_title(&other.title);
                if normalized_levenshtein(&left, &right) >= 0.92 {
                    cluster.push(other.clone());
                    used.insert(other.key.clone());
                }
            }
            if cluster.len() > 1 {
                used.insert(items[index].key.clone());
                groups.push(DuplicateGroup {
                    match_type: "title".to_string(),
                    score: 0.92,
                    items: cluster,
                });
            }
        }

        Ok(groups.into_iter().take(limit).collect())
    }

    pub fn get_related_items(&self, key: &str, limit: usize) -> ZotResult<Vec<Item>> {
        let parent_id = self.parent_item_id(key)?;
        let Some(parent_id) = parent_id else {
            return Ok(Vec::new());
        };
        let mut scores: HashMap<i64, i64> = HashMap::new();

        let mut stmt = self
            .conn
            .prepare("SELECT object FROM itemRelations WHERE itemID = ?1 AND predicateID = 1")
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
                *scores.entry(item_id).or_insert(0) += 100;
            }
        }

        let my_collections = self.get_item_collection_ids(parent_id)?;
        if !my_collections.is_empty() {
            let placeholders = repeat_placeholders(my_collections.len());
            let sql = format!(
                "SELECT itemID, COUNT(*) as cnt FROM collectionItems WHERE collectionID IN ({}) AND itemID != ?1 GROUP BY itemID",
                placeholders
            );
            let mut params_vec = vec![rusqlite::types::Value::from(parent_id)];
            params_vec.extend(
                my_collections
                    .iter()
                    .copied()
                    .map(rusqlite::types::Value::from),
            );
            let mut stmt = self
                .conn
                .prepare(&sql)
                .map_err(sql_err("related-collections"))?;
            let rows = stmt
                .query_map(params_from_iter(params_vec), |row| {
                    Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
                })
                .map_err(sql_err("related-collections"))?;
            for row in rows {
                let (item_id, score) = row.map_err(sql_err("related-collections"))?;
                *scores.entry(item_id).or_insert(0) += score;
            }
        }

        let my_tag_ids = self.get_item_tag_ids(parent_id)?;
        if !my_tag_ids.is_empty() {
            let placeholders = repeat_placeholders(my_tag_ids.len());
            let sql = format!(
                "SELECT itemID, COUNT(*) as cnt FROM itemTags WHERE tagID IN ({}) AND itemID != ?1 GROUP BY itemID HAVING cnt >= 2",
                placeholders
            );
            let mut params_vec = vec![rusqlite::types::Value::from(parent_id)];
            params_vec.extend(my_tag_ids.iter().copied().map(rusqlite::types::Value::from));
            let mut stmt = self.conn.prepare(&sql).map_err(sql_err("related-tags"))?;
            let rows = stmt
                .query_map(params_from_iter(params_vec), |row| {
                    Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
                })
                .map_err(sql_err("related-tags"))?;
            for row in rows {
                let (item_id, score) = row.map_err(sql_err("related-tags"))?;
                *scores.entry(item_id).or_insert(0) += score * 5;
            }
        }

        let mut ordered = scores.into_iter().collect::<Vec<_>>();
        ordered.sort_by_key(|(_, score)| Reverse(*score));
        let item_ids = ordered
            .into_iter()
            .take(limit)
            .map(|(item_id, _)| item_id)
            .collect::<Vec<_>>();
        self.get_items_batch(&item_ids)
    }

    pub fn get_stats(&self) -> ZotResult<LibraryStats> {
        let items = self.list_items(None, 10_000, 0)?;
        let total_items = items.len();
        let mut by_type = BTreeMap::new();
        let mut top_tags = BTreeMap::new();
        let mut collections = BTreeMap::new();
        for item in &items {
            *by_type.entry(item.item_type.clone()).or_insert(0) += 1;
            for tag in &item.tags {
                *top_tags.entry(tag.clone()).or_insert(0) += 1;
            }
            for collection in &item.collections {
                *collections.entry(collection.clone()).or_insert(0) += 1;
            }
        }
        let pdf_attachments = items
            .iter()
            .filter_map(|item| self.get_pdf_attachment(&item.key).ok().flatten())
            .count();
        let notes = items
            .iter()
            .map(|item| self.get_notes(&item.key).map(|notes| notes.len()))
            .collect::<ZotResult<Vec<_>>>()?
            .into_iter()
            .sum();
        Ok(LibraryStats {
            total_items,
            by_type,
            top_tags,
            collections,
            pdf_attachments,
            notes,
        })
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

    fn connect(db_path: &Path) -> ZotResult<(Connection, Option<TempDir>)> {
        let uri = format!(
            "file:{}?mode=ro&immutable=1",
            db_path.to_string_lossy().replace('\\', "/")
        );
        match Connection::open_with_flags(
            &uri,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI,
        ) {
            Ok(conn) => Ok((conn, None)),
            Err(_) => {
                let temp_dir = tempfile::tempdir().map_err(|source| ZotError::Io {
                    path: db_path.to_path_buf(),
                    source,
                })?;
                let temp_db = temp_dir.path().join("zotero.sqlite");
                fs::copy(db_path, &temp_db).map_err(|source| ZotError::Io {
                    path: temp_db.clone(),
                    source,
                })?;
                for suffix in ["sqlite-wal", "sqlite-shm"] {
                    let source_path = db_path.with_extension(suffix);
                    let target_path = temp_db.with_extension(suffix);
                    if source_path.exists() {
                        let _ = fs::copy(source_path, target_path);
                    }
                }
                let conn = Connection::open(&temp_db).map_err(sql_err("open-fallback-db"))?;
                Ok((conn, Some(temp_dir)))
            }
        }
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

    fn excluded_type_ids(&self) -> ZotResult<Vec<i64>> {
        let placeholders = repeat_placeholders(EXCLUDED_TYPE_NAMES.len());
        let sql = format!("SELECT itemTypeID FROM itemTypes WHERE typeName IN ({placeholders})");
        let mut stmt = self
            .conn
            .prepare(&sql)
            .map_err(sql_err("excluded-type-ids"))?;
        let rows = stmt
            .query_map(
                params_from_iter(EXCLUDED_TYPE_NAMES.iter().copied()),
                |row| row.get::<_, i64>(0),
            )
            .map_err(sql_err("excluded-type-ids"))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(sql_err("excluded-type-ids"))
    }

    fn is_excluded_item(&self, item_id: i64, excluded_ids: &[i64]) -> ZotResult<bool> {
        let item_type_id = self.item_type_id(item_id)?;
        Ok(excluded_ids.contains(&item_type_id))
    }

    fn item_type_id(&self, item_id: i64) -> ZotResult<i64> {
        self.conn
            .query_row(
                "SELECT itemTypeID FROM items WHERE itemID = ?1",
                params![item_id],
                |row| row.get::<_, i64>(0),
            )
            .map_err(sql_err("item-type-id"))
    }

    fn resolve_collection_id(&self, collection: &str) -> ZotResult<i64> {
        self.conn
            .query_row(
                "SELECT collectionID FROM collections WHERE libraryID = ?1 AND (key = ?2 OR collectionName = ?2)",
                params![self.library_id, collection],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map_err(sql_err("resolve-collection"))?
            .ok_or_else(|| ZotError::InvalidInput {
                code: "collection-not-found".to_string(),
                message: format!("Collection '{collection}' not found"),
                hint: Some("Use 'zot collection list' to inspect collection names".to_string()),
            })
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
            .prepare("SELECT tagID FROM itemTags WHERE itemID = ?1")
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
            .prepare("SELECT collectionID FROM collectionItems WHERE itemID = ?1")
            .map_err(sql_err("item-collection-ids"))?;
        let rows = stmt
            .query_map(params![item_id], |row| row.get::<_, i64>(0))
            .map_err(sql_err("item-collection-ids"))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(sql_err("item-collection-ids"))
    }

    fn collect_matching_item_ids_from_field_search(
        &self,
        like: &str,
        excluded_ids: &[i64],
        item_ids: &mut HashSet<i64>,
    ) -> ZotResult<()> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT i.itemID FROM items i JOIN itemData id ON i.itemID = id.itemID JOIN itemDataValues iv ON id.valueID = iv.valueID WHERE iv.value LIKE ?1 AND i.libraryID = ?2",
        )
        .map_err(sql_err("search-fields"))?;
        let rows = stmt
            .query_map(params![like, self.library_id], |row| row.get::<_, i64>(0))
            .map_err(sql_err("search-fields"))?;
        for row in rows {
            let item_id = row.map_err(sql_err("search-fields"))?;
            if !excluded_ids.contains(&self.item_type_id(item_id)?) {
                item_ids.insert(item_id);
            }
        }
        Ok(())
    }

    fn collect_matching_item_ids_from_creator_search(
        &self,
        like: &str,
        excluded_ids: &[i64],
        item_ids: &mut HashSet<i64>,
    ) -> ZotResult<()> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT ic.itemID FROM itemCreators ic JOIN creators c ON ic.creatorID = c.creatorID JOIN items i ON ic.itemID = i.itemID WHERE (c.firstName LIKE ?1 OR c.lastName LIKE ?1) AND i.libraryID = ?2",
        )
        .map_err(sql_err("search-creators"))?;
        let rows = stmt
            .query_map(params![like, self.library_id], |row| row.get::<_, i64>(0))
            .map_err(sql_err("search-creators"))?;
        for row in rows {
            let item_id = row.map_err(sql_err("search-creators"))?;
            if !excluded_ids.contains(&self.item_type_id(item_id)?) {
                item_ids.insert(item_id);
            }
        }
        Ok(())
    }

    fn collect_matching_item_ids_from_tag_search(
        &self,
        like: &str,
        excluded_ids: &[i64],
        item_ids: &mut HashSet<i64>,
    ) -> ZotResult<()> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT it.itemID FROM itemTags it JOIN tags t ON it.tagID = t.tagID JOIN items i ON it.itemID = i.itemID WHERE t.name LIKE ?1 AND i.libraryID = ?2",
        )
        .map_err(sql_err("search-tags"))?;
        let rows = stmt
            .query_map(params![like, self.library_id], |row| row.get::<_, i64>(0))
            .map_err(sql_err("search-tags"))?;
        for row in rows {
            let item_id = row.map_err(sql_err("search-tags"))?;
            if !excluded_ids.contains(&self.item_type_id(item_id)?) {
                item_ids.insert(item_id);
            }
        }
        Ok(())
    }

    fn collect_matching_item_ids_from_fulltext_search(
        &self,
        like: &str,
        excluded_ids: &[i64],
        item_ids: &mut HashSet<i64>,
    ) -> ZotResult<()> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT ia.parentItemID FROM fulltextItemWords fw JOIN fulltextWords w ON fw.wordID = w.wordID JOIN itemAttachments ia ON fw.itemID = ia.itemID JOIN items i ON ia.parentItemID = i.itemID WHERE w.word LIKE ?1 AND ia.parentItemID IS NOT NULL AND i.libraryID = ?2",
        )
        .map_err(sql_err("search-fulltext"))?;
        let rows = stmt
            .query_map(params![like, self.library_id], |row| row.get::<_, i64>(0))
            .map_err(sql_err("search-fulltext"))?;
        for row in rows {
            let item_id = row.map_err(sql_err("search-fulltext"))?;
            if !excluded_ids.contains(&self.item_type_id(item_id)?) {
                item_ids.insert(item_id);
            }
        }
        Ok(())
    }

    fn get_items_batch(&self, item_ids: &[i64]) -> ZotResult<Vec<Item>> {
        let excluded_ids = self.excluded_type_ids()?;
        let mut items = Vec::new();
        for item_id in item_ids {
            if !self.is_excluded_item(*item_id, &excluded_ids)? {
                items.push(self.get_item_by_id(*item_id)?);
            }
        }
        Ok(items)
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
        let mut stmt = self.conn.prepare(
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
        let mut stmt = self.conn.prepare(
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
        let mut stmt = self.conn.prepare(
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
        let mut stmt = self.conn.prepare(
            "SELECT c.key FROM collectionItems ci JOIN collections c ON ci.collectionID = c.collectionID WHERE ci.itemID = ?1 ORDER BY c.collectionName ASC",
        )
        .map_err(sql_err("item-collection-keys"))?;
        let rows = stmt
            .query_map(params![item_id], |row| row.get::<_, String>(0))
            .map_err(sql_err("item-collection-keys"))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(sql_err("item-collection-keys"))
    }
}

fn sort_items(items: &mut [Item], sort: Option<SortField>, direction: SortDirection) {
    match sort {
        Some(SortField::Title) => {
            items.sort_by(|left, right| left.title.to_lowercase().cmp(&right.title.to_lowercase()))
        }
        Some(SortField::Creator) => items.sort_by(|left, right| {
            let left_name = left
                .creators
                .first()
                .map(Creator::full_name)
                .unwrap_or_default()
                .to_lowercase();
            let right_name = right
                .creators
                .first()
                .map(Creator::full_name)
                .unwrap_or_default()
                .to_lowercase();
            left_name.cmp(&right_name)
        }),
        Some(SortField::DateAdded) => {
            items.sort_by(|left, right| left.date_added.cmp(&right.date_added))
        }
        Some(SortField::DateModified) => {
            items.sort_by(|left, right| left.date_modified.cmp(&right.date_modified))
        }
        None => items.sort_by(|left, right| left.key.cmp(&right.key)),
    }
    if matches!(direction, SortDirection::Desc) {
        items.reverse();
    }
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

#[cfg(test)]
mod tests {
    use zot_core::LibraryScope;

    use super::{LocalLibrary, SearchOptions};

    fn fixture_library() -> LocalLibrary {
        let data_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("ref")
            .join("zotero-cli-cc")
            .join("tests")
            .join("fixtures");
        match LocalLibrary::open(data_dir, LibraryScope::User) {
            Ok(lib) => lib,
            Err(err) => panic!("fixture db: {err}"),
        }
    }

    #[test]
    fn searches_titles_and_fulltext() {
        let lib = fixture_library();
        let result = match lib.search(SearchOptions {
            query: "attention".to_string(),
            ..SearchOptions::default()
        }) {
            Ok(result) => result,
            Err(err) => panic!("search failed: {err}"),
        };
        assert!(result.items.iter().any(|item| item.key == "ATTN001"));
    }

    #[test]
    fn resolves_group_library() {
        let data_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("ref")
            .join("zotero-cli-cc")
            .join("tests")
            .join("fixtures");
        let lib = match LocalLibrary::open(data_dir, LibraryScope::Group { group_id: 99999 }) {
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
}
