use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use zot_core::{AppConfig, LibraryScope};
use zot_local::LocalLibrary;
use zot_remote::{HttpRuntime, ZoteroRemote};

use crate::cli::Cli;

#[derive(Debug, Clone)]
pub(crate) struct AppContext {
    pub(crate) json: bool,
    pub(crate) profile: Option<String>,
    pub(crate) scope: LibraryScope,
    pub(crate) config: AppConfig,
    pub(crate) http: Arc<HttpRuntime>,
}

impl AppContext {
    pub(crate) fn from_cli(cli: &Cli) -> Result<Self> {
        let scope = zot_core::parse_library_scope(&cli.library)?;
        let config = AppConfig::load(cli.profile.as_deref())?;
        let http = Arc::new(HttpRuntime::new()?);
        Ok(Self {
            json: cli.json,
            profile: cli.profile.clone(),
            scope,
            config,
            http,
        })
    }

    pub(crate) fn http(&self) -> &HttpRuntime {
        &self.http
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

    pub(crate) fn library_index_path(&self) -> PathBuf {
        let scope = match &self.scope {
            LibraryScope::User => "user".to_string(),
            LibraryScope::Group { group_id } => format!("group-{group_id}"),
        };
        AppConfig::config_dir()
            .join("indexes")
            .join(format!("{scope}.idx.sqlite"))
    }
}
