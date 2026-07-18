use std::path::Path;
use std::sync::LazyLock;

use anyhow::Result;
use regex::Regex;
use zot_core::{ZotError, ZotResult};
use zot_desktop::ConnectorClient;

use crate::cli::{ItemImportArgs, ItemImportFormatArg};
use crate::context::AppContext;
use crate::output::CommandOutput;

static BIBTEX_ENTRY_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"@\w+\s*\{").expect("valid bibtex entry regex"));
static RIS_ENTRY_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^TY {2}- ").expect("valid ris entry regex"));

pub(crate) async fn handle_import(ctx: &AppContext, args: ItemImportArgs) -> Result<CommandOutput> {
    let client = ctx.connector()?;
    let payload = build_import_payload(&client, &args).await?;
    CommandOutput::new(ctx, payload, None, |payload| {
        println!(
            "{}",
            serde_json::to_string_pretty(payload).expect("serialize import result")
        );
    })
}

/// Core orchestration, kept separate from `handle_import` so tests can drive
/// it against a fake `ConnectorClient` without building an `AppContext`.
async fn build_import_payload(
    client: &ConnectorClient,
    args: &ItemImportArgs,
) -> Result<serde_json::Value> {
    client.ping().await?;
    let target = client.selected_target().await?;
    let editable = target.is_writable();

    if args.confirm && !editable {
        return Err(readonly_target_error().into());
    }

    let text = read_import_text(args)?;
    let format = resolve_format(args.format, args.file.as_deref(), &text)?;
    let entries = count_entries(format, &text);

    if !args.confirm {
        return Ok(serde_json::json!({
            "target": target,
            "editable": editable,
            "entries": entries,
            "format": format_label(format),
            "confirmed": false,
        }));
    }

    let session = format!("zot-{}", uuid::Uuid::new_v4());
    let result = client.import(&session, &text).await?;
    Ok(serde_json::json!({
        "session": result.session,
        "target": target,
        "editable": editable,
        "entries": entries,
        "format": format_label(format),
        "status": result.status,
    }))
}

fn read_import_text(args: &ItemImportArgs) -> ZotResult<String> {
    if let Some(file) = args.file.as_deref() {
        return std::fs::read_to_string(file).map_err(|source| ZotError::Io {
            path: file.to_path_buf(),
            source,
        });
    }
    args.text.clone().ok_or_else(|| ZotError::InvalidInput {
        code: "item-import-source".to_string(),
        message: "Provide --file or --text".to_string(),
        hint: None,
    })
}

fn resolve_format(
    explicit: Option<ItemImportFormatArg>,
    file: Option<&Path>,
    text: &str,
) -> ZotResult<ItemImportFormatArg> {
    if let Some(format) = explicit {
        return Ok(format);
    }
    if let Some(format) = file.and_then(format_from_extension) {
        return Ok(format);
    }
    sniff_format(text).ok_or_else(import_format_error)
}

fn format_from_extension(file: &Path) -> Option<ItemImportFormatArg> {
    let ext = file.extension()?.to_str()?;
    if ext.eq_ignore_ascii_case("bib") {
        Some(ItemImportFormatArg::Bibtex)
    } else if ext.eq_ignore_ascii_case("ris") {
        Some(ItemImportFormatArg::Ris)
    } else {
        None
    }
}

fn sniff_format(text: &str) -> Option<ItemImportFormatArg> {
    if BIBTEX_ENTRY_RE.is_match(text) {
        Some(ItemImportFormatArg::Bibtex)
    } else if RIS_ENTRY_RE.is_match(text) {
        Some(ItemImportFormatArg::Ris)
    } else {
        None
    }
}

fn count_entries(format: ItemImportFormatArg, text: &str) -> usize {
    match format {
        ItemImportFormatArg::Bibtex => BIBTEX_ENTRY_RE.find_iter(text).count(),
        ItemImportFormatArg::Ris => RIS_ENTRY_RE.find_iter(text).count(),
    }
}

