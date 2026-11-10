use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use clap::Parser;
use serde_json::{Value, json};
use tiny_http::{Header, Response, Server};
use zot_core::{AppConfig, LibraryScope};
use zot_desktop::{LocalApiClient, LocalCollection, LocalCollectionSnapshot};
use zot_local::PdfiumBackend;
use zot_remote::HttpRuntime;

use super::*;
use crate::cli::{Cli, CollectionCommand, Commands, WriteBackendArg};
use crate::context::AppContext;
use crate::output::CommandOutput;

fn collection(
    key: &str,
    name: &str,
    parent: Option<&str>,
    version: u64,
    deleted: bool,
) -> LocalCollection {
    LocalCollection {
        key: key.to_string(),
        version,
        name: name.to_string(),
        parent: parent.map(ToOwned::to_owned),
        deleted,
    }
}

fn snapshot(version: u64, collections: Vec<LocalCollection>) -> LocalCollectionSnapshot {
    LocalCollectionSnapshot {
        library_version: version,
        collections,
    }
}

fn tree() -> LocalCollectionSnapshot {
    snapshot(
        99,
        vec![
            collection("COLL0001", "Root", None, 4, false),
            collection("ZZZZZZZZ", "Late", Some("COLL0001"), 1, true),
            collection("AAAAAAAA", "Early", Some("COLL0001"), 2, false),
            collection("MMMMMMMM", "Mid", Some("AAAAAAAA"), 5, false),
            collection("OTHERXXX", "Sibling", None, 8, false),
            collection("OTHERYYY", "Cycle A", Some("OTHERZZZ"), 1, false),
            collection("OTHERZZZ", "Cycle B", Some("OTHERYYY"), 1, false),
        ],
    )
}

fn args(confirm: bool, version: u64, hash: &str) -> CollectionDeleteArgs {
    CollectionDeleteArgs {
        key: "COLL0001".to_string(),
        write: LocalDeleteArgs {
            write_backend: WriteBackendArg::Local,
            confirm,
            expected_server_id: confirm.then(|| "server".to_string()),
            expected_version: confirm.then_some(version),
            expected_descendants_sha256: confirm.then(|| hash.to_string()),
        },
    }
}

fn object(key: &str, version: u64, name: &str, parent: Value, deleted: bool) -> Value {
    let mut data = json!({
        "key": key,
        "version": version,
        "name": name,
        "parentCollection": parent,
    });
    if deleted {
        data["deleted"] = json!(1);
    }
    json!({"key": key, "version": version, "data": data})
}

fn list_body() -> Value {
    json!([
        object("COLL0001", 4, "Root", Value::Bool(false), false),
        object("ZZZZZZZZ", 1, "Late", json!("COLL0001"), true),
        object("AAAAAAAA", 2, "Early", json!("COLL0001"), false),
        object("MMMMMMMM", 5, "Mid", json!("AAAAAAAA"), false),
        object("OTHERXXX", 8, "Sibling", Value::Bool(false), false),
    ])
}

fn sorted_hash() -> String {
    sha256_hex(b"AAAAAAAA\nMMMMMMMM\nZZZZZZZZ")
}

#[derive(Debug)]
struct Captured {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
    body: Value,
}

fn server(
    responses: Vec<(u16, Value, Option<u64>)>,
) -> (LocalApiClient, JoinHandle<Vec<Captured>>) {
    let server = Server::http("127.0.0.1:0").expect("server");
    let client = LocalApiClient::with_base_url_for_tests(
        &format!("http://{}/", server.server_addr()),
        Duration::from_secs(1),
    )
    .expect("client");
    let handle = std::thread::spawn(move || {
        let mut captured = Vec::new();
        for (status, body, library_version) in responses {
            let request = server
                .recv_timeout(Duration::from_secs(3))
                .expect("recv")
                .expect("request");
            captured.push(capture(request, body, status, library_version));
        }
        captured
    });
    (client, handle)
}

