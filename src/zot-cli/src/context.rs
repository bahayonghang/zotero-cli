use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use zot_core::{AppConfig, LibraryScope, WriteBackend};
use zot_desktop::DesktopClient;
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
        let scope = zot_core::parse_library_scope(&cli.library)?;
        let mut config = AppConfig::load(cli.profile.as_deref())?;
        if let Some(write_backend) = cli.write_backend {
            config.zotero.write_backend = write_backend.into();
        }
        let http = Arc::new(HttpRuntime::new()?);
        Ok(Self {
            json: cli.json,
            profile: cli.profile.clone(),
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
            &self.config.zotero.api_key,
            self.scope.clone(),
        )
    }

    pub(crate) fn desktop(&self) -> zot_core::ZotResult<DesktopClient> {
        DesktopClient::new()
    }

    pub(crate) fn write_backend(&self) -> WriteBackend {
        self.config.zotero.write_backend
    }

    pub(crate) fn library_index_path(&self) -> PathBuf {
        let scope = match &self.scope {
            LibraryScope::User => "user".to_string(),
            LibraryScope::Group { group_id } => format!("group-{group_id}"),
        };
        AppConfig::config_dir()
            .join("indexes")
            .join(format!("{scope}.idx.sqlite"))
    }

    /// Markdown cache for library-wide PDF text extraction, colocated here
    /// with [`Self::library_index_path`] so sidecar paths have one owner.
    pub(crate) fn library_md_cache_path(&self) -> PathBuf {
        AppConfig::config_dir()
            .join("cache")
            .join("library_md_cache.sqlite")
    }
}
