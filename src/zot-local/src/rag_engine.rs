//! Shared indexing engine behind the two RAG facades.
//!
//! `SemanticStore` (whole library) and `WorkspaceRagStore` (per workspace) used
//! to duplicate the reindex choreography: item walk, PDF text cache, chunking,
//! pending-embedding collection, and embedding writeback. This module owns that
//! single loop, parameterized by a prune predicate (`is_stale`) and a
//! [`RefreshPolicy`], plus the shared embedding-dimension tracking. The
//! underlying `RagIndex` is unchanged; the facades keep only scope construction.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use zot_core::{Attachment, Item, ZotError, ZotResult};

use crate::LocalLibrary;
use crate::pdf::{PdfBackend, PdfCache};
use crate::workspace::{
    HybridMode, RagIndex, build_metadata_chunk, chunk_text, compute_term_frequencies, tokenize,
};

pub(crate) const CHUNK_MAX_TOKENS: usize = 500;
pub(crate) const CHUNK_OVERLAP_TOKENS: usize = 50;
pub(crate) const EMBEDDING_DIM_META: &str = "embedding.dim";

/// Read-only library surface the reindex loop needs to resolve items and PDFs.
pub trait RagLibrary {
    fn get_item(&self, key: &str) -> ZotResult<Option<Item>>;
    fn get_pdf_attachment(&self, key: &str) -> ZotResult<Option<Attachment>>;
    fn pdf_path(&self, attachment: &Attachment) -> PathBuf;
}

impl RagLibrary for LocalLibrary {
    fn get_item(&self, key: &str) -> ZotResult<Option<Item>> {
        self.get_item(key)
    }
    fn get_pdf_attachment(&self, key: &str) -> ZotResult<Option<Attachment>> {
        self.get_pdf_attachment(key)
    }
    fn pdf_path(&self, attachment: &Attachment) -> PathBuf {
        self.pdf_path(attachment)
    }
}

/// A chunk persisted to the index that still needs an embedding vector written
/// back via [`apply_pending_embeddings`].
#[derive(Debug, Clone)]
pub struct PendingEmbedding {
    pub chunk_id: i64,
    pub text: String,
}

/// Summary of a reindex pass.
#[derive(Debug, Clone, Default)]
pub struct ReindexStats {
    pub items: usize,
    pub chunks: usize,
    pub fulltext: bool,
}

/// How the incremental (non-rebuild) branch treats already-indexed keys.
pub(crate) enum RefreshPolicy {
    /// Rebuild every requested key: drop its chunks first, then re-insert.
    ReplaceRequested,
    /// Keep already-indexed keys verbatim; only process requested newcomers.
    SkipIndexed,
}

/// Reindex `requested` keys in a single write transaction.
///
/// `is_stale(key)` decides whether an already-indexed key should be pruned
/// (gone from the library for the semantic store, gone from the workspace for
/// the workspace store). `force_rebuild` clears the whole index first.
#[allow(clippy::too_many_arguments)]
pub(crate) fn reindex<L, B, P>(
    index: &RagIndex,
    library: &L,
    backend: &B,
    md_cache: Option<&PdfCache>,
    requested: &[&str],
    refresh: RefreshPolicy,
    is_stale: P,
    fulltext: bool,
    force_rebuild: bool,
) -> ZotResult<(ReindexStats, Vec<PendingEmbedding>)>
where
    L: RagLibrary,
    B: PdfBackend,
    P: Fn(&str) -> ZotResult<bool>,
{
    let mut stats = ReindexStats {
        items: requested.len(),
        chunks: 0,
        fulltext,
    };
    let mut pending = Vec::new();
    index.with_write_tx(|| {
        let to_process: Vec<&str> = if force_rebuild {
            index.clear()?;
            requested.to_vec()
        } else {
            match refresh {
                RefreshPolicy::ReplaceRequested => {
                    for key in requested {
                        index.remove_item_chunks(key)?;
                    }
                    for key in index.indexed_keys()? {
                        if is_stale(&key)? {
                            index.remove_item_chunks(&key)?;
                        }
                    }
                    requested.to_vec()
                }
                RefreshPolicy::SkipIndexed => {
                    let indexed = index.indexed_keys()?;
                    for key in &indexed {
                        if is_stale(key)? {
                            index.remove_item_chunks(key)?;
                        }
                    }
                    let indexed_set: HashSet<&str> = indexed.iter().map(String::as_str).collect();
                    requested
                        .iter()
                        .copied()
                        .filter(|key| !indexed_set.contains(key))
                        .collect()
                }
            }
        };
        for key in to_process {
            let Some(item) = library.get_item(key)? else {
                continue;
            };
            let metadata_chunk = build_metadata_chunk(&item);
            let chunk_id = index.insert_chunk(&item.key, "metadata", &metadata_chunk)?;
            index.insert_terms(
                chunk_id,
                &compute_term_frequencies(&tokenize(&metadata_chunk)),
            )?;
            pending.push(PendingEmbedding {
                chunk_id,
                text: metadata_chunk,
            });
            stats.chunks += 1;
            if fulltext {
                if let Some(attachment) = library.get_pdf_attachment(&item.key)? {
                    let pdf_path = library.pdf_path(&attachment);
                    let text = pdf_text(backend, md_cache, &pdf_path)?;
                    for chunk in
                        chunk_text(&text, &item.title, CHUNK_MAX_TOKENS, CHUNK_OVERLAP_TOKENS)
                    {
                        let chunk_id = index.insert_chunk(&item.key, "pdf", &chunk)?;
                        index
                            .insert_terms(chunk_id, &compute_term_frequencies(&tokenize(&chunk)))?;
                        pending.push(PendingEmbedding {
                            chunk_id,
                            text: chunk,
                        });
                        stats.chunks += 1;
                    }
                }
            }
        }
        Ok(())
    })?;
    Ok((stats, pending))
}

