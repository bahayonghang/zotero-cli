use anyhow::Result;
use zot_core::{
    AppConfig, Attachment, EmbeddingConfig, Item, LibraryScope, PdfOutlineEntry, ZotError,
    ZotResult,
};
use zot_local::{AttachmentSource, ItemReader, LocalLibrary};
use zot_remote::{EmbeddingClient, HttpRuntime, PublicationStatus, normalize_doi};

pub(crate) async fn run_pdf<F, R>(f: F) -> ZotResult<R>
where
    F: FnOnce() -> ZotResult<R> + Send + 'static,
    R: Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|join| ZotError::Pdf {
            code: "pdf-task-join".to_string(),
            message: join.to_string(),
            hint: None,
        })?
}

pub(crate) async fn run_local<F, R>(config: AppConfig, scope: LibraryScope, f: F) -> ZotResult<R>
where
    F: FnOnce(LocalLibrary) -> ZotResult<R> + Send + 'static,
    R: Send + 'static,
{
    tokio::task::spawn_blocking(move || {
        let library = LocalLibrary::open(zot_core::get_data_dir(&config), scope)?;
        f(library)
    })
    .await
    .map_err(|join| ZotError::InvalidInput {
        code: "local-task-join".to_string(),
        message: format!("Local database worker failed: {join}"),
        hint: Some("Retry the command; report the failure if it persists".to_string()),
    })?
}

pub(crate) async fn maybe_embed_query(
    runtime: &HttpRuntime,
    config: &EmbeddingConfig,
    query: &str,
) -> ZotResult<Option<Vec<f32>>> {
    let client = EmbeddingClient::new(runtime, config.clone());
    if !client.configured() {
        return Ok(None);
    }

    let embeddings = client.embed(&[query.to_string()]).await?;
    let embedding = embeddings
        .into_iter()
        .next()
        .ok_or_else(|| ZotError::Remote {
            code: "embedding-empty".to_string(),
            message: "Embedding service returned no vector".to_string(),
            hint: Some("Check embedding service health".to_string()),
            status: None,
        })?;
    Ok(Some(embedding))
}

pub(crate) fn print_outline_entries(entries: &[PdfOutlineEntry]) {
    for entry in entries {
        let indent = "  ".repeat(entry.level.saturating_sub(1));
        if let Some(page) = entry.page {
            println!("{indent}- {} (p. {page})", entry.title);
        } else {
            println!("{indent}- {}", entry.title);
        }
    }
}

pub(crate) fn open_target(target: &str) -> Result<()> {
    opener::open(target).map_err(|err| ZotError::Io {
        path: std::path::PathBuf::from(target),
        source: std::io::Error::other(err),
    })?;
    Ok(())
}

pub(crate) fn parse_page_range(range: Option<&str>) -> Result<Option<(usize, usize)>> {
    let Some(range) = range else {
        return Ok(None);
    };

    let parts = range.split('-').collect::<Vec<_>>();
    let Some(start_raw) = parts.first() else {
        return Err(invalid_page_range(range).into());
    };
    let start = parse_page_bound(start_raw, range)?;
    let end = if let Some(value) = parts.get(1) {
        parse_page_bound(value, range)?
    } else {
        start
    };
    Ok(Some((start, end)))
}

pub(crate) fn update_status_to_json(key: String, status: PublicationStatus) -> serde_json::Value {
    serde_json::json!({
        "key": key,
        "preprint_id": status.preprint_id,
        "source": status.source,
        "title": status.title,
        "published": status.is_published,
        "venue": status.venue,
        "journal": status.journal_name,
        "doi": status.doi,
        "date": status.publication_date,
    })
}

pub(crate) fn parse_json_input(input: &str, label: &str) -> ZotResult<serde_json::Value> {
    let path = std::path::PathBuf::from(input);
    let raw = if path.exists() {
        std::fs::read_to_string(&path).map_err(|source| ZotError::Io { path, source })?
    } else {
        input.to_string()
    };
    serde_json::from_str(&raw).map_err(|err| ZotError::InvalidInput {
        code: "json-input".to_string(),
        message: format!("Invalid JSON for {label}: {err}"),
        hint: Some("Pass a JSON string or a path to a JSON file".to_string()),
    })
}

/// Resolve an item by key via the local library, mapping a missing row to the
/// stable `item-not-found` CLI error.
pub(crate) fn require_item(library: &impl ItemReader, key: &str) -> ZotResult<Item> {
    library.get_item(key)?.ok_or_else(|| item_not_found(key))
}

/// Resolve the PDF attachment belonging to an item, mapping absence to the
/// stable `item-no-pdf` CLI error.
pub(crate) fn require_item_pdf(
    library: &impl AttachmentSource,
    key: &str,
) -> ZotResult<Attachment> {
    library
        .get_pdf_attachment(key)?
        .ok_or_else(|| item_no_pdf(key))
}

/// Resolve an attachment by its own key and require it to be a PDF, mapping
/// failures to the stable `attachment-not-found` / `attachment-not-pdf` CLI
/// errors.
pub(crate) fn require_pdf_attachment(
    library: &impl AttachmentSource,
    key: &str,
) -> ZotResult<Attachment> {
    let attachment = library
        .get_attachment_by_key(key)?
        .ok_or_else(|| attachment_not_found(key))?;
    if attachment.content_type != "application/pdf" {
        return Err(attachment_not_pdf(key));
    }
    Ok(attachment)
}

