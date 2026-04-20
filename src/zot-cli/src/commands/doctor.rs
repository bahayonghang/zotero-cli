use anyhow::Result;
use zot_core::{AppConfig, redact_secret};
use zot_local::{PdfBackend, PdfiumBackend};
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
    let pdf_available = pdf_backend.availability_hint().is_ok();
    let semantic_status = library::semantic_status(ctx).await.ok();
    let payload = serde_json::json!({
        "config_file": AppConfig::config_file(),
        "data_dir": data_dir,
        "db_exists": db_path.exists(),
        "write_credentials": {
            "configured": ctx.config.write_credentials_configured(),
            "library_id": if ctx.config.zotero.library_id.is_empty() { "(missing)".to_string() } else { ctx.config.zotero.library_id.clone() },
            "api_key": if ctx.config.zotero.api_key.is_empty() { "(missing)".to_string() } else { redact_secret(&ctx.config.zotero.api_key) },
        },
        "embedding": {
            "configured": ctx.config.embedding.is_configured(),
            "url": ctx.config.embedding.url,
            "model": ctx.config.embedding.model,
        },
        "semantic_scholar": {
            "configured": ctx.config.semantic_scholar_key().is_some(),
        },
        "pdf_backend": {
            "available": pdf_available,
        },
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
            if ctx.config.write_credentials_configured() {
                "configured"
            } else {
                "missing"
            }
        );
        println!(
            "PDF backend: {}",
            if pdf_available {
                "available"
            } else {
                "unavailable"
            }
        );
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
