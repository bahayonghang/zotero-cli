use std::thread::JoinHandle;
use std::time::Duration;

use clap::Parser;
use serde_json::json;
use tiny_http::{Header, Response, Server};
use zot_desktop::{LocalApiClient, LocalCollection, LocalCollectionSnapshot};

use super::*;
use crate::cli::{Cli, CollectionCommand, Commands, WriteBackendArg};

fn collection(key: &str, name: &str, parent: Option<&str>, deleted: bool) -> LocalCollection {
    LocalCollection {
        key: key.to_string(),
        version: 3,
        name: name.to_string(),
        parent: parent.map(ToOwned::to_owned),
        deleted,
    }
}

fn snapshot(collections: Vec<LocalCollection>) -> LocalCollectionSnapshot {
    LocalCollectionSnapshot {
        library_version: 12,
        collections,
    }
}

fn error_code(error: zot_core::ZotError) -> String {
    error.payload().code
}

#[test]
fn duplicate_names_are_parent_scoped_and_blank_names_are_rejected() {
    let collections = snapshot(vec![
        collection("ROOT2345", "待读", None, false),
        collection("CHILD234", "待读", Some("ROOT2345"), false),
        collection("DELCOLL1", "旧类", None, true),
    ]);
    assert_eq!(
        error_code(reject_duplicate(&collections, "待读", None).expect_err("duplicate")),
        "local-collection-duplicate"
    );
    assert!(reject_duplicate(&collections, "待读", Some("OTHER234")).is_ok());
    assert!(reject_duplicate(&collections, "新类", None).is_ok());
    assert_eq!(
        error_code(reject_duplicate(&collections, "旧类", None).expect_err("deleted duplicate")),
        "local-collection-duplicate"
    );
    assert_eq!(
        validate_name("   ").expect_err("blank").payload().code,
        "local-write-collection"
    );
    assert!(validate_name(" 待读 ").is_ok());
    let chain = parent_chain(&collections, Some("CHILD234")).expect("path");
    assert_eq!(
        chain
            .iter()
            .map(|entry| entry.key.as_str())
            .collect::<Vec<_>>(),
        ["ROOT2345", "CHILD234"]
    );
    assert_eq!(
        error_code(parent_chain(&collections, Some("MISSING2")).expect_err("missing")),
        "local-write-target"
    );
    let mut cyclic = collections;
    cyclic.collections[1].parent = Some("CHILD234".to_string());
    assert_eq!(
        error_code(parent_chain(&cyclic, Some("CHILD234")).expect_err("cycle")),
        "local-api-protocol"
    );
    let deleted_parent = snapshot(vec![collection("ROOT2345", "根", None, true)]);
    assert_eq!(
        error_code(parent_chain(&deleted_parent, Some("ROOT2345")).expect_err("deleted")),
        "local-write-target"
    );
}

