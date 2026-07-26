use std::env;
use std::fmt;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;

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

#[derive(Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(transparent)]
pub struct SecretString(String);

impl SecretString {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn expose_secret(&self) -> &str {
        &self.0
    }

    pub fn set(&mut self, value: String) {
        self.0 = value;
    }
}

impl fmt::Debug for SecretString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_empty() {
            f.write_str("SecretString(empty)")
        } else {
            f.write_str("SecretString([REDACTED])")
        }
    }
}

impl From<String> for SecretString {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for SecretString {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputConfig {
    #[serde(default = "default_format")]
    pub default_format: String,
    #[serde(default = "default_limit")]
    pub limit: usize,
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            default_format: default_format(),
            limit: default_limit(),
        }
    }
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
    pub api_key: SecretString,
    #[serde(default = "default_embedding_model")]
    pub model: String,
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            url: default_embedding_url(),
            api_key: SecretString::default(),
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
            self.api_key.set(value);
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
    pub api_key: SecretString,
    #[serde(default)]
    pub semantic_scholar_api_key: SecretString,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProfileConfig {
    #[serde(default)]
    pub data_dir: String,
    #[serde(default)]
    pub library_id: String,
    #[serde(default)]
    pub api_key: SecretString,
    #[serde(default)]
    pub semantic_scholar_api_key: SecretString,
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
    pub fn config_dir() -> ZotResult<PathBuf> {
        config_dir_from(dirs::config_dir())
    }

    pub fn config_file() -> ZotResult<PathBuf> {
        Ok(Self::config_dir()?.join(CONFIG_FILE_NAME))
    }

    pub fn state_dir() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(env::temp_dir)
            .join(CONFIG_DIR_NAME)
    }

    pub fn load_raw() -> ZotResult<Self> {
        let path = Self::config_file()?;
        if !path.exists() {
            return Ok(Self::default());
        }
        let raw = std::fs::read_to_string(&path).map_err(|source| ZotError::Io {
            path: path.clone(),
            source,
        })?;
        let mut parsed: Self = toml::from_str(&raw).map_err(|source| ZotError::ConfigParse {
            path: path.clone(),
            detail: source.to_string(),
        })?;
        parsed.normalize_legacy_output_defaults();
        parsed.validate()?;
        Ok(parsed)
    }

    pub fn load(profile: Option<&str>) -> ZotResult<Self> {
        Self::load_effective(profile).map(|(config, _)| config)
    }

    pub fn load_effective(profile: Option<&str>) -> ZotResult<(Self, Option<String>)> {
        let raw = Self::load_raw()?;
        Ok(raw.into_effective(profile))
    }

    pub fn into_effective(self, profile: Option<&str>) -> (Self, Option<String>) {
        let effective_profile = self.effective_profile_name(profile);
        (self.materialize_profile(profile), effective_profile)
    }

    pub fn save(&self) -> ZotResult<PathBuf> {
        let path = Self::config_file()?;
        self.save_to(&path)?;
        Ok(path)
    }

    fn save_to(&self, path: &Path) -> ZotResult<()> {
        self.validate()?;
        let encoded = toml::to_string_pretty(self).map_err(|source| ZotError::ConfigParse {
            path: path.to_path_buf(),
            detail: source.to_string(),
        })?;
        let dir = path.parent().ok_or_else(|| ZotError::InvalidInput {
            code: "config-dir-unavailable".to_string(),
            message: "Config file has no parent directory".to_string(),
            hint: Some("Use the platform user configuration directory".to_string()),
        })?;
        std::fs::create_dir_all(dir).map_err(|source| ZotError::Io {
            path: dir.to_path_buf(),
            source,
        })?;
        write_config_atomically(path, encoded.as_bytes())
    }

    fn validate(&self) -> ZotResult<()> {
        Self::validate_output(&self.output)?;
        for profile in self.profile.values() {
            Self::validate_output(&profile.output)?;
        }
        Ok(())
    }