/// Extract PDF text, consulting and refreshing the markdown cache when present.
fn pdf_text<B: PdfBackend>(
    backend: &B,
    cache: Option<&PdfCache>,
    pdf_path: &Path,
) -> ZotResult<String> {
    if let Some(cache) = cache {
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

/// Write embeddings back for `pending` in one transaction. Enforces a single
/// dimension per batch and records it as `embedding.dim` for query-time checks.
pub(crate) fn apply_pending_embeddings(
    index: &RagIndex,
    pending: Vec<PendingEmbedding>,
    embeddings: Vec<Vec<f32>>,
) -> ZotResult<()> {
    if pending.len() != embeddings.len() {
        return Err(ZotError::InvalidInput {
            code: "embedding-count-mismatch".into(),
            message: format!(
                "Embedding count {} does not match pending chunks {}",
                embeddings.len(),
                pending.len()
            ),
            hint: None,
        });
    }
    let embedding_dim = embeddings.first().map(Vec::len);
    if let Some(dim) = embedding_dim {
        if embeddings.iter().any(|embedding| embedding.len() != dim) {
            return Err(ZotError::InvalidInput {
                code: "embedding-dimension-mismatch".into(),
                message: "Embeddings in one writeback batch must have the same dimension".into(),
                hint: None,
            });
        }
    }
    index.with_write_tx(|| {
        for (p, e) in pending.into_iter().zip(embeddings) {
            index.set_embedding(p.chunk_id, &e)?;
        }
        if let Some(dim) = embedding_dim {
            index.set_meta(EMBEDDING_DIM_META, &dim.to_string())?;
        }
        Ok(())
    })
}

/// Reject a query embedding whose dimension disagrees with the indexed vectors.
/// A non-semantic mode, absent embedding, or legacy index without a recorded
/// dimension all skip the check.
pub(crate) fn validate_query_embedding(
    index: &RagIndex,
    mode: HybridMode,
    embedding: Option<&[f32]>,
) -> ZotResult<()> {
    if !matches!(mode, HybridMode::Semantic | HybridMode::Hybrid) {
        return Ok(());
    }
    let Some(embedding) = embedding else {
        return Ok(());
    };
    let Some(expected) = index
        .get_meta(EMBEDDING_DIM_META)?
        .and_then(|raw| raw.parse::<usize>().ok())
    else {
        return Ok(());
    };
    if embedding.len() == expected {
        return Ok(());
    }
    Err(ZotError::InvalidInput {
        code: "embedding-dimension-mismatch".into(),
        message: format!(
            "Query embedding dimension {} does not match index embedding dimension {}",
            embedding.len(),
            expected
        ),
        hint: Some("Reindex with the active embedding model.".into()),
    })
}