#[test]
fn clap_keeps_web_create_and_requires_library_version_not_item_version() {
    let cli = Cli::try_parse_from(["zot", "collection", "create", "待读"]).expect("web");
    let Commands::Collection {
        command: CollectionCommand::Create(args),
    } = cli.command
    else {
        panic!("create")
    };
    assert_eq!(args.write.write_backend, WriteBackendArg::Web);
    assert!(validate_web_options(&args.write).is_ok());
    assert!(
        Cli::try_parse_from([
            "zot",
            "collection",
            "create",
            "待读",
            "--confirm",
            "--expected-server-id",
            "server",
            "--expected-library-version",
            "12",
        ])
        .is_ok()
    );
    let cli = Cli::try_parse_from([
        "zot",
        "collection",
        "create",
        "待读",
        "--confirm",
        "--expected-server-id",
        "server",
        "--expected-library-version",
        "12",
    ])
    .expect("flags parse");
    let Commands::Collection {
        command: CollectionCommand::Create(args),
    } = cli.command
    else {
        panic!("create")
    };
    assert!(validate_web_options(&args.write).is_err());
    assert!(
        Cli::try_parse_from([
            "zot",
            "collection",
            "create",
            "待读",
            "--write-backend",
            "local",
            "--confirm",
        ])
        .is_err()
    );
    assert!(
        Cli::try_parse_from([
            "zot",
            "collection",
            "create",
            "待读",
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
}

#[derive(Debug)]
struct Captured {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
    body: Value,
}

fn server(responses: Vec<(u16, Value, bool)>) -> (LocalApiClient, JoinHandle<Vec<Captured>>) {
    let server = Server::http("127.0.0.1:0").expect("server");
    let client = LocalApiClient::with_base_url_for_tests(
        &format!("http://{}/", server.server_addr()),
        Duration::from_secs(1),
    )
    .expect("client");
    let handle = std::thread::spawn(move || {
        let mut captured = Vec::new();
        for (status, body, library_version) in responses {
            let mut request = server
                .recv_timeout(Duration::from_secs(3))
                .expect("recv")
                .expect("request");
            let mut text = String::new();
            request.as_reader().read_to_string(&mut text).expect("body");
            captured.push(Captured {
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
            });
            let mut response = Response::from_string(body.to_string()).with_status_code(status);
            for (name, value) in [
                ("Zotero-Server-ID", "server"),
                ("Zotero-API-Version", "3"),
                ("X-Zotero-Version", "10.0.6"),
            ] {
                response.add_header(Header::from_bytes(name, value).expect("header"));
            }
            if library_version {
                response.add_header(
                    Header::from_bytes("Last-Modified-Version", "12").expect("version"),
                );
                if let Some(entries) = body.as_array() {
                    response.add_header(
                        Header::from_bytes("Total-Results", entries.len().to_string())
                            .expect("total"),
                    );
                }
            }
            request.respond(response).expect("respond");
        }
        captured
    });
    (client, handle)
}

fn args(confirm: bool, parent: Option<&str>) -> CollectionCreateArgs {
    CollectionCreateArgs {
        name: "待读".to_string(),
        parent: parent.map(ToOwned::to_owned),
        write: LocalCreateArgs {
            write_backend: WriteBackendArg::Local,
            confirm,
            expected_server_id: confirm.then(|| "server".to_string()),
            expected_library_version: confirm.then_some(12),
        },
    }
}

fn list_body() -> Value {
    json!([])
}

#[tokio::test]
async fn preview_and_duplicate_do_not_authorize_or_post() {
    let (client, handle) = server(vec![(200, Value::Null, true), (200, list_body(), true)]);
    let dir = tempfile::tempdir().expect("temp");
    let path = dir.path().join("broken.json");
    std::fs::write(&path, "not-json").expect("write");
    let report = plan(&client, &LibraryScope::User, &args(false, None))
        .await
        .expect("plan");
    let report = execute(&client, report, &args(false, None).write, &path, None)
        .await
        .expect("preview");
    assert_eq!(report.state, "preview");
    assert_eq!(report.operation, "create-collection");
    assert_eq!(report.expected_library_version, 12);
    assert_eq!(report.before, Value::Null);
    assert_eq!(report.after["name"], "待读");
    assert_eq!(report.after["parentCollection"], false);
    assert!(report.collection_key.is_empty());
    assert!(
        handle
            .join()
            .expect("join")
            .iter()
            .all(|request| request.method == "GET")
    );

    let (client, handle) = server(vec![
        (200, Value::Null, true),
        (
            200,
            json!([{
                "key": "ROOT2345",
                "version": 3,
                "data": {
                    "key": "ROOT2345",
                    "version": 3,
                    "name": "待读",
                    "parentCollection": false
                }
            }]),
            true,
        ),
    ]);
    let error = plan(&client, &LibraryScope::User, &args(true, None))
        .await
        .expect_err("duplicate");
    assert_eq!(error.payload().code, "local-collection-duplicate");
    assert!(error.payload().message.contains("ROOT2345"));
    assert!(
        handle
            .join()
            .expect("join")
            .iter()
            .all(|request| request.method == "GET")
    );
}

#[tokio::test]
async fn confirmed_create_uses_library_version_and_readback() {
    let created = json!({
        "key": "NEWCOLL1",
        "version": 4,
        "data": {
            "key": "NEWCOLL1",
            "version": 4,
            "name": "子类",
            "parentCollection": "ROOT2345"
        }
    });
    let (client, handle) = server(vec![
        (200, Value::Null, true),
        (
            200,
            json!([{
                "key": "ROOT2345",
                "version": 3,
                "data": {
                    "key": "ROOT2345",
                    "version": 3,
                    "name": "根",
                    "parentCollection": false
                }
            }]),
            true,
        ),
        (200, json!({"key": "test-secret", "remember": false}), true),
        (
            200,
            json!({
                "successful": {"0": {"key": "NEWCOLL1", "data": {"key": "NEWCOLL1"}}},
                "unchanged": {},
                "failed": {}
            }),
            true,
        ),
        (200, created, true),
    ]);
    let dir = tempfile::tempdir().expect("temp");
    let mut request = args(true, Some("ROOT2345"));
    request.name = "子类".to_string();
    let report = plan(&client, &LibraryScope::User, &request)
        .await
        .expect("plan");
    assert_eq!(report.path.len(), 1);
    assert_eq!(report.path[0].name, "根");
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
    assert!(report.verified);
    assert_eq!(report.collection_key, "NEWCOLL1");
    let requests = handle.join().expect("join");
    let posts: Vec<_> = requests
        .iter()
        .filter(|request| request.method == "POST" && request.path.ends_with("/collections"))
        .collect();
    assert_eq!(posts.len(), 1);
    assert_eq!(
        posts[0].body,
        json!([{"name": "子类", "parentCollection": "ROOT2345"}])
    );
    assert!(
        posts[0]
            .headers
            .iter()
            .any(|(key, value)| key == "if-unmodified-since-version" && value == "12")
    );
    assert!(
        posts[0]
            .headers
            .iter()
            .all(|(key, _)| key != "zotero-write-token")
    );
    assert!(
        requests
            .iter()
            .all(|request| !request.path.contains("/items/"))
    );
    assert!(
        !serde_json::to_string(&report)
            .expect("serialize")
            .contains("test-secret")
    );
}

#[tokio::test]
async fn expired_create_key_reauthorizes_once_and_mismatched_readback_is_not_applied() {
    let readback = json!({
        "key": "NEWCOLL1",
        "version": 4,
        "data": {
            "key": "NEWCOLL1",
            "version": 4,
            "name": "待读",
            "parentCollection": false
        }
    });
    let success = json!({
        "successful": {"0": {"key": "NEWCOLL1", "data": {"key": "NEWCOLL1"}}},
        "unchanged": {},
        "failed": {}
    });
    let (client, handle) = server(vec![
        (200, Value::Null, true),
        (200, list_body(), true),
        (401, Value::Null, true),
        (200, json!({"key": "new-key", "remember": true}), true),
        (200, success.clone(), true),
        (200, readback, true),
    ]);
    let dir = tempfile::tempdir().expect("temp");
    let path = dir.path().join("keys");
    local_credentials::save(&path, Some("work"), "server", Some("old-key".into())).expect("save");
    let report = plan(&client, &LibraryScope::User, &args(true, None))
        .await
        .expect("plan");
    let report = execute(
        &client,
        report,
        &args(true, None).write,
        &path,
        Some("work"),
    )
    .await
    .expect("apply");
    assert_eq!(report.state, "applied");
    let requests = handle.join().expect("join");
    let creates: Vec<_> = requests
        .iter()
        .filter(|request| request.method == "POST" && request.path.ends_with("/collections"))
        .collect();
    assert_eq!(creates.len(), 2);
    assert_eq!(
        requests
            .iter()
            .filter(|request| request.path.ends_with("authorize"))
            .count(),
        1
    );

    let mismatch = json!({
        "key": "NEWCOLL1",
        "version": 4,
        "data": {
            "key": "NEWCOLL1",
            "version": 4,
            "name": "其他",
            "parentCollection": false
        }
    });
    let (client, handle) = server(vec![
        (200, Value::Null, true),
        (200, list_body(), true),
        (200, json!({"key": "once", "remember": false}), true),
        (200, success, true),
        (200, mismatch, true),
    ]);
    let report = plan(&client, &LibraryScope::User, &args(true, None))
        .await
        .expect("plan");
    let report = execute(
        &client,
        report,
        &args(true, None).write,
        &dir.path().join("other"),
        None,
    )
    .await
    .expect("mismatch");
    assert_eq!(report.state, "verification-failed");
    assert!(report.write_accepted);
    assert!(!report.verified);
    assert_eq!(report.collection_key, "NEWCOLL1");
    assert_eq!(
        handle
            .join()
            .expect("join")
            .iter()
            .filter(|request| request.method == "POST" && request.path.ends_with("/collections"))
            .count(),
        1
    );
}

#[tokio::test]
async fn unknown_create_does_not_retry_or_claim_a_same_name() {
    let (client, handle) = server(vec![
        (200, Value::Null, true),
        (200, list_body(), true),
        (200, json!({"key": "once", "remember": false}), true),
        (500, Value::Null, true),
    ]);
    let dir = tempfile::tempdir().expect("temp");
    let report = plan(&client, &LibraryScope::User, &args(true, None))
        .await
        .expect("plan");
    let report = execute(
        &client,
        report,
        &args(true, None).write,
        &dir.path().join("keys"),
        None,
    )
    .await
    .expect("unknown");
    assert_eq!(report.state, "unknown");
    assert!(!report.write_accepted);
    assert!(!report.verified);
    assert!(report.collection_key.is_empty());
    assert!(report.observed_expected_state.is_none());
    let requests = handle.join().expect("join");
    assert_eq!(
        requests
            .iter()
            .filter(|request| request.method == "POST" && request.path.ends_with("/collections"))
            .count(),
        1
    );
}

#[tokio::test]
async fn stale_library_version_and_missing_header_stop_before_post() {
    let (client, handle) = server(vec![(200, Value::Null, true), (200, list_body(), true)]);
    let mut stale = args(true, None);
    stale.write.expected_library_version = Some(3);
    let error = plan(&client, &LibraryScope::User, &stale)
        .await
        .expect_err("stale");
    assert_eq!(error.payload().code, "local-api-conflict");
    assert!(
        handle
            .join()
            .expect("join")
            .iter()
            .all(|request| request.method == "GET")
    );

    let (client, handle) = server(vec![(200, Value::Null, true), (200, list_body(), false)]);
    let error = plan(&client, &LibraryScope::User, &args(true, None))
        .await
        .expect_err("missing version");
    assert_eq!(error.payload().code, "local-api-protocol");
    assert_eq!(handle.join().expect("join").len(), 2);
}
