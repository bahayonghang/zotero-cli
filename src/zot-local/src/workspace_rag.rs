use std::collections::HashSet;
use std::path::{Path, PathBuf};

use zot_core::{QueryChunk, Workspace, ZotResult};

use crate::pdf::{PdfBackend, PdfCache};
use crate::rag_engine::{self, PendingEmbedding, RagLibrary, ReindexStats};
use crate::workspace::{HybridMode, RagIndex, WorkspaceName};

/// Options controlling [`WorkspaceRagStore::reindex_workspace`].
///
/// `force_rebuild = false` (the default) only re-embeds workspace items that
/// are not already indexed; existing chunks are kept verbatim and items that
/// have been removed from the workspace are pruned. `force_rebuild = true`
/// clears the entire index first and reprocesses every workspace item.
#[derive(Debug, Clone, Copy, Default)]
pub struct WorkspaceReindexOpts {
    pub fulltext: bool,
    pub force_rebuild: bool,
}

pub struct WorkspaceRagStore {
    index: RagIndex,
    index_path: PathBuf,
    cache_path: PathBuf,
}

impl WorkspaceRagStore {
    pub fn open(
        store: &crate::workspace::WorkspaceStore,
        workspace_name: &WorkspaceName,
    ) -> ZotResult<Self> {
        let index_path = store.rag_index_path(workspace_name)?;
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

    pub fn reindex_workspace<L: RagLibrary, B: PdfBackend>(
        &self,
        library: &L,
        workspace: &Workspace,
        backend: &B,
        opts: WorkspaceReindexOpts,
    ) -> ZotResult<(ReindexStats, Vec<PendingEmbedding>)> {
        let WorkspaceReindexOpts {
            fulltext,
            force_rebuild,
        } = opts;
        let md_cache = if fulltext {
            Some(PdfCache::new(Some(self.cache_path.clone()))?)
        } else {
            None
        };
        let workspace_keys: HashSet<&str> = workspace
            .items
            .iter()
            .map(|entry| entry.key.as_str())
            .collect();
        let requested: Vec<&str> = workspace.items.iter().map(|e| e.key.as_str()).collect();
        rag_engine::reindex(
            &self.index,
            library,
            backend,
            md_cache.as_ref(),
            &requested,
            rag_engine::RefreshPolicy::SkipIndexed,
            |key| Ok(!workspace_keys.contains(key)),
            fulltext,
            force_rebuild,
        )
    }

    pub fn apply_pending_embeddings(
        &self,
        pending: Vec<PendingEmbedding>,
        embeddings: Vec<Vec<f32>>,
    ) -> ZotResult<()> {
        rag_engine::apply_pending_embeddings(&self.index, pending, embeddings)
    }

    pub fn query(
        &self,
        question: &str,
        mode: HybridMode,
        embedding: Option<&[f32]>,
        limit: usize,
    ) -> ZotResult<Vec<QueryChunk>> {
        rag_engine::validate_query_embedding(&self.index, mode, embedding)?;
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
        rag_engine::validate_query_embedding(&self.index, mode, embedding)?;
        self.index
            .query_allowed(question, mode, embedding, &allowed_keys, limit)
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use tempfile::tempdir;

    use super::*;
    use zot_core::{Attachment, Creator, Item, Workspace, WorkspaceItem, ZotError};

    fn demo_name() -> WorkspaceName {
        WorkspaceName::parse("demo").expect("valid workspace name")
    }

    struct FakeLibrary {
        items: Vec<Item>,
        attachment: Option<Attachment>,
        pdf_path: PathBuf,
    }

    impl RagLibrary for FakeLibrary {
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
            _occurrence: usize,
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
        let rag = WorkspaceRagStore::open(&store, &demo_name()).unwrap();
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
            .reindex_workspace(
                &lib,
                &workspace,
                &FakeBackend::new(""),
                WorkspaceReindexOpts::default(),
            )
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
        let rag = WorkspaceRagStore::open(&store, &demo_name()).unwrap();
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
        rag.reindex_workspace(
            &lib,
            &indexed_workspace,
            &FakeBackend::new(""),
            WorkspaceReindexOpts::default(),
        )
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
        let rag = WorkspaceRagStore::open(&store, &demo_name()).unwrap();
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

        rag.reindex_workspace(
            &lib,
            &indexed_workspace,
            &FakeBackend::new(""),
            WorkspaceReindexOpts::default(),
        )
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
        let rag = WorkspaceRagStore::open(&store, &demo_name()).unwrap();
        let missing_item = sample_item("MISS001", "Missing", "Missing abstract");
        let workspace = workspace_with(&missing_item);
        let lib = FakeLibrary {
            items: Vec::new(),
            attachment: None,
            pdf_path: PathBuf::new(),
        };

        let (stats, pending) = rag
            .reindex_workspace(
                &lib,
                &workspace,
                &FakeBackend::new(""),
                WorkspaceReindexOpts::default(),
            )
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
        let rag = WorkspaceRagStore::open(&store, &demo_name()).unwrap();
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

        rag.reindex_workspace(
            &lib,
            &workspace,
            &backend,
            WorkspaceReindexOpts {
                fulltext: true,
                force_rebuild: true,
            },
        )
        .unwrap();
        rag.reindex_workspace(
            &lib,
            &workspace,
            &backend,
            WorkspaceReindexOpts {
                fulltext: true,
                force_rebuild: true,
            },
        )
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
        let rag = WorkspaceRagStore::open(&store, &demo_name()).unwrap();
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
        let rag = WorkspaceRagStore::open(&store, &demo_name()).unwrap();
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
    fn incremental_reindex_does_not_reembed_unchanged_items() {
        let dir = tempdir().unwrap();
        let store = crate::workspace::WorkspaceStore::new(Some(dir.path().to_path_buf()));
        let rag = WorkspaceRagStore::open(&store, &demo_name()).unwrap();
        let item = sample_item("INC001", "Incremental Paper", "abstract content");
        let workspace = workspace_with(&item);
        let lib = FakeLibrary {
            items: vec![item],
            attachment: None,
            pdf_path: PathBuf::new(),
        };
        let (_, first_pending) = rag
            .reindex_workspace(
                &lib,
                &workspace,
                &FakeBackend::new(""),
                WorkspaceReindexOpts::default(),
            )
            .unwrap();
        assert_eq!(first_pending.len(), 1);
        let (_, second_pending) = rag
            .reindex_workspace(
                &lib,
                &workspace,
                &FakeBackend::new(""),
                WorkspaceReindexOpts::default(),
            )
            .unwrap();
        assert!(
            second_pending.is_empty(),
            "unchanged workspace must not produce pending embeddings on a second reindex"
        );
    }

    #[test]
    fn force_rebuild_clears_and_re_embeds() {
        let dir = tempdir().unwrap();
        let store = crate::workspace::WorkspaceStore::new(Some(dir.path().to_path_buf()));
        let rag = WorkspaceRagStore::open(&store, &demo_name()).unwrap();
        let item = sample_item("REB001", "Rebuild Paper", "abstract content");
        let workspace = workspace_with(&item);
        let lib = FakeLibrary {
            items: vec![item],
            attachment: None,
            pdf_path: PathBuf::new(),
        };
        rag.reindex_workspace(
            &lib,
            &workspace,
            &FakeBackend::new(""),
            WorkspaceReindexOpts::default(),
        )
        .unwrap();
        let (_, rebuild_pending) = rag
            .reindex_workspace(
                &lib,
                &workspace,
                &FakeBackend::new(""),
                WorkspaceReindexOpts {
                    fulltext: false,
                    force_rebuild: true,
                },
            )
            .unwrap();
        assert_eq!(
            rebuild_pending.len(),
            1,
            "force_rebuild must clear and re-embed every workspace item"
        );
    }

    #[test]
    fn workspace_paths_follow_store_root() {
        let dir = tempdir().unwrap();
        let store = crate::workspace::WorkspaceStore::new(Some(dir.path().to_path_buf()));
        let rag = WorkspaceRagStore::open(&store, &demo_name()).unwrap();
        assert_eq!(rag.index_path(), &dir.path().join("demo.idx.sqlite"));
        assert_eq!(rag.cache_path(), &dir.path().join(".md_cache.sqlite"));
    }

    #[cfg(unix)]
    #[test]
    fn open_rejects_index_symlink_outside_root() {
        use std::os::unix::fs::symlink;

        let root = tempdir().expect("workspace root");
        let outside = tempdir().expect("outside root");
        let outside_index = outside.path().join("outside.idx.sqlite");
        std::fs::write(&outside_index, []).expect("create outside index");
        symlink(&outside_index, root.path().join("demo.idx.sqlite")).expect("create symlink");
        let store = crate::workspace::WorkspaceStore::new(Some(root.path().to_path_buf()));

        let err = WorkspaceRagStore::open(&store, &demo_name())
            .err()
            .expect("external symlink must fail");
        assert_eq!(err.payload().code, "workspace-path-boundary");
    }
}
