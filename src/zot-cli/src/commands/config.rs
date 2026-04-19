use std::path::PathBuf;

use anyhow::Result;
use zot_core::config::ProfileConfig;
use zot_core::{AppConfig, canonicalize_or_original, detect_zotero_data_dir};

use crate::cli::{
    ConfigCommand, ConfigInitArgs, ConfigKeyArg, ConfigProfilesCommand, ConfigProfilesUseArgs,
    ConfigSetArgs,
};
use crate::context::AppContext;
use crate::format::print_enveloped;

pub(crate) async fn handle(ctx: &AppContext, command: ConfigCommand) -> Result<()> {
    match command {
        ConfigCommand::Init(args) => handle_init(ctx, args).await,
        ConfigCommand::Show => handle_show(ctx).await,
        ConfigCommand::Set(args) => handle_set(ctx, args).await,
        ConfigCommand::Profiles { command } => handle_profiles(ctx, command).await,
    }
}

async fn handle_init(ctx: &AppContext, args: ConfigInitArgs) -> Result<()> {
    let mut config = AppConfig::load_raw()?;
    let target_profile = args.target_profile.clone();
    let default_data_dir = detect_default_data_dir(&config);

    if let Some(profile_name) = target_profile.as_deref() {
        let profile = config.profile.entry(profile_name.to_string()).or_default();
        apply_profile_init(profile, &args, default_data_dir);
    } else {
        apply_root_init(&mut config, &args, default_data_dir);
    }

    if args.make_default {
        config.set_default_profile(target_profile.as_deref());
    }

    let path = canonicalize_or_original(&config.save()?);
    let payload = config_change_payload(&config, path, target_profile, "initialized");
    if ctx.json {
        print_enveloped(payload, None)?;
    } else {
        print_config_change(&payload);
    }
    Ok(())
}

async fn handle_show(ctx: &AppContext) -> Result<()> {
    let raw = AppConfig::load_raw()?;
    let effective = AppConfig::load(ctx.profile.as_deref())?;
    let selected_profile = ctx
        .profile
        .clone()
        .or_else(|| raw.default_profile_name().map(ToOwned::to_owned));
    let path = canonicalize_or_original(&AppConfig::config_file());
    let payload = serde_json::json!({
        "config_file": path,
        "default_profile": raw.default_profile_name(),
        "selected_profile": selected_profile,
        "profiles": raw.profile.keys().cloned().collect::<Vec<_>>(),
        "effective": config_view(&effective),
    });

    if ctx.json {
        print_enveloped(payload, None)?;
    } else {
        println!("Config file: {}", path.display());
        println!(
            "Default profile: {}",
            raw.default_profile_name().unwrap_or("(root)")
        );
        println!(
            "Selected profile: {}",
            selected_profile.as_deref().unwrap_or("(root)")
        );
        println!(
            "Profiles: {}",
            raw.profile.keys().cloned().collect::<Vec<_>>().join(", ")
        );
        println!("Data dir: {}", effective.zotero.data_dir);
        println!(
            "Library ID: {}",
            blank_or_value(&effective.zotero.library_id)
        );
        println!("API key: {}", redact_or_missing(&effective.zotero.api_key));
        println!(
            "Semantic Scholar key: {}",
            redact_or_missing(&effective.zotero.semantic_scholar_api_key)
        );
        println!("Embedding URL: {}", effective.embedding.url);
        println!(
            "Embedding key: {}",
            redact_or_missing(&effective.embedding.api_key)
        );
        println!("Embedding model: {}", effective.embedding.model);
    }
    Ok(())
}

async fn handle_set(ctx: &AppContext, args: ConfigSetArgs) -> Result<()> {
    let mut config = AppConfig::load_raw()?;
    let target_profile = args.target_profile.clone();
    if let Some(profile_name) = target_profile.as_deref() {
        let profile = config.profile.entry(profile_name.to_string()).or_default();
        apply_profile_setting(profile, &args.key, &args.value)?;
    } else {
        apply_root_setting(&mut config, &args.key, &args.value)?;
    }

    let path = canonicalize_or_original(&config.save()?);
    let payload = config_change_payload(&config, path, target_profile, "updated");
    if ctx.json {
        print_enveloped(payload, None)?;
    } else {
        print_config_change(&payload);
    }
    Ok(())
}

