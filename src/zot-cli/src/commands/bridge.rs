use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use anyhow::Result;
use chrono::{SecondsFormat, Utc};
use serde_json::Value;
use uuid::Uuid;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};
use zot_core::{AppConfig, DesktopBridgeConfig, ZotError, bridge_connection_id};
use zot_desktop::{
    BRIDGE_PROTOCOL_VERSION, BridgeHealth, BridgeStatus, ensure_matching_instance_id,
};

use crate::cli::{BridgeCommand, BridgePairArgs, BridgeRevokeArgs, BridgeSetupArgs};
use crate::context::AppContext;
use crate::output::CommandOutput;

const PLUGIN_ID: &str = "zot-bridge@bahayonghang";
const MANIFEST: &[u8] = include_bytes!("../../../../plugins/zot-bridge/manifest.json");
const BOOTSTRAP: &[u8] = include_bytes!("../../../../plugins/zot-bridge/bootstrap.js");
const PLUGIN_ASSETS: &[(&str, &[u8])] = &[("manifest.json", MANIFEST), ("bootstrap.js", BOOTSTRAP)];

pub(crate) async fn handle(ctx: &AppContext, command: BridgeCommand) -> Result<CommandOutput> {
    match command {
        BridgeCommand::Setup(args) => handle_setup(ctx, args).await,
        BridgeCommand::Pair(args) => handle_pair(ctx, args).await,
        BridgeCommand::Status => handle_status(ctx).await,
        BridgeCommand::Revoke(args) => handle_revoke(ctx, args).await,
    }
}

async fn handle_setup(ctx: &AppContext, args: BridgeSetupArgs) -> Result<CommandOutput> {
    let output_dir = args
        .output
        .unwrap_or_else(|| AppConfig::config_dir().join("plugins"));
    let xpi_path = write_xpi(&output_dir)?;
    let opened = opener::open(&output_dir).is_ok();
    let configured = ctx.config.zotero.desktop_bridge.is_configured();
    let next_steps = setup_next_steps(configured);
    let payload = serde_json::json!({
        "xpi_path": xpi_path,
        "plugin_id": PLUGIN_ID,
        "plugin_version": env!("CARGO_PKG_VERSION"),
        "opened_directory": opened,
        "next_steps": next_steps,
    });
    CommandOutput::new(ctx, payload, None, move |_| {
        println!("Zot Bridge XPI: {}", xpi_path.display());
        println!("Install it from Zotero Add-ons Manager, then restart Zotero.");
        if configured {
            println!("The existing authorization for this Zotero profile will be reused.");
            println!("After restart, run: zot bridge status");
        } else {
            println!("After Zotero shows a pairing code, run: zot bridge pair <code>");
        }
        if !opened {
            println!("The output directory could not be opened automatically.");
        }
    })
}

fn setup_next_steps(configured: bool) -> Vec<&'static str> {
    if configured {
        vec![
            "Install or upgrade the XPI from Zotero Add-ons Manager",
            "Restart the same Zotero profile",
            "Run `zot bridge status` to verify the existing authorization",
        ]
    } else {
        vec![
            "Install the XPI from Zotero Add-ons Manager",
            "Restart Zotero",
            "Show the pairing code from the Zot Bridge Tools menu",
            "Run `zot bridge pair <code>`",
        ]
    }
}

async fn handle_pair(ctx: &AppContext, args: BridgePairArgs) -> Result<CommandOutput> {
    let client = ctx.desktop()?;
    let health = client.health().await?;
    let pair = client.pair(&args.code, &Uuid::new_v4().to_string()).await?;
    ensure_matching_instance_id(&health.instance_id, &pair.instance_id)?;
    let instance_id = if pair.instance_id.is_empty() {
        health.instance_id.clone()
    } else {
        pair.instance_id.clone()
    };
    if pair.protocol_version != BRIDGE_PROTOCOL_VERSION {
        return Err(ZotError::DesktopBridge {
            code: "bridge-protocol".to_string(),
            message: "Pair response used an incompatible protocol".to_string(),
            hint: Some("Update zot or reinstall the matching bridge plugin".to_string()),
            status: None,
        }
        .into());
    }

    let mut raw = AppConfig::load_raw()?;
    let target_profile = raw.effective_profile_name(ctx.profile.as_deref());
    raw.set_desktop_bridge(
        target_profile.as_deref(),
        DesktopBridgeConfig {
            token: pair.token,
            instance_id: instance_id.clone(),
            plugin_version: pair.plugin_version.clone(),
            protocol_version: Some(pair.protocol_version),
            paired_at: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
        },
    );
    let config_file = raw.save()?;
    let payload = serde_json::json!({
        "paired": true,
        "write_backend": "desktop",
        "target_profile": target_profile,
        "config_file": config_file,
        "plugin_version": pair.plugin_version,
        "connection_id": bridge_connection_id(&instance_id),
        "zotero_version": health.zotero_version,
        "protocol_version": pair.protocol_version,
    });
    CommandOutput::new(ctx, payload, None, move |_| {
        println!("Zot Bridge paired successfully.");
        println!(
            "Write backend for {} is now desktop.",
            target_profile.as_deref().unwrap_or("root config")
        );
    })
}

