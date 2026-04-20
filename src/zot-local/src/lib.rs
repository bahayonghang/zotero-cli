pub mod citation;
pub mod db;
pub mod pdf;
pub mod semantic;
pub mod workspace;

pub use citation::{CitationStyle, export_item, format_citation};
pub use db::{DuplicateMatchMethod, LocalLibrary, SearchOptions, SortDirection, SortField};
pub use pdf::{PdfAreaPosition, PdfBackend, PdfCache, PdfMatchPosition, PdfiumBackend};
pub use semantic::{PendingEmbedding, ReindexOpts, ReindexStats, SemanticStore};
pub use workspace::{
    HybridMode, RagIndex, WorkspaceStore, build_metadata_chunk, chunk_text,
    compute_term_frequencies, tokenize,
};
