use std::path::{Path, PathBuf};

use zot_core::{Item, QueryChunk, Workspace, ZotError, ZotResult};

use crate::db::LocalLibrary;
use crate::pdf::{PdfBackend, PdfCache};
use crate::workspace::{
    HybridMode, RagIndex, build_metadata_chunk, chunk_text, compute_term_frequencies, tokenize,
};

const CHUNK_MAX_TOKENS: usize = 500;
const CHUNK_OVERLAP_TOKENS: usize = 50;
const EMBEDDING_DIM_META: &str = "embedding.dim";

#[derive(Debug, Clone)]
pub struct PendingEmbedding {
    pub chunk_id: i64,
    pub text: String,
}

#[derive(Debug, Clone, Default)]
pub struct WorkspaceReindexStats {
    pub items: usize,
    pub chunks: usize,
    pub fulltext: bool,
}

pub trait WorkspaceRagLibrary {
    fn get_item(&self, key: &str) -> ZotResult<Option<Item>>;
    fn get_pdf_attachment(&self, key: &str) -> ZotResult<Option<zot_core::Attachment>>;
    fn pdf_path(&self, attachment: &zot_core::Attachment) -> PathBuf;
}

impl WorkspaceRagLibrary for LocalLibrary {
    fn get_item(&self, key: &str) -> ZotResult<Option<Item>> {
        self.get_item(key)
    }
    fn get_pdf_attachment(&self, key: &str) -> ZotResult<Option<zot_core::Attachment>> {
        self.get_pdf_attachment(key)
    }
    fn pdf_path(&self, attachment: &zot_core::Attachment) -> PathBuf {
        self.pdf_path(attachment)
    }
}

pub struct WorkspaceRagStore {
    index: RagIndex,
    index_path: PathBuf,
    cache_path: PathBuf,
}

impl WorkspaceRagStore {
    pub fn open(store: &crate::workspace::WorkspaceStore, workspace_name: &str) -> ZotResult<Self> {
        let index_path = store.root().join(format!("{workspace_name}.idx.sqlite"));
        let md_cache_path = store.root().join(".md_cache.sqlite");
        Ok(Self {
            index: RagIndex::open(&index_path)?,
            index_path,
            cache_path: md_cache_path.clone(),
        })
    }

    pub fn index_path(&self) -> &Path {
        &self.index_path
    }
    pub fn cache_path(&self) -> &Path {
        &self.cache_path
    }

    pub fn clear(&self) -> ZotResult<()> {
        self.index.clear()
    }