fn capture(
    mut request: tiny_http::Request,
    body: Value,
    status: u16,
    library_version: Option<u64>,
) -> Captured {
    let mut text = String::new();
    request.as_reader().read_to_string(&mut text).expect("body");
    let captured = Captured {
        method: request.method().to_string(),
        path: request.url().to_string(),
        headers: request
            .headers()
            .iter()
            .map(|header| {
                (
                    header.field.to_string().to_ascii_lowercase(),
                    header.value.to_string(),
                )
            })
            .collect(),
        body: serde_json::from_str(&text).unwrap_or(Value::Null),
    };
    let mut response = Response::from_string(body.to_string()).with_status_code(status);
    for (name, value) in [
        ("Zotero-Server-ID", "server"),
        ("Zotero-API-Version", "3"),
        ("X-Zotero-Version", "10.0.6"),
    ] {
        response.add_header(Header::from_bytes(name, value).expect("header"));
    }
    if let Some(version) = library_version {
        response.add_header(
            Header::from_bytes("Last-Modified-Version", version.to_string()).expect("version"),
        );
        if let Some(entries) = body.as_array() {
            response.add_header(
                Header::from_bytes("Total-Results", entries.len().to_string()).expect("total"),
            );
        }
    }
    request.respond(response).expect("respond");
    captured
}

fn deletes(requests: &[Captured]) -> usize {
    requests
        .iter()
        .filter(|request| request.method == "DELETE")
        .count()
}

fn object_gets(requests: &[Captured], key: &str) -> usize {
    let suffix = format!("/collections/{key}");
    requests
        .iter()
        .filter(|request| request.method == "GET" && request.path.ends_with(&suffix))
        .count()
}

fn app_ctx() -> AppContext {
    AppContext {
        json: true,
        profile: None,
        scope: LibraryScope::User,
        config: AppConfig::default(),
        http: Arc::new(HttpRuntime::default()),
        pdf: Arc::new(PdfiumBackend),
    }
}

#[test]
fn clap_accepts_object_version_zero_and_web_rejects_confirm_flags() {
    let cli = Cli::try_parse_from(["zot", "collection", "delete", "COLL0001"]).expect("web");
    let Commands::Collection {
        command: CollectionCommand::Delete(parsed),
    } = cli.command
    else {
        panic!("delete")
    };
    assert_eq!(parsed.write.write_backend, WriteBackendArg::Web);
    assert!(validate_web_options(&parsed.write).is_ok());

    let cli = Cli::try_parse_from([
        "zot",
        "collection",
        "delete",
        "COLL0001",
        "--write-backend",
        "local",
        "--confirm",
        "--expected-server-id",
        "server",
        "--expected-version",
        "0",
        "--expected-descendants-sha256",
        EMPTY_DESCENDANTS_SHA256,
    ])
    .expect("local confirm");
    let Commands::Collection {
        command: CollectionCommand::Delete(parsed),
    } = cli.command
    else {
        panic!("delete")
    };
    assert_eq!(parsed.write.expected_version, Some(0));
    assert!(validate_web_options(&parsed.write).is_err());
    assert!(
        Cli::try_parse_from([
            "zot",
            "collection",
            "delete",
            "COLL0001",
            "--write-backend",
            "local",
            "--confirm",
            "--expected-server-id",
            "server",
            "--expected-version",
            "0",
        ])
        .is_err()
    );
    assert_eq!(
        validate_object_key("coll0001")
            .expect_err("lowercase")
            .payload()
            .code,
        "local-api-key"
    );
    assert_eq!(
        require_version(None, 0)
            .expect_err("missing")
            .payload()
            .code,
        "local-api-conflict"
    );
    assert!(require_version(Some(0), 0).is_ok());
}