async fn handle_profiles(ctx: &AppContext, command: ConfigProfilesCommand) -> Result<()> {
    match command {
        ConfigProfilesCommand::List => {
            let config = AppConfig::load_raw()?;
            let payload = serde_json::json!({
                "default_profile": config.default_profile_name(),
                "profiles": config.profile.keys().cloned().collect::<Vec<_>>(),
            });
            if ctx.json {
                print_enveloped(payload, None)?;
            } else if config.profile.is_empty() {
                println!("No named profiles configured.");
            } else {
                let default_profile = config.default_profile_name();
                for profile_name in config.profile.keys() {
                    if Some(profile_name.as_str()) == default_profile {
                        println!("{profile_name} (default)");
                    } else {
                        println!("{profile_name}");
                    }
                }
            }
        }
        ConfigProfilesCommand::Use(args) => handle_profiles_use(ctx, args).await?,
    }
    Ok(())
}

async fn handle_profiles_use(ctx: &AppContext, args: ConfigProfilesUseArgs) -> Result<()> {
    let mut config = AppConfig::load_raw()?;
    if !config.profile.contains_key(&args.name) {
        return Err(zot_core::ZotError::InvalidInput {
            code: "config-profile".to_string(),
            message: format!("Profile '{}' not found", args.name),
            hint: Some("Run 'zot config init --target-profile <name>' first".to_string()),
        }
        .into());
    }
    config.set_default_profile(Some(&args.name));
    let path = canonicalize_or_original(&config.save()?);
    let payload = serde_json::json!({
        "config_file": path,
        "default_profile": args.name,
    });
    if ctx.json {
        print_enveloped(payload, None)?;
    } else {
        println!("Default profile set to {}", args.name);
        println!("Config file: {}", path.display());
    }
    Ok(())
}

fn apply_root_init(config: &mut AppConfig, args: &ConfigInitArgs, default_data_dir: String) {
    if config.zotero.data_dir.is_empty() {
        config.zotero.data_dir = default_data_dir;
    }
    if let Some(value) = args.data_dir.as_deref() {
        config.zotero.data_dir = value.to_string();
    }
    if let Some(value) = args.library_id.as_deref() {
        config.zotero.library_id = value.to_string();
    }
    if let Some(value) = args.api_key.as_deref() {
        config.zotero.api_key = value.to_string();
    }
    if let Some(value) = args.semantic_scholar_api_key.as_deref() {
        config.zotero.semantic_scholar_api_key = value.to_string();
    }
    if let Some(value) = args.embedding_url.as_deref() {
        config.embedding.url = value.to_string();
    }
    if let Some(value) = args.embedding_key.as_deref() {
        config.embedding.api_key = value.to_string();
    }
    if let Some(value) = args.embedding_model.as_deref() {
        config.embedding.model = value.to_string();
    }
}

fn apply_profile_init(
    profile: &mut ProfileConfig,
    args: &ConfigInitArgs,
    default_data_dir: String,
) {
    if profile.data_dir.is_empty() {
        profile.data_dir = default_data_dir;
    }
    if let Some(value) = args.data_dir.as_deref() {
        profile.data_dir = value.to_string();
    }
    if let Some(value) = args.library_id.as_deref() {
        profile.library_id = value.to_string();
    }
    if let Some(value) = args.api_key.as_deref() {
        profile.api_key = value.to_string();
    }
    if let Some(value) = args.semantic_scholar_api_key.as_deref() {
        profile.semantic_scholar_api_key = value.to_string();
    }
}

fn apply_root_setting(config: &mut AppConfig, key: &ConfigKeyArg, value: &str) -> Result<()> {
    match key {
        ConfigKeyArg::DataDir => config.zotero.data_dir = value.to_string(),
        ConfigKeyArg::LibraryId => config.zotero.library_id = value.to_string(),
        ConfigKeyArg::ApiKey => config.zotero.api_key = value.to_string(),
        ConfigKeyArg::SemanticScholarApiKey => {
            config.zotero.semantic_scholar_api_key = value.to_string()
        }
        ConfigKeyArg::EmbeddingUrl => config.embedding.url = value.to_string(),
        ConfigKeyArg::EmbeddingKey => config.embedding.api_key = value.to_string(),
        ConfigKeyArg::EmbeddingModel => config.embedding.model = value.to_string(),
        ConfigKeyArg::OutputFormat => config.output.default_format = value.to_string(),
        ConfigKeyArg::OutputLimit => config.output.limit = parse_limit(value)?,
        ConfigKeyArg::ExportStyle => config.export.default_style = value.to_string(),
    }
    Ok(())
}

