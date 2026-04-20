use std::env;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::error::{ZotError, ZotResult};

pub const CONFIG_DIR_NAME: &str = "zot";
pub const CONFIG_FILE_NAME: &str = "config.toml";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum LibraryScope {
    User,
    Group { group_id: i64 },
}

impl LibraryScope {
    pub fn library_type(&self) -> &'static str {
        match self {
            LibraryScope::User => "user",
            LibraryScope::Group { .. } => "group",
        }
    }

    pub fn public_id(&self, configured: Option<&str>) -> Option<String> {
        match self {
            LibraryScope::User => configured.map(ToOwned::to_owned),
            LibraryScope::Group { group_id } => Some(group_id.to_string()),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OutputConfig {
    #[serde(default = "default_format")]
    pub default_format: String,
    #[serde(default = "default_limit")]
    pub limit: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExportConfig {
    #[serde(default = "default_export_style")]
    pub default_style: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingConfig {
    #[serde(default = "default_embedding_url")]
    pub url: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default = "default_embedding_model")]
    pub model: String,
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            url: default_embedding_url(),
            api_key: String::new(),
            model: default_embedding_model(),
        }
    }
}

impl EmbeddingConfig {
    pub fn is_configured(&self) -> bool {
        !self.url.is_empty() && !self.api_key.is_empty()
    }

