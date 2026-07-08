//! Narrow, per-data-domain read traits over [`LocalLibrary`].
//!
//! `LocalLibrary` exposes ~40 inherent methods across ~10 responsibilities;
//! consumers that only read one data domain used to depend on the whole
//! concrete type, which blocked handler-level tests. Each trait below mirrors
//! the inherent `db.rs` signatures exactly and `LocalLibrary` implements them
//! by thin delegation, so callers can accept `&impl ItemReader` (etc.) and
//! tests can substitute fakes. SQL ownership is unchanged — this module only
//! narrows interfaces. The engine-side sibling with the same shape is
//! [`crate::rag_engine::RagLibrary`].

use std::path::PathBuf;

use zot_core::{
    Attachment, Collection, Item, Note, NoteSearchResult, SearchResult, TagSummary, ZotResult,
};

use crate::db::{LocalLibrary, SearchOptions, SortField};

/// Item lookup, listing, and search.
pub trait ItemReader {
    fn get_item(&self, key: &str) -> ZotResult<Option<Item>>;
    fn list_items(
        &self,
        collection: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> ZotResult<Vec<Item>>;
    fn search(&self, options: SearchOptions) -> ZotResult<SearchResult>;
    fn get_recent_items(&self, since: &str, sort: SortField, limit: usize) -> ZotResult<Vec<Item>>;
    fn get_recent_items_by_count(&self, count: usize) -> ZotResult<Vec<Item>>;
}

/// Collection tree navigation and lookup.
pub trait CollectionNav {
    fn get_collections(&self) -> ZotResult<Vec<Collection>>;
    fn get_collection(&self, collection_key: &str) -> ZotResult<Option<Collection>>;
    fn get_subcollections(&self, collection_key: &str) -> ZotResult<Vec<Collection>>;
    fn search_collections(&self, query: &str, limit: usize) -> ZotResult<Vec<Collection>>;
}

/// What a collection contains: items, counts, tags.
pub trait CollectionContent {
    fn get_collection_items(&self, collection_key: &str) -> ZotResult<Vec<Item>>;
    fn get_collection_item_count(&self, collection_key: &str) -> ZotResult<usize>;
    fn get_collection_tags(&self, collection_key: &str) -> ZotResult<Vec<TagSummary>>;
}

/// Notes attached to items, plus note search.
pub trait NoteReader {
    fn get_notes(&self, key: &str) -> ZotResult<Vec<Note>>;
    fn search_notes(&self, query: &str, limit: usize) -> ZotResult<Vec<NoteSearchResult>>;
}

/// Attachment lookup and on-disk path resolution.
pub trait AttachmentSource {
    fn get_attachments(&self, key: &str) -> ZotResult<Vec<Attachment>>;
    fn get_attachment_by_key(&self, key: &str) -> ZotResult<Option<Attachment>>;
    fn get_pdf_attachment(&self, key: &str) -> ZotResult<Option<Attachment>>;
    fn attachment_path(&self, attachment: &Attachment) -> PathBuf;
    fn pdf_path(&self, attachment: &Attachment) -> PathBuf;
}

impl ItemReader for LocalLibrary {
    fn get_item(&self, key: &str) -> ZotResult<Option<Item>> {
        self.get_item(key)
    }
    fn list_items(
        &self,
        collection: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> ZotResult<Vec<Item>> {
        self.list_items(collection, limit, offset)
    }
    fn search(&self, options: SearchOptions) -> ZotResult<SearchResult> {
        self.search(options)
    }
    fn get_recent_items(&self, since: &str, sort: SortField, limit: usize) -> ZotResult<Vec<Item>> {
        self.get_recent_items(since, sort, limit)
    }
    fn get_recent_items_by_count(&self, count: usize) -> ZotResult<Vec<Item>> {
        self.get_recent_items_by_count(count)
    }
}

impl CollectionNav for LocalLibrary {
    fn get_collections(&self) -> ZotResult<Vec<Collection>> {
        self.get_collections()
    }
    fn get_collection(&self, collection_key: &str) -> ZotResult<Option<Collection>> {
        self.get_collection(collection_key)
    }
    fn get_subcollections(&self, collection_key: &str) -> ZotResult<Vec<Collection>> {
        self.get_subcollections(collection_key)
    }
    fn search_collections(&self, query: &str, limit: usize) -> ZotResult<Vec<Collection>> {
        self.search_collections(query, limit)
    }
}

impl CollectionContent for LocalLibrary {
    fn get_collection_items(&self, collection_key: &str) -> ZotResult<Vec<Item>> {
        self.get_collection_items(collection_key)
    }
    fn get_collection_item_count(&self, collection_key: &str) -> ZotResult<usize> {
        self.get_collection_item_count(collection_key)
    }
    fn get_collection_tags(&self, collection_key: &str) -> ZotResult<Vec<TagSummary>> {
        self.get_collection_tags(collection_key)
    }
}

impl NoteReader for LocalLibrary {
    fn get_notes(&self, key: &str) -> ZotResult<Vec<Note>> {
        self.get_notes(key)
    }
    fn search_notes(&self, query: &str, limit: usize) -> ZotResult<Vec<NoteSearchResult>> {
        self.search_notes(query, limit)
    }
}

impl AttachmentSource for LocalLibrary {
    fn get_attachments(&self, key: &str) -> ZotResult<Vec<Attachment>> {
        self.get_attachments(key)
    }
    fn get_attachment_by_key(&self, key: &str) -> ZotResult<Option<Attachment>> {
        self.get_attachment_by_key(key)
    }
    fn get_pdf_attachment(&self, key: &str) -> ZotResult<Option<Attachment>> {
        self.get_pdf_attachment(key)
    }
    fn attachment_path(&self, attachment: &Attachment) -> PathBuf {
        self.attachment_path(attachment)
    }
    fn pdf_path(&self, attachment: &Attachment) -> PathBuf {
        self.pdf_path(attachment)
    }
}
