pub mod citation;
pub mod db;
pub mod graph;
pub mod library_traits;
pub mod pdf;
mod rag_engine;
pub mod semantic;
pub mod workspace;
pub mod workspace_rag;

pub use citation::{CitationStyle, export_item, format_citation};
pub use db::{DuplicateMatchMethod, LocalLibrary, SearchOptions, SortDirection, SortField};
pub use library_traits::{
    AttachmentSource, CollectionContent, CollectionNav, ItemReader, NoteReader,
};
pub use pdf::{
    PdfAreaPosition, PdfBackend, PdfCache, PdfMatchPosition, PdfiumAvailability, PdfiumBackend,
};
pub use rag_engine::{PendingEmbedding, RagLibrary, ReindexStats};
pub use semantic::{ReindexOpts, SemanticStore};
pub use workspace::{
    HybridMode, RagIndex, WorkspaceStore, build_metadata_chunk, chunk_text,
    compute_term_frequencies, tokenize,
};
pub use workspace_rag::{WorkspaceRagStore, WorkspaceReindexOpts};
