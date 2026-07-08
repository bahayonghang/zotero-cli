use std::path::PathBuf;

use anyhow::Result;
use zot_core::config::ProfileConfig;
use zot_core::{AppConfig, canonicalize_or_original, detect_zotero_data_dir};

use crate::cli::{
    ConfigCommand, ConfigInitArgs, ConfigKeyArg, ConfigProfilesCommand, ConfigProfilesUseArgs,
    ConfigSetArgs,
};
use crate::context::AppContext;
use crate::output::CommandOutput;

pub(crate) async fn handle(ctx: &AppContext, command: ConfigCommand) -> Result<CommandOutput> {
    match command {
        ConfigCommand::Init(args) => handle_init(ctx, args).await,
        ConfigCommand::Show => handle_show(ctx).await,
        ConfigCommand::Set(args) => handle_set(ctx, args).await,
        ConfigCommand::Profiles { command } => handle_profiles(ctx, command).await,
    }
}

async fn handle_init(ctx: &AppContext, args: ConfigInitArgs) -> Result<CommandOutput> {
    let mut config = AppConfig::load_raw()?;
    let target_profile = args.target_profile.clone();
    let default_data_dir = detect_default_data_dir(&config);

    apply_init(
        &mut config_target(&mut config, target_profile.as_deref()),
        &args,
        default_data_dir,
    )?;

    if args.make_default {
        config.set_default_profile(target_profile.as_deref());
    }

    let path = canonicalize_or_original(&config.save()?);
    let payload = config_change_payload(&config, path, target_profile, "initialized");
    CommandOutput::new(ctx, payload, None, print_config_change)
}

async fn handle_show(ctx: &AppContext) -> Result<CommandOutput> {
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

    CommandOutput::new(ctx, payload, None, move |_| {
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
    })
}

async fn handle_set(ctx: &AppContext, args: ConfigSetArgs) -> Result<CommandOutput> {
    let mut config = AppConfig::load_raw()?;
    let target_profile = args.target_profile.clone();
    apply_setting(
        &mut config_target(&mut config, target_profile.as_deref()),
        &args.key,
        &args.value,
    )?;

    let path = canonicalize_or_original(&config.save()?);
    let payload = config_change_payload(&config, path, target_profile, "updated");
    CommandOutput::new(ctx, payload, None, print_config_change)
}

async fn handle_profiles(
    ctx: &AppContext,
    command: ConfigProfilesCommand,
) -> Result<CommandOutput> {
    match command {
        ConfigProfilesCommand::List => {
            let config = AppConfig::load_raw()?;
            let payload = serde_json::json!({
                "default_profile": config.default_profile_name(),
                "profiles": config.profile.keys().cloned().collect::<Vec<_>>(),
            });
            CommandOutput::new(ctx, payload, None, move |_| {
                if config.profile.is_empty() {
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
            })
        }
        ConfigProfilesCommand::Use(args) => handle_profiles_use(ctx, args).await,
    }
}

async fn handle_profiles_use(
    ctx: &AppContext,
    args: ConfigProfilesUseArgs,
) -> Result<CommandOutput> {
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
    let name = args.name;
    let payload = serde_json::json!({
        "config_file": path,
        "default_profile": name,
    });
    CommandOutput::new(ctx, payload, None, move |_| {
        println!("Default profile set to {name}");
        println!("Config file: {}", path.display());
    })
}

/// Mutable write target for config commands: the root tables or one named profile.
enum ConfigTarget<'a> {
    Root(&'a mut AppConfig),
    Profile(&'a mut ProfileConfig),
}

/// Field slot a config key resolves to on a given target.
enum SettingSlot<'a> {
    Text(&'a mut String),
    Limit(&'a mut usize),
}

fn config_target<'a>(config: &'a mut AppConfig, profile_name: Option<&str>) -> ConfigTarget<'a> {
    match profile_name {
        Some(name) => ConfigTarget::Profile(config.profile.entry(name.to_string()).or_default()),
        None => ConfigTarget::Root(config),
    }
}

/// Single source of truth for where every config key lives on each target.
/// Adding a new `ConfigKeyArg` variant only requires one new arm here — the
/// exhaustive match refuses to compile until the variant is mapped. A `None`
/// profile arm marks the key as root-only; `apply_setting` turns that into
/// the rejection error.
fn setting_slot<'a>(
    target: &'a mut ConfigTarget<'_>,
    key: &ConfigKeyArg,
) -> Option<SettingSlot<'a>> {
    use SettingSlot::{Limit, Text};
    match key {
        ConfigKeyArg::DataDir => match target {
            ConfigTarget::Root(config) => Some(Text(&mut config.zotero.data_dir)),
            ConfigTarget::Profile(profile) => Some(Text(&mut profile.data_dir)),
        },
        ConfigKeyArg::LibraryId => match target {
            ConfigTarget::Root(config) => Some(Text(&mut config.zotero.library_id)),
            ConfigTarget::Profile(profile) => Some(Text(&mut profile.library_id)),
        },
        ConfigKeyArg::ApiKey => match target {
            ConfigTarget::Root(config) => Some(Text(&mut config.zotero.api_key)),
            ConfigTarget::Profile(profile) => Some(Text(&mut profile.api_key)),
        },
        ConfigKeyArg::SemanticScholarApiKey => match target {
            ConfigTarget::Root(config) => Some(Text(&mut config.zotero.semantic_scholar_api_key)),
            ConfigTarget::Profile(profile) => Some(Text(&mut profile.semantic_scholar_api_key)),
        },
        ConfigKeyArg::EmbeddingUrl => match target {
            ConfigTarget::Root(config) => Some(Text(&mut config.embedding.url)),
            ConfigTarget::Profile(_) => None,
        },
        ConfigKeyArg::EmbeddingKey => match target {
            ConfigTarget::Root(config) => Some(Text(&mut config.embedding.api_key)),
            ConfigTarget::Profile(_) => None,
        },
        ConfigKeyArg::EmbeddingModel => match target {
            ConfigTarget::Root(config) => Some(Text(&mut config.embedding.model)),
            ConfigTarget::Profile(_) => None,
        },
        ConfigKeyArg::OutputFormat => match target {
            ConfigTarget::Root(config) => Some(Text(&mut config.output.default_format)),
            ConfigTarget::Profile(profile) => Some(Text(&mut profile.output.default_format)),
        },
        ConfigKeyArg::OutputLimit => match target {
            ConfigTarget::Root(config) => Some(Limit(&mut config.output.limit)),
            ConfigTarget::Profile(profile) => Some(Limit(&mut profile.output.limit)),
        },
        ConfigKeyArg::ExportStyle => match target {
            ConfigTarget::Root(config) => Some(Text(&mut config.export.default_style)),
            ConfigTarget::Profile(profile) => Some(Text(&mut profile.export.default_style)),
        },
    }
}

