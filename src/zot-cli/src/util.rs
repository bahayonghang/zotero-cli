use anyhow::Result;
use zot_core::{EmbeddingConfig, PdfOutlineEntry, ZotError, ZotResult};
use zot_remote::{EmbeddingClient, HttpRuntime, PublicationStatus};

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
    use zot_core::ZotError;

    use super::parse_page_range;

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
}
