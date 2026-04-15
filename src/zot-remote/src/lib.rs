pub mod embedding;
pub mod semantic_scholar;
pub mod zotero;

pub use embedding::EmbeddingClient;
pub use semantic_scholar::{
    PreprintInfo, PublicationStatus, SemanticScholarClient, extract_preprint_info,
};
pub use zotero::ZoteroRemote;