fn apply_profile_setting(
    profile: &mut ProfileConfig,
    key: &ConfigKeyArg,
    value: &str,
) -> Result<()> {
    match key {
        ConfigKeyArg::DataDir => profile.data_dir = value.to_string(),
        ConfigKeyArg::LibraryId => profile.library_id = value.to_string(),
        ConfigKeyArg::ApiKey => profile.api_key = value.to_string(),
        ConfigKeyArg::SemanticScholarApiKey => profile.semantic_scholar_api_key = value.to_string(),
        ConfigKeyArg::OutputFormat => profile.output.default_format = value.to_string(),
        ConfigKeyArg::OutputLimit => profile.output.limit = parse_limit(value)?,
        ConfigKeyArg::ExportStyle => profile.export.default_style = value.to_string(),
        ConfigKeyArg::EmbeddingUrl | ConfigKeyArg::EmbeddingKey | ConfigKeyArg::EmbeddingModel => {
            return Err(zot_core::ZotError::InvalidInput {
                code: "config-key".to_string(),
                message: format!(
                    "Key '{}' is only supported at the root config level",
                    key.as_str()
                ),
                hint: Some(
                    "Use 'zot config set <key> <value>' without --target-profile".to_string(),
                ),
            }
            .into());
        }
    }
    Ok(())
}

fn parse_limit(value: &str) -> Result<usize> {
    value.parse::<usize>().map_err(|_| {
        zot_core::ZotError::InvalidInput {
            code: "config-limit".to_string(),
            message: format!("Invalid output limit '{}'", value),
            hint: Some("Pass a positive integer".to_string()),
        }
        .into()
    })
}

fn config_change_payload(
    config: &AppConfig,
    path: PathBuf,
    target_profile: Option<String>,
    status: &str,
) -> serde_json::Value {
    serde_json::json!({
        "config_file": path,
        "status": status,
        "target_profile": target_profile,
        "default_profile": config.default_profile_name(),
        "effective": config_view(config),
    })
}

fn config_view(config: &AppConfig) -> serde_json::Value {
    serde_json::json!({
        "data_dir": blank_or_value(&config.zotero.data_dir),
        "library_id": blank_or_value(&config.zotero.library_id),
        "api_key": redact_or_missing(&config.zotero.api_key),
        "semantic_scholar_api_key": redact_or_missing(&config.zotero.semantic_scholar_api_key),
        "embedding": {
            "url": blank_or_value(&config.embedding.url),
            "api_key": redact_or_missing(&config.embedding.api_key),
            "model": blank_or_value(&config.embedding.model),
        },
        "output": {
            "default_format": config.output.default_format,
            "limit": config.output.limit,
        },
        "export": {
            "default_style": config.export.default_style,
        },
    })
}

fn print_config_change(payload: &serde_json::Value) {
    let config_file = payload
        .get("config_file")
        .and_then(|value| value.as_str())
        .unwrap_or_default();
    let status = payload
        .get("status")
        .and_then(|value| value.as_str())
        .unwrap_or("updated");
    let target_profile = payload
        .get("target_profile")
        .and_then(|value| value.as_str())
        .unwrap_or("(root)");
    println!("Config {status}: {target_profile}");
    println!("Config file: {config_file}");
}

fn detect_default_data_dir(config: &AppConfig) -> String {
    detect_zotero_data_dir(config).display().to_string()
}

fn redact_or_missing(value: &str) -> String {
    if value.is_empty() {
        "(missing)".to_string()
    } else {
        zot_core::redact_secret(value)
    }
}

fn blank_or_value(value: &str) -> String {
    if value.is_empty() {
        "(missing)".to_string()
    } else {
        value.to_string()
    }
}

trait ConfigKeyArgExt {
    fn as_str(&self) -> &'static str;
}

impl ConfigKeyArgExt for ConfigKeyArg {
    fn as_str(&self) -> &'static str {
        match self {
            ConfigKeyArg::DataDir => "data-dir",
            ConfigKeyArg::LibraryId => "library-id",
            ConfigKeyArg::ApiKey => "api-key",
            ConfigKeyArg::SemanticScholarApiKey => "semantic-scholar-api-key",
            ConfigKeyArg::EmbeddingUrl => "embedding-url",
            ConfigKeyArg::EmbeddingKey => "embedding-key",
            ConfigKeyArg::EmbeddingModel => "embedding-model",
            ConfigKeyArg::OutputFormat => "output-format",
            ConfigKeyArg::OutputLimit => "output-limit",
            ConfigKeyArg::ExportStyle => "export-style",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn updates_profile_specific_settings() {
        let mut profile = ProfileConfig::default();
        apply_profile_setting(&mut profile, &ConfigKeyArg::LibraryId, "42")
            .expect("set library id");
        assert_eq!(profile.library_id, "42");

        let err = apply_profile_setting(
            &mut profile,
            &ConfigKeyArg::EmbeddingUrl,
            "https://example.com",
        )
        .expect_err("embedding url should fail for profile");
        let err = err.downcast_ref::<zot_core::ZotError>().expect("zot error");
        match err {
            zot_core::ZotError::InvalidInput { code, .. } => assert_eq!(code, "config-key"),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn parses_output_limit_for_config_updates() {
        assert_eq!(parse_limit("25").expect("limit"), 25);
        assert!(parse_limit("bad").is_err());
    }
}