fn format_label(format: ItemImportFormatArg) -> &'static str {
    match format {
        ItemImportFormatArg::Bibtex => "bibtex",
        ItemImportFormatArg::Ris => "ris",
    }
}

fn import_format_error() -> ZotError {
    ZotError::Connector {
        code: "connector-import-format".to_string(),
        message: "Could not determine import format from --format, file extension, or content"
            .to_string(),
        hint: Some("Pass --format bibtex|ris or use a .bib/.ris file".to_string()),
        status: None,
    }
}

fn readonly_target_error() -> ZotError {
    ZotError::Connector {
        code: "connector-target-readonly".to_string(),
        message: "The selected Zotero target is not writable".to_string(),
        hint: Some("Select a writable collection in Zotero, then retry".to_string()),
        status: None,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::thread::JoinHandle;
    use std::time::Duration;

    use tiny_http::{Response, Server};

    use super::*;

    fn args(file: Option<&str>, text: Option<&str>, confirm: bool) -> ItemImportArgs {
        ItemImportArgs {
            file: file.map(PathBuf::from),
            text: text.map(ToOwned::to_owned),
            format: None,
            confirm,
        }
    }

    #[test]
    fn resolve_format_prefers_explicit_over_extension_and_content() {
        assert_eq!(
            resolve_format(
                Some(ItemImportFormatArg::Ris),
                Some(Path::new("refs.bib")),
                "@article{key1,\n}\n"
            )
            .expect("explicit format wins"),
            ItemImportFormatArg::Ris
        );
    }

    #[test]
    fn resolve_format_falls_back_to_extension_case_insensitively() {
        assert_eq!(
            resolve_format(None, Some(Path::new("refs.BIB")), "no markers here")
                .expect("extension format"),
            ItemImportFormatArg::Bibtex
        );
        assert_eq!(
            resolve_format(None, Some(Path::new("refs.ris")), "no markers here")
                .expect("extension format"),
            ItemImportFormatArg::Ris
        );
    }

    #[test]
    fn resolve_format_sniffs_content_when_no_format_or_known_extension() {
        assert_eq!(
            resolve_format(None, None, "@article{key1,\n  title={T},\n}\n")
                .expect("sniffed bibtex"),
            ItemImportFormatArg::Bibtex
        );
        assert_eq!(
            resolve_format(None, None, "TY  - JOUR\nTI  - Title\nER  - \n").expect("sniffed ris"),
            ItemImportFormatArg::Ris
        );
    }

    #[test]
    fn resolve_format_errors_when_undetectable() {
        let err = resolve_format(None, Some(Path::new("refs.txt")), "plain text, no markers")
            .expect_err("undetectable format");
        match err {
            ZotError::Connector { code, .. } => assert_eq!(code, "connector-import-format"),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn count_entries_counts_bibtex_and_ris_records() {
        let bibtex = "@article{key1,\n  title={A},\n}\n@book{key2,\n  title={B},\n}\n";
        assert_eq!(count_entries(ItemImportFormatArg::Bibtex, bibtex), 2);

        let ris = "TY  - JOUR\nER  - \nTY  - BOOK\nER  - \n";
        assert_eq!(count_entries(ItemImportFormatArg::Ris, ris), 2);
    }

    #[test]
    fn read_import_text_prefers_file_and_rejects_neither() {
        let missing = args(None, None, false);
        let err = read_import_text(&missing).expect_err("neither file nor text");
        match err {
            ZotError::InvalidInput { code, .. } => assert_eq!(code, "item-import-source"),
            other => panic!("unexpected error: {other:?}"),
        }

        let text_only = args(None, Some("@article{key1,\n}\n"), false);
        assert_eq!(
            read_import_text(&text_only).expect("text source"),
            "@article{key1,\n}\n"
        );
    }

    struct Captured {
        path: String,
    }

    fn spawn_connector(responses: Vec<(u16, String)>) -> (String, JoinHandle<Vec<Captured>>) {
        let server = Server::http(("127.0.0.1", 0)).expect("bind fake connector server");
        let port = server.server_addr().to_ip().expect("fake server IP").port();
        let base = format!("http://127.0.0.1:{port}/");
        let handle = std::thread::spawn(move || {
            let mut captured = Vec::new();
            for (status, body) in responses {
                let mut request = server.recv().expect("receive request");
                let mut drained = String::new();
                let _ = request.as_reader().read_to_string(&mut drained);
                captured.push(Captured {
                    path: request.url().to_string(),
                });
                let _ = request.respond(Response::from_string(body).with_status_code(status));
            }
            captured
        });
        (base, handle)
    }

    fn ping_ok() -> (u16, String) {
        (200, String::new())
    }

    fn selected_target_body(editable: bool, library_editable: bool) -> (u16, String) {
        (
            200,
            serde_json::json!({
                "id": 1,
                "name": "Reading List",
                "editable": editable,
                "libraryEditable": library_editable,
            })
            .to_string(),
        )
    }

    #[tokio::test]
    async fn dry_run_never_sends_an_import_request() {
        // Only ping + getSelectedCollection are scripted; a third `recv()`
        // for `/connector/import` would hang the server thread and fail the
        // `join` below, proving no import request was sent.
        let (base, handle) = spawn_connector(vec![ping_ok(), selected_target_body(true, true)]);
        let client = ConnectorClient::with_base_url_for_tests(&base, Duration::from_secs(2))
            .expect("client");
        let args = args(None, Some("@article{key1,\n}\n"), false);

        let payload = build_import_payload(&client, &args)
            .await
            .expect("dry run payload");
        assert_eq!(payload["confirmed"], false);
        assert_eq!(payload["entries"], 1);
        assert_eq!(payload["format"], "bibtex");
        assert_eq!(payload["editable"], true);
        assert!(payload.get("session").is_none());

        let captured = handle.join().expect("join server");
        assert_eq!(captured.len(), 2);
        assert_eq!(captured[1].path, "/connector/getSelectedCollection");
    }

    #[tokio::test]
    async fn readonly_target_is_rejected_before_confirm_ever_sends_import() {
        // Same two-response script as the dry-run test: if the readonly gate
        // failed to stop the confirm branch, the code would call `import()`
        // and hang waiting for a third scripted response.
        let (base, handle) = spawn_connector(vec![ping_ok(), selected_target_body(false, true)]);
        let client = ConnectorClient::with_base_url_for_tests(&base, Duration::from_secs(2))
            .expect("client");
        let args = args(None, Some("@article{key1,\n}\n"), true);

        let err = build_import_payload(&client, &args)
            .await
            .expect_err("readonly target must be rejected");
        let err = err.downcast_ref::<ZotError>().expect("zot error");
        match err {
            ZotError::Connector { code, .. } => assert_eq!(code, "connector-target-readonly"),
            other => panic!("unexpected error: {other:?}"),
        }

        let captured = handle.join().expect("join server");
        assert_eq!(captured.len(), 2);
        assert!(
            captured
                .iter()
                .all(|entry| entry.path != "/connector/import")
        );
    }

    #[tokio::test]
    async fn confirmed_writable_target_sends_import_and_reports_session() {
        let (base, handle) = spawn_connector(vec![
            ping_ok(),
            selected_target_body(true, true),
            (200, serde_json::json!([{ "key": "ABCD1234" }]).to_string()),
        ]);
        let client = ConnectorClient::with_base_url_for_tests(&base, Duration::from_secs(2))
            .expect("client");
        let args = args(None, Some("@article{key1,\n}\n"), true);

        let payload = build_import_payload(&client, &args)
            .await
            .expect("confirmed import payload");
        assert_eq!(payload["status"], 200);
        assert_eq!(payload["entries"], 1);
        assert!(
            payload["session"]
                .as_str()
                .expect("session string")
                .starts_with("zot-")
        );

        let captured = handle.join().expect("join server");
        assert_eq!(captured.len(), 3);
        assert!(
            captured[2]
                .path
                .starts_with("/connector/import?session=zot-")
        );
    }
}
