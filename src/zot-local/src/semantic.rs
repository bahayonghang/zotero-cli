//! Facade over the RAG index pipeline.
//!
//! The CLI used to orchestrate `LocalLibrary + RagIndex + PdfCache + PdfBackend +
//! EmbeddingClient` directly. `SemanticStore` collapses that choreography behind
//! a small, synchronous-except-for-embedding surface. Embeddings stay in the
//! caller because they involve async network I/O.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use zot_core::{Item, SemanticHit, SemanticIndexStatus, ZotError, ZotResult};

use crate::LocalLibrary;
use crate::pdf::{PdfBackend, PdfCache};
use crate::workspace::{
    HybridMode, RagIndex, build_metadata_chunk, chunk_text, compute_term_frequencies, tokenize,
};

const CHUNK_MAX_TOKENS: usize = 500;
const CHUNK_OVERLAP_TOKENS: usize = 50;

/// Options controlling the reindex pass.
pub struct ReindexOpts<'a> {
    pub items: &'a [Item],
    pub fulltext: bool,
    pub force_rebuild: bool,
}

/// A chunk that has been persisted to the index but still needs an embedding
/// vector written back via [`SemanticStore::apply_embeddings`].
#[derive(Debug, Clone)]
pub struct PendingEmbedding {
    pub chunk_id: i64,
    pub text: String,
}

/// Summary returned by [`SemanticStore::reindex_chunks`].
#[derive(Debug, Clone, Default)]
pub struct ReindexStats {
    pub items: usize,
    pub chunks: usize,
    pub fulltext: bool,
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
    /// Returns the chunks that still need embeddings applied.
    pub fn reindex_chunks<B: PdfBackend>(
        &self,
        library: &LocalLibrary,
        backend: &B,
        opts: ReindexOpts<'_>,
    ) -> ZotResult<(ReindexStats, Vec<PendingEmbedding>)> {
        let mut stats = ReindexStats {
            items: opts.items.len(),
            chunks: 0,
            fulltext: opts.fulltext,
        };
        let mut pending = Vec::new();
        self.index.with_write_tx(|| {
            if !opts.force_rebuild {
                for item in opts.items {
                    self.index.remove_item_chunks(&item.key)?;
                }
                for key in self.index.indexed_keys()? {
                    if library.get_item(&key)?.is_none() {
                        self.index.remove_item_chunks(&key)?;
                    }
                }
            }
            for item in opts.items {
                let metadata_chunk = build_metadata_chunk(item);
                let chunk_id = self
                    .index
                    .insert_chunk(&item.key, "metadata", &metadata_chunk)?;
                self.index.insert_terms(
                    chunk_id,
                    &compute_term_frequencies(&tokenize(&metadata_chunk)),
                )?;
                pending.push(PendingEmbedding {
                    chunk_id,
                    text: metadata_chunk,
                });
                stats.chunks += 1;

                if opts.fulltext
                    && let Some(attachment) = library.get_pdf_attachment(&item.key)?
                {
                    let pdf_path = library.pdf_path(&attachment);
                    let text = self.pdf_text(backend, &pdf_path)?;
                    for chunk in
                        chunk_text(&text, &item.title, CHUNK_MAX_TOKENS, CHUNK_OVERLAP_TOKENS)
                    {
                        let chunk_id = self.index.insert_chunk(&item.key, "pdf", &chunk)?;
                        self.index
                            .insert_terms(chunk_id, &compute_term_frequencies(&tokenize(&chunk)))?;
                        pending.push(PendingEmbedding {
                            chunk_id,
                            text: chunk,
                        });
                        stats.chunks += 1;
                    }
                }
            }
            Ok(())
        })?;
        Ok((stats, pending))
    }

    fn pdf_text<B: PdfBackend>(&self, backend: &B, pdf_path: &Path) -> ZotResult<String> {
        if let Some(cache) = &self.md_cache {
            if let Some(cached) = cache.get(pdf_path)? {
                return Ok(cached);
            }
            let extracted = backend.extract_text(pdf_path, None)?;
            cache.put(pdf_path, &extracted)?;
            Ok(extracted)
        } else {
            backend.extract_text(pdf_path, None)
        }
    }

    /// Write back embeddings for the pending chunks returned by
    /// `reindex_chunks`. All writes happen inside one transaction.
    pub fn apply_embeddings(&self, pairs: &[(i64, Vec<f32>)]) -> ZotResult<()> {
        if pairs.is_empty() {
            return Ok(());
        }
        self.index.with_write_tx(|| {
            for (chunk_id, embedding) in pairs {
                self.index.set_embedding(*chunk_id, embedding)?;
            }
            Ok(())
        })
    }

    /// Convenience wrapper for callers that have paired pending chunks with
    /// the matching embeddings in order.
    pub fn apply_pending_embeddings(
        &self,
        pending: Vec<PendingEmbedding>,
        embeddings: Vec<Vec<f32>>,
    ) -> ZotResult<()> {
        if pending.len() != embeddings.len() {
            return Err(ZotError::InvalidInput {
                code: "embedding-count-mismatch".to_string(),
                message: format!(
                    "Embedding count {} does not match pending chunks {}",
                    embeddings.len(),
                    pending.len()
                ),
                hint: None,
            });
        }
        let pairs: Vec<(i64, Vec<f32>)> = pending
            .into_iter()
            .map(|p| p.chunk_id)
            .zip(embeddings)
            .collect();
        self.apply_embeddings(&pairs)
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