/// Applies one key/value to the target. Root-only keys are rejected for
/// profile targets; this is the single rejection rule shared by `config init`
/// and `config set`.
fn apply_setting(target: &mut ConfigTarget<'_>, key: &ConfigKeyArg, value: &str) -> Result<()> {
    match setting_slot(target, key) {
        Some(SettingSlot::Text(slot)) => *slot = value.to_string(),
        Some(SettingSlot::Limit(slot)) => *slot = parse_limit(value)?,
        None => {
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

/// Applies `config init` flags through the same `apply_setting` path.
fn apply_init(
    target: &mut ConfigTarget<'_>,
    args: &ConfigInitArgs,
    default_data_dir: String,
) -> Result<()> {
    if let Some(SettingSlot::Text(data_dir)) = setting_slot(target, &ConfigKeyArg::DataDir)
        && data_dir.is_empty()
    {
        *data_dir = default_data_dir;
    }

    for (key, value) in provided_init_settings(args) {
        // `config init` keeps its historical behavior of silently ignoring
        // root-only keys on profile targets; `config set` rejects them.
        if setting_slot(target, &key).is_some() {
            apply_setting(target, &key, value)?;
        }
    }
    Ok(())
}

fn provided_init_settings(args: &ConfigInitArgs) -> Vec<(ConfigKeyArg, &str)> {
    [
        (ConfigKeyArg::DataDir, args.data_dir.as_deref()),
        (ConfigKeyArg::LibraryId, args.library_id.as_deref()),
        (ConfigKeyArg::ApiKey, args.api_key.as_deref()),
        (
            ConfigKeyArg::SemanticScholarApiKey,
            args.semantic_scholar_api_key.as_deref(),
        ),
        (ConfigKeyArg::EmbeddingUrl, args.embedding_url.as_deref()),
        (ConfigKeyArg::EmbeddingKey, args.embedding_key.as_deref()),
        (
            ConfigKeyArg::EmbeddingModel,
            args.embedding_model.as_deref(),
        ),
    ]
    .into_iter()
    .filter_map(|(key, value)| value.map(|value| (key, value)))
    .collect()
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

    fn init_args() -> ConfigInitArgs {
        ConfigInitArgs {
            target_profile: None,
            make_default: false,
            data_dir: None,
            library_id: None,
            api_key: None,
            semantic_scholar_api_key: None,
            embedding_url: None,
            embedding_key: None,
            embedding_model: None,
        }
    }

    #[test]
    fn updates_profile_specific_settings() {
        let mut profile = ProfileConfig::default();
        apply_setting(
            &mut ConfigTarget::Profile(&mut profile),
            &ConfigKeyArg::LibraryId,
            "42",
        )
        .expect("set library id");
        assert_eq!(profile.library_id, "42");

        let err = apply_setting(
            &mut ConfigTarget::Profile(&mut profile),
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
    fn applies_same_key_through_root_and_profile_targets() {
        let mut config = AppConfig::default();
        apply_setting(
            &mut ConfigTarget::Root(&mut config),
            &ConfigKeyArg::LibraryId,
            "42",
        )
        .expect("root library id");
        apply_setting(
            &mut ConfigTarget::Root(&mut config),
            &ConfigKeyArg::OutputLimit,
            "25",
        )
        .expect("root output limit");
        assert_eq!(config.zotero.library_id, "42");
        assert_eq!(config.output.limit, 25);

        let mut profile = ProfileConfig::default();
        apply_setting(
            &mut ConfigTarget::Profile(&mut profile),
            &ConfigKeyArg::LibraryId,
            "42",
        )
        .expect("profile library id");
        apply_setting(
            &mut ConfigTarget::Profile(&mut profile),
            &ConfigKeyArg::OutputLimit,
            "25",
        )
        .expect("profile output limit");
        assert_eq!(profile.library_id, "42");
        assert_eq!(profile.output.limit, 25);
    }

    #[test]
    fn every_key_applies_at_root_and_root_only_keys_reject_profiles() {
        use clap::ValueEnum;

        for key in ConfigKeyArg::value_variants() {
            let value = match key {
                ConfigKeyArg::OutputLimit => "25",
                _ => "value",
            };

            let mut config = AppConfig::default();
            apply_setting(&mut ConfigTarget::Root(&mut config), key, value)
                .expect("every key must apply at the root level");

            let mut profile = ProfileConfig::default();
            let result = apply_setting(&mut ConfigTarget::Profile(&mut profile), key, value);
            match key {
                ConfigKeyArg::EmbeddingUrl
                | ConfigKeyArg::EmbeddingKey
                | ConfigKeyArg::EmbeddingModel => {
                    let err = result.expect_err("root-only keys must be rejected for profiles");
                    let err = err.downcast_ref::<zot_core::ZotError>().expect("zot error");
                    match err {
                        zot_core::ZotError::InvalidInput {
                            code,
                            message,
                            hint,
                        } => {
                            assert_eq!(code, "config-key");
                            assert_eq!(
                                message,
                                &format!(
                                    "Key '{}' is only supported at the root config level",
                                    key.as_str()
                                )
                            );
                            assert_eq!(
                                hint.as_deref(),
                                Some("Use 'zot config set <key> <value>' without --target-profile")
                            );
                        }
                        other => panic!("unexpected error: {other:?}"),
                    }
                }
                _ => {
                    result.expect("shared keys must apply to profiles");
                }
            }
        }
    }

    #[test]
    fn init_reuses_apply_setting_and_skips_root_only_keys_for_profiles() {
        let mut args = init_args();
        args.library_id = Some("7".to_string());
        args.embedding_url = Some("https://example.com".to_string());

        let mut config = AppConfig::default();
        apply_init(
            &mut ConfigTarget::Root(&mut config),
            &args,
            "detected-dir".to_string(),
        )
        .expect("root init");
        assert_eq!(config.zotero.data_dir, "detected-dir");
        assert_eq!(config.zotero.library_id, "7");
        assert_eq!(config.embedding.url, "https://example.com");

        let mut profile = ProfileConfig::default();
        apply_init(
            &mut ConfigTarget::Profile(&mut profile),
            &args,
            "detected-dir".to_string(),
        )
        .expect("profile init ignores root-only keys");
        assert_eq!(profile.data_dir, "detected-dir");
        assert_eq!(profile.library_id, "7");
    }

    #[test]
    fn init_keeps_explicit_and_existing_data_dir_over_detected_default() {
        let mut args = init_args();
        args.data_dir = Some("explicit-dir".to_string());

        let mut config = AppConfig::default();
        apply_init(
            &mut ConfigTarget::Root(&mut config),
            &args,
            "detected-dir".to_string(),
        )
        .expect("root init");
        assert_eq!(config.zotero.data_dir, "explicit-dir");

        let mut profile = ProfileConfig {
            data_dir: "existing-dir".to_string(),
            ..ProfileConfig::default()
        };
        apply_init(
            &mut ConfigTarget::Profile(&mut profile),
            &init_args(),
            "detected-dir".to_string(),
        )
        .expect("profile init");
        assert_eq!(profile.data_dir, "existing-dir");
    }

    #[test]
    fn parses_output_limit_for_config_updates() {
        assert_eq!(parse_limit("25").expect("limit"), 25);
        assert!(parse_limit("bad").is_err());
    }
}