async fn handle_status(ctx: &AppContext) -> Result<CommandOutput> {
    let client = ctx.desktop()?;
    let health = client.health().await?;
    let configured = ctx.config.zotero.desktop_bridge.is_configured();
    let status = if configured {
        ensure_matching_instance_id(
            &ctx.config.zotero.desktop_bridge.instance_id,
            &health.instance_id,
        )?;
        let status = client
            .status(&ctx.config.zotero.desktop_bridge.token)
            .await?;
        ensure_matching_instance_id(&health.instance_id, &status.instance_id)?;
        persist_migrated_instance_id(ctx, &health.instance_id)?;
        Some(status)
    } else {
        None
    };
    let available = status.as_ref().is_some_and(|value| value.paired);
    let payload = bridge_status_payload(
        &health,
        status.as_ref(),
        configured,
        available,
        ctx.write_backend(),
    );
    CommandOutput::new(ctx, payload, None, move |_| {
        println!("Zot Bridge plugin: {}", health.plugin_version);
        println!("Zotero: {}", health.zotero_version);
        if let Some(connection_id) = bridge_connection_id(&health.instance_id) {
            println!("Connection: {connection_id}");
        }
        println!(
            "Pairing: {}",
            if available { "available" } else { "not paired" }
        );
    })
}

async fn handle_revoke(ctx: &AppContext, args: BridgeRevokeArgs) -> Result<CommandOutput> {
    let mut raw = AppConfig::load_raw()?;
    let target_profile = raw.effective_profile_name(ctx.profile.as_deref());
    let bridge = raw
        .desktop_bridge_for_target(target_profile.as_deref())
        .cloned()
        .ok_or_else(|| ZotError::InvalidInput {
            code: "bridge-profile".to_string(),
            message: "The selected config profile does not exist".to_string(),
            hint: Some("Run `zot config profiles list`".to_string()),
        })?;
    if !args.local_only {
        if !bridge.is_configured() {
            return Err(ZotError::InvalidInput {
                code: "bridge-unpaired".to_string(),
                message: "The selected config target has no desktop bridge token".to_string(),
                hint: Some("Run `zot bridge pair <code>`".to_string()),
            }
            .into());
        }
        let client = ctx.desktop()?;
        let health = client.health().await?;
        ensure_matching_instance_id(&bridge.instance_id, &health.instance_id)?;
        client.revoke(&bridge.token).await?;
    }
    raw.clear_desktop_bridge(target_profile.as_deref());
    let config_file = raw.save()?;
    let payload = serde_json::json!({
        "revoked": !args.local_only,
        "local_credentials_cleared": true,
        "write_backend": write_backend_name(ctx.write_backend()),
        "target_profile": target_profile,
        "config_file": config_file,
    });
    CommandOutput::new(ctx, payload, None, move |_| {
        if args.local_only {
            println!("Local Zot Bridge credentials cleared.");
            println!("Reset authorization from Zotero Tools if the plugin token still exists.");
        } else {
            println!("Zot Bridge authorization revoked and local credentials cleared.");
        }
    })
}

fn bridge_status_payload(
    health: &BridgeHealth,
    status: Option<&BridgeStatus>,
    configured: bool,
    available: bool,
    write_backend: zot_core::WriteBackend,
) -> Value {
    serde_json::json!({
        "installed": true,
        "configured": configured,
        "available": available,
        "write_backend": write_backend_name(write_backend),
        "plugin_version": health.plugin_version,
        "zotero_version": health.zotero_version,
        "protocol_version": health.protocol_version,
        "connection_id": bridge_connection_id(&health.instance_id),
        "capabilities": status.map(|value| &value.capabilities),
        "libraries": status.map(|value| &value.libraries),
        "hint": if configured {
            Value::Null
        } else {
            Value::String("Show a pairing code in Zotero and run `zot bridge pair <code>`".to_string())
        },
    })
}

fn persist_migrated_instance_id(ctx: &AppContext, observed: &str) -> Result<bool> {
    if observed.is_empty() || !ctx.config.zotero.desktop_bridge.instance_id.is_empty() {
        return Ok(false);
    }
    let mut raw = AppConfig::load_raw()?;
    let target_profile = raw.effective_profile_name(ctx.profile.as_deref());
    let bridge = raw
        .desktop_bridge_for_target(target_profile.as_deref())
        .ok_or_else(|| ZotError::InvalidInput {
            code: "bridge-profile".to_string(),
            message: "The selected config profile does not exist".to_string(),
            hint: Some("Run `zot config profiles list`".to_string()),
        })?;
    if !bridge.is_configured() || !bridge.instance_id.is_empty() {
        return Ok(false);
    }
    if !raw.set_desktop_bridge_instance_id(target_profile.as_deref(), observed.to_string()) {
        return Ok(false);
    }
    raw.save()?;
    Ok(true)
}

