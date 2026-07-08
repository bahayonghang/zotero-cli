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

use crate::pdf::{PdfBackend, PdfCache};
use crate::rag_engine::{self, PendingEmbedding, RagLibrary, ReindexStats};
use crate::workspace::{HybridMode, RagIndex};
use crate::LocalLibrary;

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
    pub fn search(
        &self,
        library: &LocalLibrary,
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