    fn normalize_legacy_output_defaults(&mut self) {
        Self::normalize_output(&mut self.output);
        for profile in self.profile.values_mut() {
            Self::normalize_output(&mut profile.output);
        }
    }

    fn normalize_output(output: &mut OutputConfig) {
        if output.default_format.is_empty() {
            output.default_format = default_format();
        }
        if output.limit == 0 {
            output.limit = default_limit();
        }
    }

    fn validate_output(output: &OutputConfig) -> ZotResult<()> {
        if !matches!(output.default_format.as_str(), "table" | "json") {
            return Err(ZotError::InvalidInput {
                code: "config-value".to_string(),
                message: format!("Invalid output format '{}'", output.default_format),
                hint: Some("Use 'table' or 'json'".to_string()),
            });
        }
        if output.limit == 0 {
            return Err(ZotError::InvalidInput {
                code: "config-value".to_string(),
                message: "Output limit must be greater than zero".to_string(),
                hint: Some("Set output-limit to a positive integer".to_string()),
            });
        }
        Ok(())
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
            self.zotero.api_key.set(value);
        }
        if let Ok(value) = env::var("SEMANTIC_SCHOLAR_API_KEY") {
            self.zotero.semantic_scholar_api_key.set(value);
        }
        if let Ok(value) = env::var("S2_API_KEY") {
            self.zotero.semantic_scholar_api_key.set(value);
        }
    }

    pub fn write_credentials_configured(&self) -> bool {
        !self.zotero.library_id.is_empty() && !self.zotero.api_key.is_empty()
    }

    pub fn semantic_scholar_key(&self) -> Option<&str> {
        (!self.zotero.semantic_scholar_api_key.is_empty())
            .then_some(self.zotero.semantic_scholar_api_key.expose_secret())
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

    pub fn effective_profile_name(&self, explicit: Option<&str>) -> Option<String> {
        explicit
            .map(ToOwned::to_owned)
            .or_else(|| self.default_profile_name().map(ToOwned::to_owned))
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
    let mut chars = value.chars().rev();
    let suffix = chars.by_ref().take(4).collect::<Vec<_>>();
    if chars.next().is_none() {
        return "(set)".to_string();
    }
    format!("***{}", suffix.into_iter().rev().collect::<String>())
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
    let dir = AppConfig::config_dir()?;
    std::fs::create_dir_all(&dir).map_err(|source| ZotError::Io {
        path: dir.clone(),
        source,
    })?;
    Ok(dir)
}

fn config_dir_from(base: Option<PathBuf>) -> ZotResult<PathBuf> {
    base.map(|path| path.join(CONFIG_DIR_NAME))
        .ok_or_else(|| ZotError::InvalidInput {
            code: "config-dir-unavailable".to_string(),
            message: "Platform user configuration directory is unavailable".to_string(),
            hint: Some("Set up a user home/configuration directory and retry".to_string()),
        })
}

fn write_config_atomically(path: &Path, contents: &[u8]) -> ZotResult<()> {
    let dir = path.parent().ok_or_else(|| ZotError::InvalidInput {
        code: "config-dir-unavailable".to_string(),
        message: "Config file has no parent directory".to_string(),
        hint: Some("Use the platform user configuration directory".to_string()),
    })?;
    let mut temp = NamedTempFile::new_in(dir).map_err(|source| ZotError::Io {
        path: dir.to_path_buf(),
        source,
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        temp.as_file()
            .set_permissions(std::fs::Permissions::from_mode(0o600))
            .map_err(|source| ZotError::Io {
                path: temp.path().to_path_buf(),
                source,
            })?;
    }
    temp.write_all(contents).map_err(|source| ZotError::Io {
        path: temp.path().to_path_buf(),
        source,
    })?;
    temp.flush().map_err(|source| ZotError::Io {
        path: temp.path().to_path_buf(),
        source,
    })?;
    temp.as_file().sync_all().map_err(|source| ZotError::Io {
        path: temp.path().to_path_buf(),
        source,
    })?;
    temp.persist(path).map_err(|error| ZotError::Io {
        path: path.to_path_buf(),
        source: error.error,
    })?;
    #[cfg(unix)]
    std::fs::File::open(dir)
        .and_then(|directory| directory.sync_all())
        .map_err(|source| ZotError::Io {
            path: dir.to_path_buf(),
            source,
        })?;
    Ok(())
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

    #[test]
    fn old_bridge_config_fields_are_ignored() {
        let config: AppConfig =
            toml::from_str("[zotero]\ndata_dir = 'Zotero'\nwrite_backend = 'desktop'\n[zotero.desktop_bridge]\ntoken = 'secret-token'\n")
                .expect("parse legacy config");
        assert_eq!(config.zotero.data_dir, "Zotero");
    }

    #[test]
    fn effective_profile_prefers_explicit_then_default() {
        let mut config = AppConfig::default();
        config.set_default_profile(Some("default"));
        assert_eq!(
            config.effective_profile_name(Some("explicit")),
            Some("explicit".to_string())
        );
        assert_eq!(
            config.effective_profile_name(None),
            Some("default".to_string())
        );
    }

    #[test]
    fn secret_debug_is_redacted_and_toml_round_trips() {
        let canary = "secret-canary-1234";
        let mut config = AppConfig::default();
        config.zotero.api_key = canary.into();
        config.zotero.semantic_scholar_api_key = canary.into();
        config.embedding.api_key = canary.into();
        config.profile.insert(
            "work".to_string(),
            ProfileConfig {
                api_key: canary.into(),
                semantic_scholar_api_key: canary.into(),
                ..ProfileConfig::default()
            },
        );

        assert!(!format!("{config:?}").contains(canary));
        let encoded = toml::to_string(&config).expect("serialize config");
        let decoded: AppConfig = toml::from_str(&encoded).expect("deserialize config");
        assert_eq!(decoded.zotero.api_key.expose_secret(), canary);
        assert_eq!(decoded.embedding.api_key.expose_secret(), canary);
    }

    #[test]
    fn config_path_fails_closed_without_platform_directory() {
        let error = config_dir_from(None).expect_err("missing config dir must fail");
        assert_eq!(error.payload().code, "config-dir-unavailable");
    }

    #[test]
    fn redacts_unicode_secrets_on_character_boundaries() {
        assert_eq!(redact_secret("abcd"), "(set)");
        assert_eq!(redact_secret("abcde"), "***bcde");
        assert_eq!(redact_secret("密钥甲乙丙丁戊"), "***乙丙丁戊");
        assert_eq!(redact_secret("a🔒b密c钥"), "***b密c钥");
    }

    #[test]
    fn save_to_replaces_existing_config_without_temp_residue() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join(CONFIG_FILE_NAME);
        std::fs::write(&path, "old contents").expect("write old config");
        let mut config = AppConfig::default();
        config.zotero.api_key = "replacement-secret".into();

        config.save_to(&path).expect("atomic save");

        let saved = std::fs::read_to_string(&path).expect("read saved config");
        assert!(saved.contains("replacement-secret"));
        let entries = std::fs::read_dir(dir.path())
            .expect("read config dir")
            .collect::<Result<Vec<_>, _>>()
            .expect("collect config dir");
        assert_eq!(entries.len(), 1);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mode = std::fs::metadata(&path)
                .expect("config metadata")
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600);
        }
    }

    #[test]
    fn normalizes_legacy_empty_output_defaults_before_validation() {
        let mut config: AppConfig = toml::from_str(
            "[output]\ndefault_format = ''\nlimit = 0\n[profile.work.output]\ndefault_format = ''\nlimit = 0\n",
        )
        .expect("parse legacy output config");

        config.normalize_legacy_output_defaults();
        config.validate().expect("normalized config is valid");

        assert_eq!(config.output.default_format, "table");
        assert_eq!(config.output.limit, 50);
        assert_eq!(config.profile["work"].output.default_format, "table");
        assert_eq!(config.profile["work"].output.limit, 50);
    }
}