fn write_backend_name(value: zot_core::WriteBackend) -> &'static str {
    match value {
        zot_core::WriteBackend::Web => "web",
        zot_core::WriteBackend::Desktop => "desktop",
    }
}

fn write_xpi(output_dir: &Path) -> Result<PathBuf> {
    validate_manifest()?;
    std::fs::create_dir_all(output_dir).map_err(|source| ZotError::Io {
        path: output_dir.to_path_buf(),
        source,
    })?;
    let target = output_dir.join(format!("zot-bridge-{}.xpi", env!("CARGO_PKG_VERSION")));
    let temp = output_dir.join(format!(".zot-bridge-{}.tmp", Uuid::new_v4()));
    let file = File::create(&temp).map_err(|source| ZotError::Io {
        path: temp.clone(),
        source,
    })?;
    let mut writer = ZipWriter::new(file);
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .unix_permissions(0o644);
    for (name, bytes) in PLUGIN_ASSETS {
        writer.start_file(*name, options).map_err(xpi_error)?;
        writer.write_all(bytes).map_err(|source| ZotError::Io {
            path: temp.clone(),
            source,
        })?;
    }
    writer.finish().map_err(xpi_error)?;
    if target.exists() {
        std::fs::remove_file(&target).map_err(|source| ZotError::Io {
            path: target.clone(),
            source,
        })?;
    }
    std::fs::rename(&temp, &target).map_err(|source| ZotError::Io {
        path: target.clone(),
        source,
    })?;
    validate_xpi(&target)?;
    Ok(target)
}

fn validate_manifest() -> Result<()> {
    let manifest: Value =
        serde_json::from_slice(MANIFEST).map_err(|err| ZotError::InvalidInput {
            code: "bridge-manifest".to_string(),
            message: format!("Invalid embedded bridge manifest: {err}"),
            hint: None,
        })?;
    if manifest["version"] != env!("CARGO_PKG_VERSION")
        || manifest["applications"]["zotero"]["id"] != PLUGIN_ID
    {
        return Err(ZotError::InvalidInput {
            code: "bridge-manifest".to_string(),
            message: "Bridge manifest version or plugin ID does not match the CLI".to_string(),
            hint: Some("Keep workspace and plugin versions synchronized".to_string()),
        }
        .into());
    }
    // Zotero's installer rejects any XPI whose manifest omits one of these keys.
    for key in ["update_url", "strict_max_version"] {
        if manifest["applications"]["zotero"][key]
            .as_str()
            .is_none_or(str::is_empty)
        {
            return Err(ZotError::InvalidInput {
                code: "bridge-manifest".to_string(),
                message: format!("Bridge manifest is missing applications.zotero.{key}"),
                hint: Some(
                    "Zotero requires applications.zotero id, update_url, and strict_max_version"
                        .to_string(),
                ),
            }
            .into());
        }
    }
    Ok(())
}

fn validate_xpi(path: &Path) -> Result<()> {
    let file = File::open(path).map_err(|source| ZotError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let mut archive = ZipArchive::new(file).map_err(xpi_error)?;
    let mut names = Vec::new();
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(xpi_error)?;
        names.push(entry.name().to_string());
        let mut sink = Vec::new();
        entry
            .read_to_end(&mut sink)
            .map_err(|source| ZotError::Io {
                path: path.to_path_buf(),
                source,
            })?;
    }
    let expected = PLUGIN_ASSETS
        .iter()
        .map(|(name, _)| (*name).to_string())
        .collect::<Vec<_>>();
    if names != expected {
        return Err(ZotError::InvalidInput {
            code: "bridge-xpi".to_string(),
            message: "Generated XPI contains an unexpected file set".to_string(),
            hint: None,
        }
        .into());
    }
    Ok(())
}

fn xpi_error(error: zip::result::ZipError) -> ZotError {
    ZotError::InvalidInput {
        code: "bridge-xpi".to_string(),
        message: format!("Could not build or validate the Zot Bridge XPI: {error}"),
        hint: Some("Retry `zot bridge setup` in a writable output directory".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bridge_xpi_contains_only_allowlisted_assets() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = write_xpi(dir.path()).expect("write XPI");
        validate_xpi(&path).expect("validate XPI");
        assert!(
            path.file_name()
                .is_some_and(|name| { name.to_string_lossy().contains(env!("CARGO_PKG_VERSION")) })
        );
    }

    #[test]
    fn bridge_manifest_matches_workspace_version_and_id() {
        validate_manifest().expect("manifest");
    }

    #[test]
    fn setup_reuses_existing_pairing_for_same_config_target() {
        let steps = setup_next_steps(true);
        assert!(steps.iter().any(|step| step.contains("bridge status")));
        assert!(steps.iter().all(|step| !step.contains("bridge pair")));
        assert!(steps.iter().all(|step| !step.contains("pairing code")));
    }

    #[test]
    fn setup_requires_pairing_for_an_unconfigured_target() {
        let steps = setup_next_steps(false);
        assert!(steps.iter().any(|step| step.contains("bridge pair")));
        assert!(steps.iter().any(|step| step.contains("pairing code")));
    }
}
