use anyhow::Result;
use zot_core::{AppConfig, ZotError, redact_secret};
use zot_desktop::{
    BridgeHealth, BridgeStatus, ConnectorPing, LocalHttpStatus, ensure_matching_instance_id,
};
use zot_local::{PdfiumAvailability, PdfiumBackend};
use zot_remote::BetterBibTexClient;

use crate::commands::library;
use crate::context::AppContext;
use crate::output::CommandOutput;

const DOCTOR_BANNER: &str = r#"       .-----------------------.
      /  .-----------------.  \
     /  /   _________       \  \
    |  |   /  / Z / /\       |  |
    |  |  /__/___/ /  \      |  |
    |  |  \  \   \/ /\ \     |  |
    |  |   \__\____/  \_\    |  |
    |  |      .-.-.    [#]   |  |
    |  |     (  o )          |  |
    |  |      `-'-'          |  |
     \  \   zot doctor      /  /
      \  '-----------------'  /
       '---------------------'"#;

pub(crate) async fn handle(ctx: &AppContext) -> Result<CommandOutput> {
    let data_dir = zot_core::get_data_dir(&ctx.config);
    let db_path = data_dir.join("zotero.sqlite");
    let pdf_backend = PdfiumBackend;
    let pdf_status = pdf_backend.status();
    let library = ctx.local_library();
    let schema_version = library
        .as_ref()
        .ok()
        .and_then(|library| library.check_schema_compatibility().ok())
        .flatten();
    let libraries = library
        .as_ref()
        .ok()
        .and_then(|library| library.get_libraries().ok())
        .unwrap_or_default();
    let feeds = library
        .as_ref()
        .ok()
        .and_then(|library| library.get_feeds().ok())
        .unwrap_or_default();
    let bbt = BetterBibTexClient::new(ctx.http());
    let bbt_available = bbt.probe().await;
    let desktop = ctx.desktop()?;
    let local_http = desktop.probe_local_http().await;
    let bridge_health = desktop.health().await;
    let bridge_status = if let Ok(health) = &bridge_health
        && ctx.config.zotero.desktop_bridge.is_configured()
    {
        Some(
            match ensure_matching_instance_id(
                &ctx.config.zotero.desktop_bridge.instance_id,
                &health.instance_id,
            ) {
                Ok(()) => match desktop
                    .status(&ctx.config.zotero.desktop_bridge.token)
                    .await
                {
                    Ok(status) => {
                        ensure_matching_instance_id(&health.instance_id, &status.instance_id)
                            .map(|()| status)
                    }
                    Err(error) => Err(error),
                },
                Err(error) => Err(error),
            },
        )
    } else {
        None
    };
    let connector = ctx.connector()?;
    let connector_ping = connector.ping().await;
    let local_sqlite_available = library.is_ok();
    let web_write_configured = ctx.config.write_credentials_configured();
    let pdf_available = pdf_status.available;
    let semantic_status = library::semantic_status(ctx).await.ok();
    let payload = serde_json::json!({
        "config_file": AppConfig::config_file(),
        "data_dir": data_dir,
        "db_exists": db_path.exists(),
        "write_credentials": write_credentials_payload(&ctx.config),
        "selected_write_backend": match ctx.write_backend() {
            zot_core::WriteBackend::Web => "web",
            zot_core::WriteBackend::Desktop => "desktop",
        },
        "capabilities": {
            "local_sqlite_read": local_sqlite_capability(&library),
            "local_http_read": local_http_capability(&local_http),
            "desktop_write": desktop_write_capability(
                ctx.config.zotero.desktop_bridge.is_configured(),
                &bridge_health,
                bridge_status.as_ref(),
            ),
            "connector_write": connector_write_capability(&connector_ping),
            "web_write": web_write_capability(&ctx.config),
        },
        "embedding": {
            "configured": ctx.config.embedding.is_configured(),
            "url": ctx.config.embedding.url,
            "model": ctx.config.embedding.model,
        },
        "semantic_scholar": {
            "configured": ctx.config.semantic_scholar_key().is_some(),
        },
        "pdf_backend": pdf_backend_payload(&pdf_status),
        "better_bibtex": {
            "available": bbt_available,
        },
        "libraries": {
            "count": libraries.len(),
            "feeds_available": !feeds.is_empty(),
        },
        "semantic_index": semantic_status,
        "annotation_support": {
            "pdf_outline": pdf_available,
            "annotation_creation": ctx.config.write_credentials_configured() && pdf_available,
        },
        "schema_version": schema_version,
    });
    let write_creds_label = write_credentials_label(&ctx.config);
    let local_http_label = capability_label(&local_http);
    let desktop_write_label = desktop_capability_label(
        ctx.config.zotero.desktop_bridge.is_configured(),
        &bridge_health,
        bridge_status.as_ref(),
    );
    let connector_write_label = capability_label(&connector_ping);
    CommandOutput::new(ctx, payload, None, move |_| {
        println!("{DOCTOR_BANNER}");
        println!("Config: {}", AppConfig::config_file().display());
        println!("Data dir: {}", data_dir.display());
        println!("Database exists: {}", db_path.exists());
        println!("Write credentials: {write_creds_label}");
        println!(
            "Local SQLite read: {}",
            if local_sqlite_available {
                "available"
            } else {
                "unavailable"
            }
        );
        println!("Local HTTP read: {local_http_label}");
        println!("Desktop write: {desktop_write_label}");
        println!("Connector write: {connector_write_label}");
        println!(
            "Web write: {}",
            if web_write_configured {
                "configured"
            } else {
                "not configured"
            }
        );
        println!("PDF backend: {}", pdf_backend_label(&pdf_status));
        println!(
            "Better BibTeX: {}",
            if bbt_available {
                "available"
            } else {
                "unavailable"
            }
        );
        println!("Libraries discovered: {}", libraries.len());
        println!("Feeds discovered: {}", feeds.len());
        if let Some(status) = semantic_status {
            println!(
                "Semantic index: {} (items={}, chunks={})",
                if status.exists { "present" } else { "missing" },
                status.indexed_items,
                status.indexed_chunks
            );
        }
        if let Some(version) = schema_version {
            println!("Schema version: {version}");
        }
    })
}

fn local_sqlite_capability(
    library: &zot_core::ZotResult<zot_local::LocalLibrary>,
) -> serde_json::Value {
    match library {
        Ok(_) => serde_json::json!({
            "configured": true,
            "available": true,
        }),
        Err(error) => serde_json::json!({
            "configured": true,
            "available": false,
            "error": error.payload(),
        }),
    }
}

fn local_http_capability(status: &zot_core::ZotResult<LocalHttpStatus>) -> serde_json::Value {
    match status {
        Ok(status) => serde_json::json!({
            "configured": true,
            "available": status.available,
            "zotero_version": status.zotero_version,
            "connector_api_version": status.connector_api_version,
        }),
        Err(error) => serde_json::json!({
            "configured": true,
            "available": false,
            "error": error.payload(),
        }),
    }
}

fn desktop_write_capability(
    configured: bool,
    health: &zot_core::ZotResult<BridgeHealth>,
    status: Option<&zot_core::ZotResult<BridgeStatus>>,
) -> serde_json::Value {
    match health {
        Err(error) => serde_json::json!({
            "configured": configured,
            "available": false,
            "error": error.payload(),
        }),
        Ok(health) if !configured => serde_json::json!({
            "configured": false,
            "available": false,
            "installed": true,
            "plugin_version": health.plugin_version,
            "zotero_version": health.zotero_version,
            "protocol_version": health.protocol_version,
            "hint": "Show a pairing code in Zotero and run `zot bridge pair <code>`",
        }),
        Ok(health) => match status {
            Some(Ok(status)) => serde_json::json!({
                "configured": true,
                "available": status.paired,
                "installed": true,
                "plugin_version": health.plugin_version,
                "zotero_version": health.zotero_version,
                "protocol_version": health.protocol_version,
                "capabilities": status.capabilities,
                "libraries": status.libraries,
            }),
            Some(Err(error)) => serde_json::json!({
                "configured": true,
                "available": false,
                "installed": true,
                "plugin_version": health.plugin_version,
                "zotero_version": health.zotero_version,
                "protocol_version": health.protocol_version,
                "error": error.payload(),
            }),
            None => serde_json::json!({
                "configured": true,
                "available": false,
                "installed": true,
                "plugin_version": health.plugin_version,
                "zotero_version": health.zotero_version,
                "protocol_version": health.protocol_version,
                "error": {
                    "code": "bridge-status",
                    "message": "Desktop bridge status was not checked",
                    "hint": "Run `zot bridge status`"
                }
            }),
        },
    }
}

fn connector_write_capability(ping: &zot_core::ZotResult<ConnectorPing>) -> serde_json::Value {
    match ping {
        Ok(_) => serde_json::json!({
            "configured": true,
            "available": true,
            "scope": "import-only",
        }),
        Err(error) => serde_json::json!({
            "configured": true,
            "available": false,
            "scope": "import-only",
            "hint": "Start Zotero to enable local import",
            "error": error.payload(),
        }),
    }
}

fn web_write_capability(config: &AppConfig) -> serde_json::Value {
    let configured = config.write_credentials_configured();
    serde_json::json!({
        "configured": configured,
        "available": configured,
        "checked": "credentials-only",
        "hint": if configured {
            serde_json::Value::Null
        } else {
            serde_json::Value::String("Run `zot config init` or set ZOT_LIBRARY_ID and ZOT_API_KEY for Web API writes".to_string())
        },
    })
}

fn capability_label<T>(result: &Result<T, ZotError>) -> &'static str {
    if result.is_ok() {
        "available"
    } else {
        "unavailable"
    }
}

fn desktop_capability_label(
    configured: bool,
    health: &zot_core::ZotResult<BridgeHealth>,
    status: Option<&zot_core::ZotResult<BridgeStatus>>,
) -> &'static str {
    if health.is_err() {
        "unavailable"
    } else if !configured {
        "plugin installed; not paired"
    } else if status.is_some_and(|value| value.as_ref().is_ok_and(|status| status.paired)) {
        "available"
    } else {
        "configured; unavailable"
    }
}

fn write_credentials_payload(config: &AppConfig) -> serde_json::Value {
    serde_json::json!({
        "configured": config.write_credentials_configured(),
        "library_id": if config.zotero.library_id.is_empty() { "(missing)".to_string() } else { config.zotero.library_id.clone() },
        "api_key": if config.zotero.api_key.is_empty() { "(missing)".to_string() } else { redact_secret(&config.zotero.api_key) },
        "required_for_local_read": false,
        "required_for_remote_write": true,
        "note": "Optional for local reads; required only for Zotero Web API writes.",
    })
}

fn write_credentials_label(config: &AppConfig) -> &'static str {
    if config.write_credentials_configured() {
        "configured (used only for Zotero Web API writes)"
    } else {
        "missing (optional for local reads; only needed for Zotero Web API writes)"
    }
}

fn pdf_backend_payload(status: &PdfiumAvailability) -> serde_json::Value {
    serde_json::json!({
        "available": status.available,
        "auto_download_supported": status.auto_download_supported,
        "cached": status.cached,
        "note": status.note,
    })
}

fn pdf_backend_label(status: &PdfiumAvailability) -> &'static str {
    if status.available {
        "available"
    } else if status.auto_download_supported {
        "unavailable (auto-download on first local PDF read)"
    } else {
        "unavailable (set ZOT_PDFIUM_LIB_PATH or PDFIUM_LIB_PATH)"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_credentials_payload_marks_local_reads_as_optional() {
        /*
         * ========================================================================
         * 步骤1：校验写凭据说明
         * ========================================================================
         * 目标：
         * 1) 保证 doctor 明确写凭据不影响本地读取
         * 2) 保证远端写入依赖说明稳定输出
         */
        eprintln!("开始校验 doctor 写凭据说明...");

        // 1.1 准备缺省配置
        let config = AppConfig::default();

        // 1.2 校验 JSON 字段
        let payload = write_credentials_payload(&config);
        assert_eq!(payload["required_for_local_read"], false);
        assert_eq!(payload["required_for_remote_write"], true);
        assert_eq!(
            payload["note"],
            "Optional for local reads; required only for Zotero Web API writes."
        );

        // 1.3 校验 CLI 文案
        assert_eq!(
            write_credentials_label(&config),
            "missing (optional for local reads; only needed for Zotero Web API writes)"
        );

        eprintln!("doctor 写凭据说明校验完成");
    }

    #[test]
    fn desktop_capability_reports_installed_but_unpaired() {
        let health = Ok(BridgeHealth {
            instance_id: "test-instance".to_string(),
            plugin_version: "0.6.0".to_string(),
            zotero_version: "9.0.6".to_string(),
            protocol_version: 1,
            capabilities: vec!["status".to_string()],
        });
        let payload = desktop_write_capability(false, &health, None);
        assert_eq!(payload["configured"], false);
        assert_eq!(payload["available"], false);
        assert_eq!(payload["installed"], true);
        assert_eq!(
            desktop_capability_label(false, &health, None),
            "plugin installed; not paired"
        );
    }

    #[test]
    fn web_capability_is_credentials_only() {
        let payload = web_write_capability(&AppConfig::default());
        assert_eq!(payload["configured"], false);
        assert_eq!(payload["available"], false);
        assert_eq!(payload["checked"], "credentials-only");
    }

    #[test]
    fn connector_capability_reports_scope_and_availability() {
        let available = Ok(ConnectorPing {
            available: true,
            zotero_version: Some("7.0.35".to_string()),
        });
        let payload = connector_write_capability(&available);
        assert_eq!(payload["configured"], true);
        assert_eq!(payload["available"], true);
        assert_eq!(payload["scope"], "import-only");
        assert!(payload.get("hint").is_none());
        assert_eq!(capability_label(&available), "available");

        let unavailable: zot_core::ZotResult<ConnectorPing> = Err(ZotError::Connector {
            code: "connector-unreachable".to_string(),
            message: "Could not connect to the Zotero connector server".to_string(),
            hint: Some("Start Zotero, then retry".to_string()),
            status: None,
        });
        let payload = connector_write_capability(&unavailable);
        assert_eq!(payload["configured"], true);
        assert_eq!(payload["available"], false);
        assert_eq!(payload["scope"], "import-only");
        assert_eq!(payload["hint"], "Start Zotero to enable local import");
        assert!(
            !payload["hint"]
                .as_str()
                .unwrap_or_default()
                .contains("bridge")
        );
        assert_eq!(capability_label(&unavailable), "unavailable");
    }

    #[test]
    fn pdf_backend_label_reports_auto_download_support() {
        /*
         * ========================================================================
         * 步骤2：校验 PDF backend 文案
         * ========================================================================
         * 目标：
         * 1) 保证自动下载能力会反映到 doctor 输出
         * 2) 保证手工配置 hint 保持稳定
         */
        eprintln!("开始校验 doctor PDF backend 文案...");

        // 2.1 自动下载可用但当前未绑定
        let auto_download = PdfiumAvailability {
            available: false,
            cached: false,
            auto_download_supported: true,
            note: "Pdfium will auto-download on the first local PDF read.".to_string(),
        };
        assert_eq!(
            pdf_backend_label(&auto_download),
            "unavailable (auto-download on first local PDF read)"
        );

        // 2.2 当前已可用
        let available = PdfiumAvailability {
            available: true,
            cached: true,
            auto_download_supported: true,
            note: "Pdfium is ready for local PDF reads.".to_string(),
        };
        assert_eq!(pdf_backend_label(&available), "available");

        // 2.3 当前不可用且不支持自动下载
        let manual_only = PdfiumAvailability {
            available: false,
            cached: false,
            auto_download_supported: false,
            note: "Set ZOT_PDFIUM_LIB_PATH or PDFIUM_LIB_PATH to a compatible Pdfium library."
                .to_string(),
        };
        assert_eq!(
            pdf_backend_label(&manual_only),
            "unavailable (set ZOT_PDFIUM_LIB_PATH or PDFIUM_LIB_PATH)"
        );

        eprintln!("doctor PDF backend 文案校验完成");
    }
}