    pub fn reindex_workspace<L: WorkspaceRagLibrary, B: PdfBackend>(
        &self,
        library: &L,
        workspace: &Workspace,
        backend: &B,
        fulltext: bool,
    ) -> ZotResult<(WorkspaceReindexStats, Vec<PendingEmbedding>)> {
        let mut stats = WorkspaceReindexStats {
            items: workspace.items.len(),
            chunks: 0,
            fulltext,
        };
        let mut pending = Vec::new();
        let md_cache = if fulltext {
            Some(PdfCache::new(Some(self.cache_path.clone()))?)
        } else {
            None
        };
        self.index.with_write_tx(|| {
            self.index.clear()?;
            for entry in &workspace.items {
                if let Some(item) = library.get_item(&entry.key)? {
                    let metadata_chunk = build_metadata_chunk(&item);
                    let chunk_id =
                        self.index
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
                    if fulltext && let Some(attachment) = library.get_pdf_attachment(&item.key)? {
                        let pdf_path = library.pdf_path(&attachment);
                        let text = self.pdf_text(backend, md_cache.as_ref(), &pdf_path)?;
                        for chunk in
                            chunk_text(&text, &item.title, CHUNK_MAX_TOKENS, CHUNK_OVERLAP_TOKENS)
                        {
                            let chunk_id = self.index.insert_chunk(&item.key, "pdf", &chunk)?;
                            self.index.insert_terms(
                                chunk_id,
                                &compute_term_frequencies(&tokenize(&chunk)),
                            )?;
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

    pub fn apply_pending_embeddings(
        &self,
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
        if let Some(dim) = embedding_dim
            && embeddings.iter().any(|embedding| embedding.len() != dim)
        {
            return Err(ZotError::InvalidInput {
                code: "embedding-dimension-mismatch".into(),
                message: "Embeddings in one writeback batch must have the same dimension".into(),
                hint: None,
            });
        }
        self.index.with_write_tx(|| {
            for (p, e) in pending.into_iter().zip(embeddings) {
                self.index.set_embedding(p.chunk_id, &e)?;
            }
            if let Some(dim) = embedding_dim {
                self.index.set_meta(EMBEDDING_DIM_META, &dim.to_string())?;
            }
            Ok(())
        })
    }

    pub fn query(
        &self,
        question: &str,
        mode: HybridMode,
        embedding: Option<&[f32]>,
        limit: usize,
    ) -> ZotResult<Vec<QueryChunk>> {
        self.validate_query_embedding(mode, embedding)?;
        self.index.query(question, mode, embedding, limit)
    }

    pub fn query_workspace(
        &self,
        workspace: &Workspace,
        question: &str,
        mode: HybridMode,
        embedding: Option<&[f32]>,
        limit: usize,
    ) -> ZotResult<Vec<QueryChunk>> {
        let allowed_keys = workspace
            .items
            .iter()
            .map(|item| item.key.as_str())
            .collect::<Vec<_>>();
        if allowed_keys.is_empty() {
            return Ok(Vec::new());
        }
        self.validate_query_embedding(mode, embedding)?;
        self.index
            .query_allowed(question, mode, embedding, &allowed_keys, limit)
    }

    fn validate_query_embedding(
        &self,
        mode: HybridMode,
        embedding: Option<&[f32]>,
    ) -> ZotResult<()> {
        if !matches!(mode, HybridMode::Semantic | HybridMode::Hybrid) {
            return Ok(());
        }
        let Some(embedding) = embedding else {
            return Ok(());
        };
        let Some(expected) = self
            .index
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
                "Query embedding dimension {} does not match workspace index dimension {}",
                embedding.len(),
                expected
            ),
            hint: Some("Reindex the workspace with the active embedding model.".into()),
        })
    }

    fn pdf_text<B: PdfBackend>(
        &self,
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
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use tempfile::tempdir;

    use super::*;
    use zot_core::{Attachment, Creator, Item, Workspace, WorkspaceItem};

    struct FakeLibrary {
        items: Vec<Item>,
        attachment: Option<Attachment>,
        pdf_path: PathBuf,
    }

    impl WorkspaceRagLibrary for FakeLibrary {
        fn get_item(&self, key: &str) -> ZotResult<Option<Item>> {
            Ok(self.items.iter().find(|item| item.key == key).cloned())
        }
        fn get_pdf_attachment(&self, _key: &str) -> ZotResult<Option<zot_core::Attachment>> {
            Ok(self.attachment.clone())
        }
        fn pdf_path(&self, _attachment: &zot_core::Attachment) -> PathBuf {
            self.pdf_path.clone()
        }
    }

    struct FakeBackend {
        extracted: Cell<usize>,
        text: String,
    }

    impl FakeBackend {
        fn new(text: impl Into<String>) -> Self {
            Self {
                extracted: Cell::new(0),
                text: text.into(),
            }
        }
    }

    impl PdfBackend for FakeBackend {
        fn availability_hint(&self) -> ZotResult<()> {
            Ok(())
        }
        fn extract_text(
            &self,
            _pdf_path: &Path,
            _page_range: Option<(usize, usize)>,
        ) -> ZotResult<String> {
            self.extracted.set(self.extracted.get() + 1);
            Ok(self.text.clone())
        }
        fn extract_annotations(
            &self,
            _pdf_path: &Path,
        ) -> ZotResult<Vec<zot_core::AnnotationSnippet>> {
            Ok(Vec::new())
        }
        fn extract_outline(&self, _pdf_path: &Path) -> ZotResult<Vec<zot_core::PdfOutlineEntry>> {
            Ok(Vec::new())
        }
        fn find_text_position(
            &self,
            _pdf_path: &Path,
            _page: usize,
            _text: &str,
        ) -> ZotResult<Option<crate::pdf::PdfMatchPosition>> {
            Ok(None)
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
            Err(ZotError::InvalidInput {
                code: "not-used".into(),
                message: "not used".into(),
                hint: None,
            })
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

    fn workspace_with(item: &Item) -> Workspace {
        Workspace {
            name: "demo".into(),
            created: "2026-01-01T00:00:00Z".into(),
            description: "".into(),
            items: vec![WorkspaceItem {
                key: item.key.clone(),
                title: item.title.clone(),
                added: "now".into(),
            }],
        }
    }

    #[test]
    fn metadata_only_workspace_is_queryable() {
        let dir = tempdir().unwrap();
        let store = crate::workspace::WorkspaceStore::new(Some(dir.path().to_path_buf()));
        let rag = WorkspaceRagStore::open(&store, "demo").unwrap();
        let item = sample_item(
            "ATTN001",
            "Attention Is All You Need",
            "Transformer architecture",
        );
        let workspace = workspace_with(&item);
        let lib = FakeLibrary {
            items: vec![item],
            attachment: None,
            pdf_path: PathBuf::new(),
        };
        let (_stats, pending) = rag
            .reindex_workspace(&lib, &workspace, &FakeBackend::new(""), false)
            .unwrap();
        assert_eq!(pending.len(), 1);
        let hits = rag
            .query_workspace(&workspace, "attention", HybridMode::Bm25, None, 10)
            .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].item_key, "ATTN001");
    }

    #[test]
    fn query_workspace_filters_stale_chunks_by_current_membership() {
        let dir = tempdir().unwrap();
        let store = crate::workspace::WorkspaceStore::new(Some(dir.path().to_path_buf()));
        let rag = WorkspaceRagStore::open(&store, "demo").unwrap();
        let item = sample_item(
            "ATTN001",
            "Attention Is All You Need",
            "Transformer architecture",
        );
        let indexed_workspace = workspace_with(&item);
        let lib = FakeLibrary {
            items: vec![item],
            attachment: None,
            pdf_path: PathBuf::new(),
        };
        rag.reindex_workspace(&lib, &indexed_workspace, &FakeBackend::new(""), false)
            .unwrap();

        let current_workspace = Workspace {
            name: "demo".into(),
            created: "2026-01-01T00:00:00Z".into(),
            description: "".into(),
            items: Vec::new(),
        };

        let hits = rag
            .query_workspace(&current_workspace, "attention", HybridMode::Bm25, None, 10)
            .unwrap();
        assert!(hits.is_empty());
    }

    #[test]
    fn query_workspace_filters_before_limit() {
        let dir = tempdir().unwrap();
        let store = crate::workspace::WorkspaceStore::new(Some(dir.path().to_path_buf()));
        let rag = WorkspaceRagStore::open(&store, "demo").unwrap();
        let target = sample_item("KEEP001", "Kept Paper", "needle");
        let mut items = vec![target.clone()];
        for index in 0..8 {
            items.push(sample_item(
                &format!("DROP{index:03}"),
                &format!("Removed Paper {index}"),
                &"needle ".repeat(20),
            ));
        }
        let indexed_workspace = Workspace {
            name: "demo".into(),
            created: "2026-01-01T00:00:00Z".into(),
            description: "".into(),
            items: items
                .iter()
                .map(|item| WorkspaceItem {
                    key: item.key.clone(),
                    title: item.title.clone(),
                    added: "now".into(),
                })
                .collect(),
        };
        let current_workspace = workspace_with(&target);
        let lib = FakeLibrary {
            items,
            attachment: None,
            pdf_path: PathBuf::new(),
        };

        rag.reindex_workspace(&lib, &indexed_workspace, &FakeBackend::new(""), false)
            .unwrap();

        let hits = rag
            .query_workspace(&current_workspace, "needle", HybridMode::Bm25, None, 1)
            .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].item_key, "KEEP001");
    }

    #[test]
    fn reindex_skips_missing_workspace_items() {
        let dir = tempdir().unwrap();
        let store = crate::workspace::WorkspaceStore::new(Some(dir.path().to_path_buf()));
        let rag = WorkspaceRagStore::open(&store, "demo").unwrap();
        let missing_item = sample_item("MISS001", "Missing", "Missing abstract");
        let workspace = workspace_with(&missing_item);
        let lib = FakeLibrary {
            items: Vec::new(),
            attachment: None,
            pdf_path: PathBuf::new(),
        };

        let (stats, pending) = rag
            .reindex_workspace(&lib, &workspace, &FakeBackend::new(""), false)
            .unwrap();

        assert_eq!(stats.items, 1);
        assert_eq!(stats.chunks, 0);
        assert!(pending.is_empty());
        assert!(
            rag.query("missing", HybridMode::Bm25, None, 10)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn fulltext_reindex_reuses_pdf_cache() {
        let dir = tempdir().unwrap();
        let store = crate::workspace::WorkspaceStore::new(Some(dir.path().to_path_buf()));
        let rag = WorkspaceRagStore::open(&store, "demo").unwrap();
        let pdf_path = dir.path().join("paper.pdf");
        std::fs::write(&pdf_path, b"pdf").unwrap();
        let item = sample_item(
            "ATTN001",
            "Attention Is All You Need",
            "Transformer architecture",
        );
        let workspace = workspace_with(&item);
        let lib = FakeLibrary {
            items: vec![item],
            attachment: Some(Attachment {
                key: "PDF001".into(),
                parent_key: "ATTN001".into(),
                filename: "paper.pdf".into(),
                content_type: "application/pdf".into(),
            }),
            pdf_path,
        };
        let backend = FakeBackend::new("cached pdf attention content");

        rag.reindex_workspace(&lib, &workspace, &backend, true)
            .unwrap();
        rag.reindex_workspace(&lib, &workspace, &backend, true)
            .unwrap();

        assert_eq!(backend.extracted.get(), 1);
        let hits = rag.query("cached", HybridMode::Bm25, None, 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].source, "pdf");
    }

    #[test]
    fn query_embedding_dimension_mismatch_errors() {
        let dir = tempdir().unwrap();
        let store = crate::workspace::WorkspaceStore::new(Some(dir.path().to_path_buf()));
        let rag = WorkspaceRagStore::open(&store, "demo").unwrap();
        rag.apply_pending_embeddings(
            vec![PendingEmbedding {
                chunk_id: 1,
                text: "x".into(),
            }],
            vec![vec![1.0, 0.0, 0.0]],
        )
        .unwrap();

        let err = rag
            .query("attention", HybridMode::Semantic, Some(&[1.0, 0.0]), 10)
            .unwrap_err();
        assert!(matches!(err, ZotError::InvalidInput { .. }));
    }

    #[test]
    fn embedding_count_mismatch_errors() {
        let dir = tempdir().unwrap();
        let store = crate::workspace::WorkspaceStore::new(Some(dir.path().to_path_buf()));
        let rag = WorkspaceRagStore::open(&store, "demo").unwrap();
        let err = rag
            .apply_pending_embeddings(
                vec![PendingEmbedding {
                    chunk_id: 1,
                    text: "x".into(),
                }],
                vec![],
            )
            .unwrap_err();
        assert!(matches!(err, ZotError::InvalidInput { .. }));
    }

    #[test]
    fn workspace_paths_follow_store_root() {
        let dir = tempdir().unwrap();
        let store = crate::workspace::WorkspaceStore::new(Some(dir.path().to_path_buf()));
        let rag = WorkspaceRagStore::open(&store, "demo").unwrap();
        assert_eq!(rag.index_path(), &dir.path().join("demo.idx.sqlite"));
        assert_eq!(rag.cache_path(), &dir.path().join(".md_cache.sqlite"));
    }
}