#[test]
fn descendant_hash_sorts_keys_and_ignores_names_and_unrelated_cycles() {
    let descendants = descendant_tree(&tree(), "COLL0001").expect("tree").1;
    assert_eq!(
        descendants
            .iter()
            .map(|item| item.key.as_str())
            .collect::<Vec<_>>(),
        ["AAAAAAAA", "MMMMMMMM", "ZZZZZZZZ"]
    );
    assert!(descendants[2].deleted);
    assert_eq!(descendants_sha256(&descendants), sorted_hash());
    assert_eq!(sha256_hex(b""), EMPTY_DESCENDANTS_SHA256);
    assert_ne!(
        descendants_sha256(&descendants),
        sha256_hex(b"ZZZZZZZZ\nMMMMMMMM\nAAAAAAAA")
    );

    let mut renamed = tree();
    renamed.collections[1].name = "Renamed".to_string();
    let renamed = descendant_tree(&renamed, "COLL0001").expect("renamed").1;
    assert_eq!(descendants_sha256(&renamed), sorted_hash());

    let mut reparented = tree();
    reparented.collections[1].parent = Some("OTHERXXX".to_string());
    let reparented = descendant_tree(&reparented, "COLL0001")
        .expect("reparented")
        .1;
    assert_eq!(
        reparented
            .iter()
            .map(|item| item.key.as_str())
            .collect::<Vec<_>>(),
        ["AAAAAAAA", "MMMMMMMM"]
    );
    assert_ne!(descendants_sha256(&reparented), sorted_hash());

    let empty = snapshot(0, vec![collection("COLL0001", "Root", None, 0, true)]);
    let (target, descendants) = descendant_tree(&empty, "COLL0001").expect("empty");
    assert!(target.deleted);
    assert!(descendants.is_empty());
    assert_eq!(descendants_sha256(&descendants), EMPTY_DESCENDANTS_SHA256);
    let report =
        build_report("server", &LibraryScope::User, empty, &args(false, 0, "")).expect("preview");
    assert!(report.already_trashed);
    assert_eq!(report.version, 0);
    assert_eq!(report.library_version, 0);
    assert_eq!(report.descendant_count, 0);
    assert_eq!(report.effect, EFFECT);

    assert_eq!(
        descendant_tree(&snapshot(1, vec![]), "COLL0001")
            .expect_err("missing")
            .payload()
            .code,
        "local-write-target"
    );
    let mut cyclic = snapshot(
        1,
        vec![
            collection("COLL0001", "Root", Some("CHILDBBB"), 1, false),
            collection("CHILDBBB", "Child", Some("COLL0001"), 1, false),
        ],
    );
    cyclic.collections[0].parent = Some("CHILDBBB".to_string());
    assert_eq!(
        descendant_tree(&cyclic, "COLL0001")
            .expect_err("cycle")
            .payload()
            .code,
        "local-api-protocol"
    );
    let duplicate = snapshot(
        1,
        vec![
            collection("COLL0001", "Root", None, 1, false),
            collection("COLL0001", "Again", None, 2, false),
        ],
    );
    assert_eq!(
        descendant_tree(&duplicate, "OTHERXXX")
            .expect_err("duplicate")
            .payload()
            .code,
        "local-api-protocol"
    );
    assert_eq!(
        require_descendant_hash(Some("abcd"), EMPTY_DESCENDANTS_SHA256)
            .expect_err("format")
            .payload()
            .code,
        "local-write-options"
    );
    assert_eq!(
        require_descendant_hash(Some(&"ab".repeat(32)), EMPTY_DESCENDANTS_SHA256)
            .expect_err("mismatch")
            .payload()
            .code,
        "local-delete-descendants"
    );
    assert!(
        require_descendant_hash(
            Some(&format!(
                "  {}  ",
                EMPTY_DESCENDANTS_SHA256.to_ascii_uppercase()
            )),
            EMPTY_DESCENDANTS_SHA256,
        )
        .is_ok()
    );
}

#[tokio::test]
async fn preview_does_not_authorize_or_delete() {
    let (client, handle) = server(vec![(200, Value::Null, None), (200, list_body(), Some(99))]);
    let dir = tempfile::tempdir().expect("temp");
    let path = dir.path().join("broken.json");
    std::fs::write(&path, "not-json").expect("write");
    let report = plan(&client, &LibraryScope::User, &args(false, 0, ""))
        .await
        .expect("plan");
    let report = execute(&client, report, &args(false, 0, "").write, &path, None)
        .await
        .expect("preview");
    assert_eq!(report.state, "preview");
    assert_eq!(report.operation, "delete-collection");
    assert_eq!(report.backend, "local");
    assert_eq!(report.library_version, 99);
    assert_eq!(report.version, 4);
    assert!(!report.already_trashed);
    assert_eq!(report.descendant_count, 3);
    assert_eq!(report.descendants_sha256, sorted_hash());
    assert_eq!(report.effect, EFFECT);
    assert!(!report.write_accepted);
    assert!(!report.verified);
    assert!(report.observed_expected_state.is_none());
    let requests = handle.join().expect("join");
    assert!(requests.iter().all(|request| request.method == "GET"));
    assert_eq!(deletes(&requests), 0);
    assert!(
        requests
            .iter()
            .all(|request| !request.path.contains("authorize"))
    );
    assert!(requests.iter().all(|request| {
        !request
            .headers
            .iter()
            .any(|(name, _)| name == "zotero-api-key")
    }));
    assert!(
        !serde_json::to_string(&report)
            .expect("json")
            .contains("item_count")
    );
}

