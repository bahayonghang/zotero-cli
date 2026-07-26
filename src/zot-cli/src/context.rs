use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use zot_core::{AppConfig, LibraryScope};
use zot_desktop::ConnectorClient;
use zot_local::{LocalLibrary, PdfBackend, PdfiumBackend};
use zot_remote::{HttpRuntime, ZoteroRemote};

use crate::cli::Cli;

#[derive(Clone)]
pub(crate) struct AppContext {
    pub(crate) json: bool,
    pub(crate) profile: Option<String>,
    pub(crate) scope: LibraryScope,
    pub(crate) config: AppConfig,
    pub(crate) http: Arc<HttpRuntime>,
    /// PDF engine for command-side extraction; tests inject fakes here.
    /// `doctor` keeps its own concrete `PdfiumBackend` because it reports
    /// Pdfium-specific diagnostics (`status()` is not on the trait).
    pub(crate) pdf: Arc<dyn PdfBackend + Send + Sync>,
}

impl fmt::Debug for AppContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AppContext")
            .field("json", &self.json)
            .field("profile", &self.profile)
            .field("scope", &self.scope)
            .field("config", &self.config)
            .field("http", &self.http)
            .finish_non_exhaustive()
    }
}

impl AppContext {
    pub(crate) fn from_cli(cli: &Cli) -> Result<Self> {
        Self::from_cli_with_config(cli, AppConfig::load_raw()?)
    }

    fn from_cli_with_config(cli: &Cli, raw: AppConfig) -> Result<Self> {
        let scope = zot_core::parse_library_scope(&cli.library)?;
        let (config, profile) = raw.into_effective(cli.profile.as_deref());
        let json = cli.json || config.output.default_format == "json";
        let http = Arc::new(HttpRuntime::new()?);
        Ok(Self {
            json,
            profile,
            scope,
            config,
            http,
            pdf: Arc::new(PdfiumBackend),
        })
    }

    pub(crate) fn http(&self) -> &HttpRuntime {
        &self.http
    }

    pub(crate) fn pdf_backend(&self) -> Arc<dyn PdfBackend + Send + Sync> {
        Arc::clone(&self.pdf)
    }

    pub(crate) fn local_library(&self) -> zot_core::ZotResult<LocalLibrary> {
        LocalLibrary::open(zot_core::get_data_dir(&self.config), self.scope.clone())
    }

    pub(crate) fn remote(&self) -> zot_core::ZotResult<ZoteroRemote> {
        let library_id = self
            .scope
            .public_id(Some(&self.config.zotero.library_id))
            .ok_or_else(|| zot_core::ZotError::InvalidInput {
                code: "write-credentials".to_string(),
                message: "Missing configured library_id for remote writes".to_string(),
                hint: Some("Run `zot config init` or set ZOT_LIBRARY_ID".to_string()),
            })?;
        if self.config.zotero.api_key.is_empty() {
            return Err(zot_core::ZotError::InvalidInput {
                code: "write-credentials".to_string(),
                message: "Missing Zotero API key".to_string(),
                hint: Some("Run `zot config init` or set ZOT_API_KEY".to_string()),
            });
        }
        ZoteroRemote::new(
            &self.http,
            library_id,
            self.config.zotero.api_key.expose_secret(),
            self.scope.clone(),
        )
    }

    pub(crate) fn connector(&self) -> zot_core::ZotResult<ConnectorClient> {
        ConnectorClient::new()
    }

    pub(crate) fn library_index_path(&self) -> PathBuf {
        let scope = match &self.scope {
            LibraryScope::User => "user".to_string(),
            LibraryScope::Group { group_id } => format!("group-{group_id}"),
        };
        AppConfig::state_dir()
            .join("indexes")
            .join(format!("{scope}.idx.sqlite"))
    }

    /// Markdown cache for library-wide PDF text extraction, colocated here
    /// with [`Self::library_index_path`] so sidecar paths have one owner.
    pub(crate) fn library_md_cache_path(&self) -> PathBuf {
        AppConfig::state_dir()
            .join("cache")
            .join("library_md_cache.sqlite")
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;
    use zot_core::config::{OutputConfig, ProfileConfig};

    use super::*;

    #[test]
    fn default_profile_controls_context_metadata_and_output_mode() {
        let cli = Cli::try_parse_from(["zot", "doctor"]).expect("parse cli");
        let mut raw = AppConfig::default();
        raw.profile.insert(
            "work".to_string(),
            ProfileConfig {
                output: OutputConfig {
                    default_format: "json".to_string(),
                    limit: 17,
                },
                ..ProfileConfig::default()
            },
        );
        raw.set_default_profile(Some("work"));

        let context = AppContext::from_cli_with_config(&cli, raw).expect("build context");

        assert!(context.json);
        assert_eq!(context.profile.as_deref(), Some("work"));
        assert_eq!(context.config.output.limit, 17);
    }

    #[test]
    fn context_debug_never_exposes_secret_canary() {
        let cli = Cli::try_parse_from(["zot", "doctor"]).expect("parse cli");
        let mut raw = AppConfig::default();
        raw.zotero.api_key = "context-secret-canary".into();

        let context = AppContext::from_cli_with_config(&cli, raw).expect("build context");

        assert!(!format!("{context:?}").contains("context-secret-canary"));
    }
}
