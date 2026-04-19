use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use regex::Regex;
use rusqlite::{Connection, OptionalExtension, params, params_from_iter};
use strsim::normalized_levenshtein;
use tempfile::TempDir;
use zot_core::{
    AnnotationRecord, Attachment, ChildItem, CitationKeyMatch, Collection, Creator, DuplicateGroup,
    FeedInfo, Item, LibraryInfo, LibraryScope, LibraryStats, Note, NoteSearchResult, SearchResult,
    TagSummary, ZotError, ZotResult,
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

        if let Some(tag) = options.tag.as_deref() {
            item_ids = self.filter_item_ids_by_tag(item_ids, tag)?;
        }

        if let Some(creator) = options.creator.as_deref() {
            item_ids = self.filter_item_ids_by_creator(item_ids, creator)?;
        }

        if let Some(year) = options.year.as_deref() {
            item_ids = self.filter_item_ids_by_year(item_ids, year)?;
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

    pub fn search_notes(&self, query: &str, limit: usize) -> ZotResult<Vec<NoteSearchResult>> {
        let pattern = format!("%{query}%");
        let title_field_id = self.field_id("title")?.unwrap_or(4);
        let sql = format!(
            "SELECT i.key, n.note, n.title, pi.key, pdv.value
             FROM itemNotes n
             JOIN items i ON n.itemID = i.itemID
             LEFT JOIN items pi ON n.parentItemID = pi.itemID
             LEFT JOIN itemData pd ON pi.itemID = pd.itemID AND pd.fieldID = {title_field_id}
             LEFT JOIN itemDataValues pdv ON pd.valueID = pdv.valueID
             WHERE n.note LIKE ?1
             AND i.libraryID = ?2
             AND i.itemID NOT IN (SELECT itemID FROM deletedItems)
             LIMIT ?3"
        );
        let mut stmt = self.conn.prepare(&sql).map_err(sql_err("search-notes"))?;
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
            .prepare(
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
            .prepare(
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
        children.extend(self.get_note_children(key)?);
        children.extend(self.get_attachment_children(key)?);
        children.extend(self.get_annotation_children(key)?);
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
                .map(child_to_annotation_record)
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
                .prepare(&sql)
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
        let pattern = format!("%{query}%");
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
             WHERE (ia.text LIKE ?1 OR ia.comment LIKE ?1)
             AND i.libraryID = ?2
             AND i.itemID NOT IN (SELECT itemID FROM deletedItems)
             LIMIT ?3"
        );
        let mut stmt = self
            .conn
            .prepare(&sql)
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
            .prepare(
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
            .prepare(&query)
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
            .prepare(
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
            .prepare(&query)
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

    pub fn get_attachment_by_key(&self, key: &str) -> ZotResult<Option<Attachment>> {
        let mut stmt = self
            .conn
            .prepare(
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

    pub fn get_recent_items_by_count(&self, count: usize) -> ZotResult<Vec<Item>> {
        let mut stmt = self
            .conn
            .prepare(
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

    pub fn find_duplicates(
        &self,
        method: DuplicateMatchMethod,
        collection: Option<&str>,
        limit: usize,
    ) -> ZotResult<Vec<DuplicateGroup>> {
        let items = self.list_items(collection, 10_000, 0)?;
        let mut groups = Vec::new();
        let mut seen: BTreeSet<String> = BTreeSet::new();

        if matches!(
            method,
            DuplicateMatchMethod::Doi | DuplicateMatchMethod::Both
        ) {
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
                    if seen.insert(format!("doi:{group_key}")) {
                        groups.push(DuplicateGroup {
                            match_type: "doi".to_string(),
                            score: 1.0,
                            items,
                        });
                    }
                }
            }
        }

        if matches!(
            method,
            DuplicateMatchMethod::Title | DuplicateMatchMethod::Both
        ) {
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
            .prepare(&sql)
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
            .prepare(&sql)
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
            .prepare(&sql)
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
            .prepare(&sql)
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
            .prepare(&sql)
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

    fn filter_item_ids_by_tag(&self, item_ids: HashSet<i64>, tag: &str) -> ZotResult<HashSet<i64>> {
        self.filter_item_ids_by_exact_name(
            item_ids,
            "SELECT DISTINCT it.itemID FROM itemTags it JOIN tags t ON it.tagID = t.tagID WHERE LOWER(t.name) = ? AND it.itemID IN",
            tag.to_lowercase(),
            "tag-filter-batch",
        )
    }

    fn filter_item_ids_by_creator(
        &self,
        item_ids: HashSet<i64>,
        creator: &str,
    ) -> ZotResult<HashSet<i64>> {
        self.filter_item_ids_by_exact_name(
            item_ids,
            "SELECT DISTINCT ic.itemID FROM itemCreators ic JOIN creators c ON ic.creatorID = c.creatorID WHERE LOWER(TRIM(COALESCE(c.firstName, '') || ' ' || c.lastName)) LIKE ? AND ic.itemID IN",
            format!("%{}%", creator.to_lowercase()),
            "creator-filter-batch",
        )
    }

    fn filter_item_ids_by_year(
        &self,
        item_ids: HashSet<i64>,
        year: &str,
    ) -> ZotResult<HashSet<i64>> {
        self.filter_item_ids_by_exact_name(
            item_ids,
            "SELECT DISTINCT id.itemID FROM itemData id JOIN fields f ON id.fieldID = f.fieldID JOIN itemDataValues iv ON id.valueID = iv.valueID WHERE f.fieldName = 'date' AND iv.value LIKE ? AND id.itemID IN",
            format!("{year}%"),
            "year-filter-batch",
        )
    }

    fn filter_item_ids_by_exact_name(
        &self,
        item_ids: HashSet<i64>,
        sql_prefix: &str,
        filter_value: String,
        context: &'static str,
    ) -> ZotResult<HashSet<i64>> {
        if item_ids.is_empty() {
            return Ok(item_ids);
        }

        const CHUNK: usize = 500;
        let candidate_ids = item_ids.into_iter().collect::<Vec<_>>();
        let mut matched = HashSet::new();
        for chunk in candidate_ids.chunks(CHUNK) {
            let placeholders = repeat_placeholders(chunk.len());
            let sql = format!("{sql_prefix} ({placeholders})");
            let mut params = Vec::with_capacity(chunk.len() + 1);
            params.push(rusqlite::types::Value::from(filter_value.clone()));
            params.extend(chunk.iter().copied().map(rusqlite::types::Value::from));

            let mut stmt = self.conn.prepare(&sql).map_err(sql_err(context))?;
            let rows = stmt
                .query_map(params_from_iter(params.iter()), |row| row.get::<_, i64>(0))
                .map_err(sql_err(context))?;
            for row in rows {
                matched.insert(row.map_err(sql_err(context))?);
            }
        }
        Ok(matched)
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

    fn get_note_children(&self, key: &str) -> ZotResult<Vec<ChildItem>> {
        Ok(self
            .get_notes(key)?
            .into_iter()
            .map(|note| ChildItem {
                key: note.key,
                parent_key: Some(note.parent_key),
                item_type: "note".to_string(),
                title: None,
                content_type: None,
                filename: None,
                note: Some(note.content),
                annotation_type: None,
                text: None,
                comment: None,
                color: None,
                page_label: None,
                tags: note.tags,
            })
            .collect())
    }

    fn get_attachment_children(&self, key: &str) -> ZotResult<Vec<ChildItem>> {
        Ok(self
            .get_attachments(key)?
            .into_iter()
            .map(|attachment| ChildItem {
                key: attachment.key,
                parent_key: Some(attachment.parent_key),
                item_type: "attachment".to_string(),
                title: Some(attachment.filename.clone()),
                content_type: Some(attachment.content_type),
                filename: Some(attachment.filename),
                note: None,
                annotation_type: None,
                text: None,
                comment: None,
                color: None,
                page_label: None,
                tags: Vec::new(),
            })
            .collect())
    }

    fn get_annotation_children(&self, key: &str) -> ZotResult<Vec<ChildItem>> {
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
                .prepare(
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
                .prepare(
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
                    Ok(ChildItem {
                        key: row.get::<_, String>(0)?,
                        parent_key: Some(attachment_key.clone()),
                        item_type: "annotation".to_string(),
                        title: None,
                        content_type: None,
                        filename: None,
                        note: None,
                        annotation_type: Some(annotation_type_name(row.get::<_, i64>(5)?)),
                        text: Some(row.get::<_, Option<String>>(1)?.unwrap_or_default()),
                        comment: Some(row.get::<_, Option<String>>(2)?.unwrap_or_default()),
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

fn sort_items(items: &mut [Item], sort: Option<SortField>, direction: SortDirection) {
    match sort {
        Some(SortField::Title) => items.sort_by_key(|item| item.title.to_lowercase()),
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

fn child_to_annotation_record(child: ChildItem) -> AnnotationRecord {
    AnnotationRecord {
        key: child.key,
        parent_key: None,
        parent_title: None,
        attachment_key: child.parent_key,
        attachment_title: None,
        annotation_type: child
            .annotation_type
            .unwrap_or_else(|| "unknown".to_string()),
        text: child.text.unwrap_or_default(),
        comment: child.comment.unwrap_or_default(),
        color: child.color,
        page_label: child.page_label,
        tags: child.tags,
    }
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

#[cfg(test)]
mod tests {
    use rusqlite::Connection;
    use tempfile::TempDir;
    use zot_core::LibraryScope;

    use super::{DuplicateMatchMethod, LocalLibrary, SearchOptions};

    struct TestFixture {
        lib: LocalLibrary,
        _dir: TempDir,
    }

    fn rich_fixture_library() -> TestFixture {
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
        assert!(
            children
                .iter()
                .any(|child| child.item_type == "note" && child.key == "NOTE004")
        );
        assert!(
            children
                .iter()
                .any(|child| child.item_type == "attachment" && child.key == "ATCH005")
        );
        assert!(children.iter().any(|child| {
            child.item_type == "annotation"
                && child.key == "ANNO011"
                && child.annotation_type.as_deref() == Some("highlight")
        }));

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
            let groups = match lib.find_duplicates(method, None, 10) {
                Ok(groups) => groups,
                Err(err) => panic!("find duplicates failed for {:?}: {err}", method),
            };
            assert!(groups.iter().any(|group| {
                let keys = group
                    .items
                    .iter()
                    .map(|item| item.key.as_str())
                    .collect::<Vec<_>>();
                keys.contains(&"ATTN001") && keys.contains(&"DUPE008")
            }));
        }
    }
}