#[tokio::test]
async fn mismatched_identity_does_not_delete() {
    let (client, handle) = server(vec![(200, Value::Null, None)]);
    let mut stale_server = args(true, 4, &sorted_hash());
    stale_server.write.expected_server_id = Some("other".to_string());
    let error = plan(&client, &LibraryScope::User, &stale_server)
        .await
        .expect_err("server");
    assert_eq!(error.payload().code, "local-api-conflict");
    let requests = handle.join().expect("join");
    assert_eq!(deletes(&requests), 0);
    assert!(requests.iter().all(|request| request.method == "GET"));

    let (client, handle) = server(vec![(200, Value::Null, None), (200, list_body(), Some(99))]);
    let error = plan(
        &client,
        &LibraryScope::User,
        &args(true, 99, &sorted_hash()),
    )
    .await
    .expect_err("library version is not the object version");
    assert_eq!(error.payload().code, "local-api-conflict");
    assert_eq!(deletes(&handle.join().expect("join")), 0);

    let (client, handle) = server(vec![(200, Value::Null, None), (200, list_body(), Some(99))]);
    let error = plan(
        &client,
        &LibraryScope::User,
        &args(true, 4, EMPTY_DESCENDANTS_SHA256),
    )
    .await
    .expect_err("hash");
    assert_eq!(error.payload().code, "local-delete-descendants");
    assert_eq!(deletes(&handle.join().expect("join")), 0);
}

#[tokio::test]
async fn invalid_key_or_hash_does_not_call_the_local_api() {
    let client =
        LocalApiClient::with_base_url_for_tests("http://127.0.0.1:1/", Duration::from_millis(200))
            .expect("client");
    let mut bad_key = args(false, 0, "");
    bad_key.key = "short".to_string();
    let error = plan(&client, &LibraryScope::User, &bad_key)
        .await
        .expect_err("key");
    assert_eq!(error.payload().code, "local-api-key");
    let error = plan(&client, &LibraryScope::User, &args(true, 0, "nope"))
        .await
        .expect_err("hash");
    assert_eq!(error.payload().code, "local-write-options");
}

#[tokio::test]
async fn confirmed_delete_applies_only_after_matching_204_and_get_404() {
    let hash = sorted_hash();
    let (client, handle) = server(vec![
        (200, Value::Null, None),
        (200, list_body(), Some(99)),
        (200, json!({"key": "test-secret", "remember": false}), None),
        (204, Value::Null, None),
        (404, Value::Null, None),
    ]);
    let dir = tempfile::tempdir().expect("temp");
    let request = args(true, 4, &hash);
    let report = plan(&client, &LibraryScope::User, &request)
        .await
        .expect("plan");
    assert_eq!(report.version, 4);
    assert_ne!(report.version, report.library_version);
    let report = execute(
        &client,
        report,
        &request.write,
        &dir.path().join("keys"),
        None,
    )
    .await
    .expect("apply");
    assert_eq!(report.state, "applied");
    assert!(report.write_accepted);
    assert!(report.verified);
    assert_eq!(report.observed_expected_state, Some(true));
    assert!(report.error.is_none());
    assert_eq!(exit_code(report.state), 0);
    let requests = handle.join().expect("join");
    assert_eq!(deletes(&requests), 1);
    assert_eq!(object_gets(&requests, "COLL0001"), 1);
    let delete = requests
        .iter()
        .find(|request| request.method == "DELETE")
        .expect("delete");
    assert_eq!(delete.path, "/api/users/0/collections/COLL0001");
    assert!(!delete.path.contains('?'));
    assert_eq!(delete.body, Value::Null);
    for (name, value) in [
        ("zotero-server-id", "server"),
        ("zotero-api-key", "test-secret"),
        ("if-unmodified-since-version", "4"),
    ] {
        assert!(
            delete
                .headers
                .iter()
                .any(|(header, header_value)| header == name && header_value == value)
        );
    }
    assert!(
        delete
            .headers
            .iter()
            .all(|(header, _)| header != "zotero-write-token")
    );
    assert!(
        requests
            .iter()
            .all(|request| !request.path.contains("/items/"))
    );
    assert!(
        !serde_json::to_string(&report)
            .expect("json")
            .contains("test-secret")
    );

    let (client, handle) = server(vec![
        (200, Value::Null, None),
        (200, list_body(), Some(99)),
        (200, json!({"key": "test-secret", "remember": false}), None),
        (204, Value::Null, None),
        (
            200,
            object("COLL0001", 4, "Root", Value::Bool(false), false),
            None,
        ),
    ]);
    let report = plan(&client, &LibraryScope::User, &request)
        .await
        .expect("plan");
    let report = execute(
        &client,
        report,
        &request.write,
        &dir.path().join("still-there"),
        None,
    )
    .await
    .expect("readback");
    assert_eq!(report.state, "verification-failed");
    assert!(report.write_accepted);
    assert!(!report.verified);
    assert_eq!(report.observed_expected_state, Some(false));
    assert_eq!(exit_code(report.state), 1);
    assert_eq!(deletes(&handle.join().expect("join")), 1);
}

