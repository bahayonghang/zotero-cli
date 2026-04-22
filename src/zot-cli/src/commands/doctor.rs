use anyhow::Result;
use zot_core::{AppConfig, redact_secret};
use zot_local::{PdfiumAvailability, PdfiumBackend};
use zot_remote::BetterBibTexClient;

use crate::commands::library;
use crate::context::AppContext;
use crate::format::print_enveloped;

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

pub(crate) async fn handle(ctx: &AppContext) -> Result<()> {
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
    let pdf_available = pdf_status.available;
    let semantic_status = library::semantic_status(ctx).await.ok();
    let payload = serde_json::json!({
        "config_file": AppConfig::config_file(),
        "data_dir": data_dir,
        "db_exists": db_path.exists(),
        "write_credentials": write_credentials_payload(&ctx.config),
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
    if ctx.json {
        print_enveloped(payload, None)?;
    } else {
        println!("{DOCTOR_BANNER}");
        println!("Config: {}", AppConfig::config_file().display());
        println!("Data dir: {}", data_dir.display());
        println!("Database exists: {}", db_path.exists());
        println!(
            "Write credentials: {}",
            write_credentials_label(&ctx.config)
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
    }
    Ok(())
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
