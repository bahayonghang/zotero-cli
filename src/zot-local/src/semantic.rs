//! Facade over the RAG index pipeline.
//!
//! The CLI used to orchestrate `LocalLibrary + RagIndex + PdfCache + PdfBackend +
//! EmbeddingClient` directly. `SemanticStore` collapses that choreography behind
//! a small, synchronous-except-for-embedding surface. Embeddings stay in the
//! caller because they involve async network I/O. The shared reindex loop and
//! embedding-dimension tracking live in [`crate::rag_engine`].

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use zot_core::{Item, SemanticHit, SemanticIndexStatus, ZotResult};

use crate::library_traits::{CollectionContent, ItemReader};
use crate::pdf::{PdfBackend, PdfCache};
use crate::rag_engine::{self, PendingEmbedding, RagLibrary, ReindexStats};
use crate::workspace::{HybridMode, RagIndex};

/// Options controlling the reindex pass.
pub struct ReindexOpts<'a> {
    pub items: &'a [Item],
    pub fulltext: bool,
    pub force_rebuild: bool,
}

/// Front door for the library's semantic index. Wraps the `RagIndex` together
/// with the optional PDF markdown cache so callers don't have to thread them
/// everywhere.
pub struct SemanticStore {
    index: RagIndex,
    index_path: PathBuf,
    md_cache: Option<PdfCache>,
}

impl SemanticStore {
    /// Open (or create) the index at `index_path`. Supply `pdf_md_cache_path`
    /// when reindex should cache extracted PDF text between runs.
    pub fn open(
        index_path: impl AsRef<Path>,
        pdf_md_cache_path: Option<PathBuf>,
    ) -> ZotResult<Self> {
        let index_path = index_path.as_ref().to_path_buf();
        let index = RagIndex::open(&index_path)?;
        let md_cache = match pdf_md_cache_path {
            Some(path) => Some(PdfCache::new(Some(path))?),
            None => None,
        };
        Ok(Self {
            index,
            index_path,
            md_cache,
        })
    }

    /// Fetch status without creating the index file on disk.
    pub fn status_at(path: impl AsRef<Path>) -> ZotResult<SemanticIndexStatus> {
        let path = path.as_ref();
        if !path.exists() {
            return Ok(SemanticIndexStatus {
                exists: false,
                path: path.display().to_string(),
                indexed_items: 0,
                indexed_chunks: 0,
                chunks_with_embeddings: 0,
                last_indexed_at: None,
            });
        }
        let store = Self::open(path, None)?;
        store.status()
    }

    pub fn status(&self) -> ZotResult<SemanticIndexStatus> {
        Ok(SemanticIndexStatus {
            exists: true,
            path: self.index_path.display().to_string(),
            indexed_items: self.index.indexed_keys()?.len(),
            indexed_chunks: self.index.chunk_count()?,
            chunks_with_embeddings: self.index.embedding_count()?,
            last_indexed_at: self.index.get_meta("indexed_at")?,
        })
    }

    pub fn clear(&self) -> ZotResult<()> {
        self.index.clear()
    }

    pub fn mark_indexed_at(&self, timestamp: &str) -> ZotResult<()> {
        self.index.set_meta("indexed_at", timestamp)
    }

    /// Rebuild chunks + BM25 terms for the given items in a single transaction.
    /// Returns the chunks that still need embeddings applied; already-indexed
    /// keys no longer present in the library are pruned.
    pub fn reindex_chunks<L: RagLibrary, B: PdfBackend>(
        &self,
        library: &L,
        backend: &B,
        opts: ReindexOpts<'_>,
    ) -> ZotResult<(ReindexStats, Vec<PendingEmbedding>)> {
        let keys: Vec<&str> = opts.items.iter().map(|item| item.key.as_str()).collect();
        rag_engine::reindex(
            &self.index,
            library,
            backend,
            self.md_cache.as_ref(),
            &keys,
            rag_engine::RefreshPolicy::ReplaceRequested,
            |key| Ok(library.get_item(key)?.is_none()),
            opts.fulltext,
            opts.force_rebuild,
        )
    }

    /// Write back embeddings for the pending chunks returned by
    /// `reindex_chunks`, recording the batch dimension for query-time checks.
    pub fn apply_pending_embeddings(
        &self,
        pending: Vec<PendingEmbedding>,
        embeddings: Vec<Vec<f32>>,
    ) -> ZotResult<()> {
        rag_engine::apply_pending_embeddings(&self.index, pending, embeddings)
    }