#[tokio::test]
async fn unknown_delete_does_not_become_applied_when_a_later_404_is_available() {
    let hash = sorted_hash();
    let server = Server::http("127.0.0.1:0").expect("server");
    let client = LocalApiClient::with_base_url_for_tests(
        &format!("http://{}/", server.server_addr()),
        Duration::from_secs(1),
    )
    .expect("client");
    let scripted = vec![
        (200, Value::Null, None),
        (200, list_body(), Some(99)),
        (200, json!({"key": "test-secret", "remember": false}), None),
        (500, Value::Null, None),
    ];
    let handle = std::thread::spawn(move || {
        let mut captured = Vec::new();
        for (status, body, library_version) in scripted {
            let request = server
                .recv_timeout(Duration::from_secs(3))
                .expect("recv")
                .expect("request");
            captured.push(capture(request, body, status, library_version));
        }
        let extra = match server.recv_timeout(Duration::from_millis(200)) {
            Ok(Some(request)) => Some(capture(request, Value::Null, 404, None)),
            _ => None,
        };
        (captured, extra)
    });
    let dir = tempfile::tempdir().expect("temp");
    let request = args(true, 4, &hash);
    let report = plan(&client, &LibraryScope::User, &request)
        .await
        .expect("plan");
    let report = execute(
        &client,
        report,
        &request.write,
        &dir.path().join("keys"),
        None,
    )
    .await
    .expect("unknown");
    assert_eq!(report.state, "unknown");
    assert!(!report.write_accepted);
    assert!(!report.verified);
    assert!(report.observed_expected_state.is_none());
    assert_eq!(
        report.error.as_ref().expect("error").code,
        "local-api-unknown"
    );
    let state = report.state;
    let output = CommandOutput::new(&app_ctx(), report, None, |_| {})
        .expect("output")
        .with_exit_code(exit_code(state));
    assert_eq!(output.exit_code(), 1);
    assert!(output.as_json().expect("json").contains("\"ok\": true"));
    let (requests, extra) = handle.join().expect("join");
    assert_eq!(deletes(&requests), 1);
    assert_eq!(object_gets(&requests, "COLL0001"), 0);
    assert!(extra.is_none());
}