    pub fn apply_env_overrides(&mut self) {
        if let Ok(value) = env::var("ZOT_EMBEDDING_URL") {
            self.url = value;
        }
        if let Ok(value) = env::var("ZOT_EMBEDDING_KEY") {
            self.api_key = value;
        }
        if let Ok(value) = env::var("ZOT_EMBEDDING_MODEL") {
            self.model = value;
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ZoteroConfig {
    #[serde(default)]
    pub data_dir: String,
    #[serde(default)]
    pub library_id: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub semantic_scholar_api_key: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProfileConfig {
    #[serde(default)]
    pub data_dir: String,
    #[serde(default)]
    pub library_id: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub semantic_scholar_api_key: String,
    #[serde(default)]
    pub output: OutputConfig,
    #[serde(default)]
    pub export: ExportConfig,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub zotero: ZoteroConfig,
    #[serde(default)]
    pub output: OutputConfig,
    #[serde(default)]
    pub export: ExportConfig,
    #[serde(default)]
    pub embedding: EmbeddingConfig,
    #[serde(default)]
    pub profile: std::collections::BTreeMap<String, ProfileConfig>,
    #[serde(default)]
    pub default: std::collections::BTreeMap<String, String>,
}

impl AppConfig {
    pub fn config_dir() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(CONFIG_DIR_NAME)
    }

    pub fn config_file() -> PathBuf {
        Self::config_dir().join(CONFIG_FILE_NAME)
    }

    pub fn load_raw() -> ZotResult<Self> {
        let path = Self::config_file();
        if !path.exists() {
            return Ok(Self::default());
        }
        let raw = std::fs::read_to_string(&path).map_err(|source| ZotError::Io {
            path: path.clone(),
            source,
        })?;
        let parsed: Self = toml::from_str(&raw).map_err(|source| ZotError::ConfigParse {
            path: path.clone(),
            detail: source.to_string(),
        })?;
        Ok(parsed)
    }

    pub fn load(profile: Option<&str>) -> ZotResult<Self> {
        Ok(Self::load_raw()?.materialize_profile(profile))
    }

    pub fn save(&self) -> ZotResult<PathBuf> {
        let path = Self::config_file();
        ensure_config_dir()?;
        let encoded = toml::to_string_pretty(self).map_err(|source| ZotError::ConfigParse {
            path: path.clone(),
            detail: source.to_string(),
        })?;
        std::fs::write(&path, encoded).map_err(|source| ZotError::Io {
            path: path.clone(),
            source,
        })?;
        Ok(path)
    }

    fn materialize_profile(mut self, profile_name: Option<&str>) -> Self {
        let selected = profile_name
            .map(ToOwned::to_owned)
            .or_else(|| self.default.get("profile").cloned());

        if let Some(name) = selected
            && let Some(profile) = self.profile.get(&name)
        {
            self.zotero.data_dir = profile.data_dir.clone();
            self.zotero.library_id = profile.library_id.clone();
            self.zotero.api_key = profile.api_key.clone();
            self.zotero.semantic_scholar_api_key = profile.semantic_scholar_api_key.clone();
            self.output = profile.output.clone();
            self.export = profile.export.clone();
        }

        self.apply_env_overrides();
        self.embedding.apply_env_overrides();
        self
    }

    pub fn apply_env_overrides(&mut self) {
        if let Ok(value) = env::var("ZOT_DATA_DIR") {
            self.zotero.data_dir = value;
        }
        if let Ok(value) = env::var("ZOT_LIBRARY_ID") {
            self.zotero.library_id = value;
        }
        if let Ok(value) = env::var("ZOT_API_KEY") {
            self.zotero.api_key = value;
        }
        if let Ok(value) = env::var("SEMANTIC_SCHOLAR_API_KEY") {
            self.zotero.semantic_scholar_api_key = value;
        }
        if let Ok(value) = env::var("S2_API_KEY") {
            self.zotero.semantic_scholar_api_key = value;
        }
    }

    pub fn write_credentials_configured(&self) -> bool {
        !self.zotero.library_id.is_empty() && !self.zotero.api_key.is_empty()
    }

    pub fn semantic_scholar_key(&self) -> Option<&str> {
        (!self.zotero.semantic_scholar_api_key.is_empty())
            .then_some(self.zotero.semantic_scholar_api_key.as_str())
    }

    pub fn default_profile_name(&self) -> Option<&str> {
        self.default.get("profile").map(String::as_str)
    }

    pub fn set_default_profile(&mut self, profile_name: Option<&str>) {
        if let Some(profile_name) = profile_name {
            self.default
                .insert("profile".to_string(), profile_name.to_string());
        } else {
            self.default.remove("profile");
        }
    }
}

pub fn parse_library_scope(value: &str) -> ZotResult<LibraryScope> {
    if value == "user" {
        return Ok(LibraryScope::User);
    }

    static GROUP_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^group:(\d+)$").expect("valid regex"));
    if let Some(captures) = GROUP_RE.captures(value) {
        let group_id = captures
            .get(1)
            .and_then(|m| m.as_str().parse::<i64>().ok())
            .ok_or_else(|| ZotError::InvalidInput {
                code: "invalid-library".to_string(),
                message: format!("Invalid library scope: {value}"),
                hint: Some("Use 'user' or 'group:<id>'".to_string()),
            })?;
        return Ok(LibraryScope::Group { group_id });
    }

    Err(ZotError::InvalidInput {
        code: "invalid-library".to_string(),
        message: format!("Invalid library scope: {value}"),
        hint: Some("Use 'user' or 'group:<id>'".to_string()),
    })
}

pub fn detect_zotero_data_dir(config: &AppConfig) -> PathBuf {
    if !config.zotero.data_dir.is_empty() {
        return PathBuf::from(&config.zotero.data_dir);
    }

    #[cfg(target_os = "windows")]
    {
        if let Some(path) = windows_registry_data_dir() {
            return path;
        }

        if let Ok(app_data) = env::var("APPDATA") {
            let candidate = PathBuf::from(app_data).join("Zotero");
            if candidate.exists() {
                return candidate;
            }
        }
        if let Ok(local_app_data) = env::var("LOCALAPPDATA") {
            let candidate = PathBuf::from(local_app_data).join("Zotero");
            if candidate.exists() {
                return candidate;
            }
        }
    }

    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Zotero")
}

pub fn get_data_dir(config: &AppConfig) -> PathBuf {
    if let Ok(value) = env::var("ZOT_DATA_DIR") {
        return PathBuf::from(value);
    }
    detect_zotero_data_dir(config)
}

#[cfg(target_os = "windows")]
fn windows_registry_data_dir() -> Option<PathBuf> {
    use winreg::RegKey;
    use winreg::enums::HKEY_CURRENT_USER;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let key = hkcu.open_subkey("Software\\Zotero\\Zotero").ok()?;
    let path: String = key.get_value("dataDir").ok()?;
    let candidate = PathBuf::from(path);
    candidate.exists().then_some(candidate)
}

pub fn redact_secret(value: &str) -> String {
    if value.len() <= 4 {
        return "(set)".to_string();
    }
    format!("***{}", &value[value.len() - 4..])
}

fn default_format() -> String {
    "table".to_string()
}

fn default_limit() -> usize {
    50
}

fn default_export_style() -> String {
    "bibtex".to_string()
}

fn default_embedding_url() -> String {
    "https://api.jina.ai/v1/embeddings".to_string()
}

fn default_embedding_model() -> String {
    "jina-embeddings-v3".to_string()
}

pub fn ensure_config_dir() -> ZotResult<PathBuf> {
    let dir = AppConfig::config_dir();
    std::fs::create_dir_all(&dir).map_err(|source| ZotError::Io {
        path: dir.clone(),
        source,
    })?;
    Ok(dir)
}

pub fn canonicalize_or_original(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_library_scope() {
        assert_eq!(parse_library_scope("user").unwrap(), LibraryScope::User);
        assert_eq!(
            parse_library_scope("group:42").unwrap(),
            LibraryScope::Group { group_id: 42 }
        );
        assert!(parse_library_scope("group:abc").is_err());
    }

    #[test]
    fn manages_default_profile_name() {
        let mut config = AppConfig::default();
        assert_eq!(config.default_profile_name(), None);

        config.set_default_profile(Some("work"));
        assert_eq!(config.default_profile_name(), Some("work"));

        config.set_default_profile(None);
        assert_eq!(config.default_profile_name(), None);
    }
}