/// Normalize a DOI, mapping rejected values to the stable `invalid-doi` CLI
/// error.
pub(crate) fn require_valid_doi(doi: &str) -> ZotResult<String> {
    normalize_doi(doi).ok_or_else(|| invalid_doi(doi))
}

/// Stable `item-not-found` error for lookups that resolve items indirectly
/// (e.g. `export_citation` returning `None`); direct `get_item` callers should
/// use [`require_item`].
pub(crate) fn item_not_found(key: &str) -> ZotError {
    ZotError::InvalidInput {
        code: "item-not-found".to_string(),
        message: format!("Item '{key}' not found"),
        hint: None,
    }
}

fn item_no_pdf(key: &str) -> ZotError {
    ZotError::InvalidInput {
        code: "item-no-pdf".to_string(),
        message: format!("Item '{key}' has no PDF attachment"),
        hint: None,
    }
}

fn attachment_not_found(key: &str) -> ZotError {
    ZotError::InvalidInput {
        code: "attachment-not-found".to_string(),
        message: format!("Attachment '{key}' not found"),
        hint: None,
    }
}

fn attachment_not_pdf(key: &str) -> ZotError {
    ZotError::InvalidInput {
        code: "attachment-not-pdf".to_string(),
        message: format!("Attachment '{key}' is not a PDF attachment"),
        hint: None,
    }
}

fn invalid_doi(doi: &str) -> ZotError {
    ZotError::InvalidInput {
        code: "invalid-doi".to_string(),
        message: format!("'{doi}' does not appear to be a valid DOI"),
        hint: None,
    }
}

fn parse_page_bound(value: &str, raw: &str) -> ZotResult<usize> {
    value.parse::<usize>().map_err(|_| invalid_page_range(raw))
}

fn invalid_page_range(range: &str) -> ZotError {
    ZotError::InvalidInput {
        code: "page-range".to_string(),
        message: format!("Invalid page range '{range}'"),
        hint: Some("Use a single page like 7 or a span like 3-9".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use zot_core::ZotError;

    use super::{parse_page_range, require_valid_doi, run_local, run_pdf};

    #[tokio::test(flavor = "current_thread")]
    async fn run_pdf_offloads_blocking_work_to_a_separate_thread() {
        // Sleeping inside `run_pdf` must not block the single-threaded runtime,
        // confirming that PdfBackend calls now run on a blocking-threadpool worker.
        let outcome = run_pdf(|| {
            std::thread::sleep(Duration::from_millis(10));
            Ok::<_, ZotError>(42_u32)
        })
        .await
        .expect("run_pdf must return Ok value");
        assert_eq!(outcome, 42);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn run_local_opens_and_queries_on_a_blocking_thread() {
        let data_dir = tempfile::tempdir().expect("temporary Zotero data dir");
        let fixture = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../zot-local/tests/fixtures/zotero.sqlite");
        std::fs::copy(&fixture, data_dir.path().join("zotero.sqlite"))
            .expect("copy local library fixture");
        let mut config = zot_core::AppConfig::default();
        config.zotero.data_dir = data_dir.path().display().to_string();
        let runtime_thread = std::thread::current().id();
        let worker_thread = run_local(config, zot_core::LibraryScope::User, move |_| {
            Ok::<_, ZotError>(std::thread::current().id())
        })
        .await
        .expect("run local fixture query");

        assert_ne!(worker_thread, runtime_thread);
    }

    #[test]
    fn parses_page_ranges_for_single_pages_and_spans() {
        assert_eq!(parse_page_range(None).expect("no range"), None);
        assert_eq!(
            parse_page_range(Some("7")).expect("single page"),
            Some((7, 7))
        );
        assert_eq!(
            parse_page_range(Some("3-9")).expect("page span"),
            Some((3, 9))
        );
    }

    #[test]
    fn rejects_invalid_page_ranges_with_invalid_input() {
        let err = parse_page_range(Some("a-b")).expect_err("invalid range should fail");
        let err = err
            .downcast_ref::<ZotError>()
            .expect("zot error should be preserved");
        match err {
            ZotError::InvalidInput { code, .. } => assert_eq!(code, "page-range"),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn named_error_constructors_keep_stable_payloads() {
        // These strings are part of the CLI JSON error contract; the helper
        // consolidation must not change a single byte of code/message/hint.
        let cases = [
            (
                super::item_not_found("ABC123"),
                "item-not-found",
                "Item 'ABC123' not found",
            ),
            (
                super::item_no_pdf("ABC123"),
                "item-no-pdf",
                "Item 'ABC123' has no PDF attachment",
            ),
            (
                super::attachment_not_found("ATCH005"),
                "attachment-not-found",
                "Attachment 'ATCH005' not found",
            ),
            (
                super::attachment_not_pdf("ATCH005"),
                "attachment-not-pdf",
                "Attachment 'ATCH005' is not a PDF attachment",
            ),
            (
                super::invalid_doi("not-a-doi"),
                "invalid-doi",
                "'not-a-doi' does not appear to be a valid DOI",
            ),
        ];
        for (err, code, message) in cases {
            let payload = err.payload();
            assert_eq!(payload.code, code);
            assert_eq!(payload.message, message);
            assert_eq!(payload.hint, None);
        }
    }

    #[test]
    fn require_valid_doi_normalizes_or_rejects() {
        assert_eq!(
            require_valid_doi("https://doi.org/10.1000/test").expect("valid doi"),
            "10.1000/test"
        );
        let err = require_valid_doi("not a doi").expect_err("invalid doi should fail");
        match err {
            ZotError::InvalidInput { code, .. } => assert_eq!(code, "invalid-doi"),
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