#[tokio::test]
async fn version_zero_can_confirm_and_absent_or_conflict_are_not_unknown() {
    let (client, handle) = server(vec![
        (200, Value::Null, None),
        (
            200,
            json!([object("COLL0001", 0, "Root", Value::Bool(false), false)]),
            Some(12),
        ),
        (200, json!({"key": "test-secret", "remember": false}), None),
        (204, Value::Null, None),
        (404, Value::Null, None),
    ]);
    let dir = tempfile::tempdir().expect("temp");
    let request = args(true, 0, EMPTY_DESCENDANTS_SHA256);
    let report = plan(&client, &LibraryScope::User, &request)
        .await
        .expect("plan");
    assert_eq!(report.version, 0);
    assert_eq!(report.descendants_sha256, EMPTY_DESCENDANTS_SHA256);
    let report = execute(
        &client,
        report,
        &request.write,
        &dir.path().join("keys"),
        None,
    )
    .await
    .expect("apply");
    assert_eq!(report.state, "applied");
    let requests = handle.join().expect("join");
    let delete = requests
        .iter()
        .find(|request| request.method == "DELETE")
        .expect("delete");
    assert!(
        delete
            .headers
            .iter()
            .any(|(name, value)| { name == "if-unmodified-since-version" && value == "0" })
    );

    let (client, handle) = server(vec![
        (200, Value::Null, None),
        (
            200,
            json!([object("COLL0001", 0, "Root", Value::Bool(false), false)]),
            Some(12),
        ),
        (200, json!({"key": "test-secret", "remember": false}), None),
        (404, Value::Null, None),
    ]);
    let report = plan(&client, &LibraryScope::User, &request)
        .await
        .expect("plan");
    let report = execute(
        &client,
        report,
        &request.write,
        &dir.path().join("absent"),
        None,
    )
    .await
    .expect("absent");
    assert_eq!(report.state, "absent");
    assert!(!report.write_accepted);
    assert_eq!(
        report.error.as_ref().expect("error").code,
        "local-delete-absent"
    );
    assert!(
        report
            .error
            .as_ref()
            .expect("error")
            .message
            .contains("did not delete")
    );
    let requests = handle.join().expect("join");
    assert_eq!(deletes(&requests), 1);
    assert_eq!(object_gets(&requests, "COLL0001"), 0);
    assert_eq!(exit_code(report.state), 1);

    let (client, handle) = server(vec![
        (200, Value::Null, None),
        (
            200,
            json!([object("COLL0001", 0, "Root", Value::Bool(false), false)]),
            Some(12),
        ),
        (200, json!({"key": "test-secret", "remember": false}), None),
        (412, Value::Null, None),
    ]);
    let report = plan(&client, &LibraryScope::User, &request)
        .await
        .expect("plan");
    let error = execute(
        &client,
        report,
        &request.write,
        &dir.path().join("conflict"),
        None,
    )
    .await
    .expect_err("conflict");
    assert_eq!(error.payload().code, "local-api-conflict");
    let requests = handle.join().expect("join");
    assert_eq!(deletes(&requests), 1);
    assert_eq!(object_gets(&requests, "COLL0001"), 0);
}

#[tokio::test]
async fn expired_delete_key_reauthorizes_once() {
    let (client, handle) = server(vec![
        (200, Value::Null, None),
        (
            200,
            json!([object("COLL0001", 0, "Root", Value::Bool(false), false)]),
            Some(12),
        ),
        (401, Value::Null, None),
        (200, json!({"key": "new-secret", "remember": false}), None),
        (204, Value::Null, None),
        (404, Value::Null, None),
    ]);
    let dir = tempfile::tempdir().expect("temp");
    let path = dir.path().join("keys");
    local_credentials::save(&path, Some("work"), "server", Some("old-secret".into()))
        .expect("save");
    let request = args(true, 0, EMPTY_DESCENDANTS_SHA256);
    let report = plan(&client, &LibraryScope::User, &request)
        .await
        .expect("plan");
    let report = execute(&client, report, &request.write, &path, Some("work"))
        .await
        .expect("apply");
    assert_eq!(report.state, "applied");
    let requests = handle.join().expect("join");
    assert_eq!(deletes(&requests), 2);
    assert_eq!(
        requests
            .iter()
            .filter(|request| request.path.ends_with("authorize"))
            .count(),
        1
    );
    assert!(
        !serde_json::to_string(&report)
            .expect("json")
            .contains("old-secret")
    );
    assert!(
        !serde_json::to_string(&report)
            .expect("json")
            .contains("new-secret")
    );
}

#[tokio::test]
async fn web_delete_rejects_local_flags_before_remote_and_plain_delete_uses_web() {
    let ctx = app_ctx();
    let mut flagged = args(true, 0, EMPTY_DESCENDANTS_SHA256);
    flagged.write.write_backend = WriteBackendArg::Web;
    let error = crate::commands::collection::handle(&ctx, CollectionCommand::Delete(flagged))
        .await
        .expect_err("flags");
    assert_eq!(
        error
            .downcast_ref::<zot_core::ZotError>()
            .expect("zot")
            .payload()
            .code,
        "local-write-options"
    );

    let error = crate::commands::collection::handle(
        &ctx,
        CollectionCommand::Delete(CollectionDeleteArgs {
            key: "COLL0001".to_string(),
            write: LocalDeleteArgs::default(),
        }),
    )
    .await
    .expect_err("web");
    assert_eq!(
        error
            .downcast_ref::<zot_core::ZotError>()
            .expect("zot")
            .payload()
            .code,
        "write-credentials"
    );
}
