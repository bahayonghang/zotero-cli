pub mod config;
pub mod envelope;
pub mod error;
pub mod model;

pub use config::{
    AppConfig, EmbeddingConfig, LibraryScope, detect_zotero_data_dir, get_data_dir,
    parse_library_scope, redact_secret,
};
pub use envelope::{CliEnvelope, EnvelopeMeta};
pub use error::{ErrorPayload, ZotError, ZotResult};
pub use model::*;
