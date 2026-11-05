pub mod config;
pub mod envelope;
pub mod error;
pub mod model;
pub mod net;
pub mod search_match;

pub use config::{
    AppConfig, EmbeddingConfig, LibraryScope, SecretString, canonicalize_or_original,
    detect_zotero_data_dir, ensure_config_dir, get_data_dir, parse_library_scope, redact_secret,
};
pub use envelope::{CliEnvelope, EnvelopeMeta};
pub use error::{ErrorPayload, ZotError, ZotResult};
pub use model::*;
pub use search_match::{
    SearchItemDirection, SearchItemSort, collapse_field_text, collapsed_contains,
    extra_line_has_citation_key, first_creator_name, item_matches_creator_filter,
    item_matches_field_filters, item_matches_metadata_query, item_matches_tag_filter,
    item_matches_year_filter, sort_search_items,
};