    /// Hybrid search over the index. `allowed_collection` narrows the result
    /// set to items that belong to that Zotero collection key.
    pub fn search<L: ItemReader + CollectionContent>(
        &self,
        library: &L,
        query: &str,
        mode: HybridMode,
        query_embedding: Option<&[f32]>,
        allowed_collection: Option<&str>,
        limit: usize,
    ) -> ZotResult<Vec<SemanticHit>> {
        rag_engine::validate_query_embedding(&self.index, mode, query_embedding)?;

        let allowed_keys: HashSet<String> = match allowed_collection {
            Some(collection) => library
                .get_collection_items(collection)?
                .into_iter()
                .map(|item| item.key)
                .collect(),
            None => HashSet::new(),
        };

        let chunks = self
            .index
            .query(query, mode, query_embedding, limit.saturating_mul(5))?;
        let mut deduped: BTreeMap<String, SemanticHit> = BTreeMap::new();
        for chunk in chunks {
            if !allowed_keys.is_empty() && !allowed_keys.contains(&chunk.item_key) {
                continue;
            }
            if let Some(item) = library.get_item(&chunk.item_key)? {
                let entry = deduped
                    .entry(item.key.clone())
                    .or_insert_with(|| SemanticHit {
                        item: item.clone(),
                        score: chunk.score,
                        source: chunk.source.clone(),
                        matched_chunk: Some(chunk.content.clone()),
                    });
                if chunk.score > entry.score {
                    entry.score = chunk.score;
                    entry.source = chunk.source.clone();
                    entry.matched_chunk = Some(chunk.content.clone());
                }
            }
            if deduped.len() >= limit {
                break;
            }
        }
        Ok(deduped.into_values().collect())
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use tempfile::tempdir;

    use super::*;
    use crate::db::{SearchOptions, SortField};
    use crate::rag_engine::RagLibrary;
    use zot_core::{Attachment, Creator, Item, SearchResult, TagSummary, ZotError};

    /// Fake spanning the three narrow faces `SemanticStore` consumes:
    /// `RagLibrary` for reindex, `ItemReader` + `CollectionContent` for search.
    /// Methods a test must never reach return a `not-used` error.
    struct FakeLibrary {
        items: Vec<Item>,
        collection_key: String,
        collection_members: Vec<String>,
    }

    fn not_used<T>() -> ZotResult<T> {
        Err(ZotError::InvalidInput {
            code: "not-used".into(),
            message: "not used by this test".into(),
            hint: None,
        })
    }

    impl RagLibrary for FakeLibrary {
        fn get_item(&self, key: &str) -> ZotResult<Option<Item>> {
            Ok(self.items.iter().find(|item| item.key == key).cloned())
        }
        fn get_pdf_attachment(&self, _key: &str) -> ZotResult<Option<Attachment>> {
            Ok(None)
        }
        fn pdf_path(&self, _attachment: &Attachment) -> PathBuf {
            PathBuf::new()
        }
    }

    impl ItemReader for FakeLibrary {
        fn get_item(&self, key: &str) -> ZotResult<Option<Item>> {
            Ok(self.items.iter().find(|item| item.key == key).cloned())
        }
        fn list_items(
            &self,
            _collection: Option<&str>,
            _limit: usize,
            _offset: usize,
        ) -> ZotResult<Vec<Item>> {
            not_used()
        }
        fn search(&self, _options: SearchOptions) -> ZotResult<SearchResult> {
            not_used()
        }
        fn get_recent_items(
            &self,
            _since: &str,
            _sort: SortField,
            _limit: usize,
        ) -> ZotResult<Vec<Item>> {
            not_used()
        }
        fn get_recent_items_by_count(&self, _count: usize) -> ZotResult<Vec<Item>> {
            not_used()
        }
    }

    impl CollectionContent for FakeLibrary {
        fn get_collection_items(&self, collection_key: &str) -> ZotResult<Vec<Item>> {
            if collection_key != self.collection_key {
                return Ok(Vec::new());
            }
            Ok(self
                .items
                .iter()
                .filter(|item| self.collection_members.contains(&item.key))
                .cloned()
                .collect())
        }
        fn get_collection_item_count(&self, _collection_key: &str) -> ZotResult<usize> {
            not_used()
        }
        fn get_collection_tags(&self, _collection_key: &str) -> ZotResult<Vec<TagSummary>> {
            not_used()
        }
    }

    /// Backend that must never be touched: every test below indexes metadata
    /// only (`fulltext: false`).
    struct UntouchedBackend;

    impl PdfBackend for UntouchedBackend {
        fn availability_hint(&self) -> ZotResult<()> {
            not_used()
        }
        fn extract_text(
            &self,
            _pdf_path: &Path,
            _page_range: Option<(usize, usize)>,
        ) -> ZotResult<String> {
            not_used()
        }
        fn extract_annotations(
            &self,
            _pdf_path: &Path,
        ) -> ZotResult<Vec<zot_core::AnnotationSnippet>> {
            not_used()
        }
        fn extract_outline(&self, _pdf_path: &Path) -> ZotResult<Vec<zot_core::PdfOutlineEntry>> {
            not_used()
        }
        fn find_text_position(
            &self,
            _pdf_path: &Path,
            _page: usize,
            _text: &str,
            _occurrence: usize,
        ) -> ZotResult<Option<crate::pdf::PdfMatchPosition>> {
            not_used()
        }
        fn build_area_position(
            &self,
            _pdf_path: &Path,
            _page: usize,
            _x: f32,
            _y: f32,
            _width: f32,
            _height: f32,
        ) -> ZotResult<crate::pdf::PdfAreaPosition> {
            not_used()
        }
    }

    fn sample_item(key: &str, title: &str, abstract_note: &str) -> Item {
        Item {
            key: key.into(),
            item_type: "journalArticle".into(),
            title: title.into(),
            creators: vec![Creator {
                first_name: "Ashish".into(),
                last_name: "Vaswani".into(),
                creator_type: "author".into(),
            }],
            abstract_note: Some(abstract_note.into()),
            date: None,
            url: None,
            doi: None,
            tags: vec![],
            collections: vec![],
            date_added: None,
            date_modified: None,
            extra: Default::default(),
        }
    }

    fn fake_library() -> FakeLibrary {
        FakeLibrary {
            items: vec![
                sample_item(
                    "ATTN001",
                    "Attention Is All You Need",
                    "Transformer architecture",
                ),
                sample_item(
                    "BERT002",
                    "BERT Pretraining",
                    "Transformer encoder pretraining",
                ),
            ],
            collection_key: "COLL1".into(),
            collection_members: vec!["BERT002".into()],
        }
    }

    fn indexed_store(dir: &Path, library: &FakeLibrary) -> SemanticStore {
        let store = SemanticStore::open(dir.join("idx.sqlite"), None).expect("open store");
        let (stats, pending) = store
            .reindex_chunks(
                library,
                &UntouchedBackend,
                ReindexOpts {
                    items: &library.items,
                    fulltext: false,
                    force_rebuild: false,
                },
            )
            .expect("reindex via fakes");
        assert_eq!(stats.items, 2);
        assert_eq!(pending.len(), 2, "one metadata chunk per item");
        store
    }

    #[test]
    fn reindex_then_search_works_entirely_through_fakes() {
        let dir = tempdir().expect("tempdir");
        let library = fake_library();
        let store = indexed_store(dir.path(), &library);
        let hits = store
            .search(&library, "attention", HybridMode::Bm25, None, None, 10)
            .expect("search via fakes");
        assert!(!hits.is_empty());
        assert_eq!(hits[0].item.key, "ATTN001");
        assert!(hits[0].matched_chunk.is_some());
    }

    #[test]
    fn search_narrows_results_to_the_allowed_collection() {
        let dir = tempdir().expect("tempdir");
        let library = fake_library();
        let store = indexed_store(dir.path(), &library);
        // Both items match "transformer"; the collection filter must keep
        // only the member resolved through `CollectionContent`.
        let unfiltered = store
            .search(&library, "transformer", HybridMode::Bm25, None, None, 10)
            .expect("unfiltered search");
        assert_eq!(unfiltered.len(), 2);
        let filtered = store
            .search(
                &library,
                "transformer",
                HybridMode::Bm25,
                None,
                Some("COLL1"),
                10,
            )
            .expect("filtered search");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].item.key, "BERT002");
    }
}
