pub mod better_bibtex;
pub mod embedding;
pub mod oa;
pub mod scite;
pub mod semantic_scholar;
pub mod zotero;

pub use better_bibtex::{BetterBibTexClient, BetterBibTexSearchItem};
pub use embedding::EmbeddingClient;
pub use oa::{
    ArxivWork, CrossRefWork, OaClient, ResolvedPdfUrl, normalize_arxiv_id, normalize_doi,
};
pub use scite::SciteClient;
pub use semantic_scholar::{
    PreprintInfo, PublicationStatus, SemanticScholarClient, extract_preprint_info,
};
pub use zotero::ZoteroRemote;
