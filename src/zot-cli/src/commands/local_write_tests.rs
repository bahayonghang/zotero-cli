use clap::Parser;
use std::thread::JoinHandle;
use std::time::Duration;
use tiny_http::{Header, Response, Server};

use super::*;
use crate::cli::{Cli, Commands, ItemCommand, WriteBackendArg};

fn item() -> LocalObject {
    serde_json::from_value(json!({"key":"ITEMABCD","version":7,"data":{
        "key":"ITEMABCD","version":7,"itemType":"conferencePaper","title":"Existing title",
        "DOI":"", "abstractNote":" ", "creators":[],"collections":["COLLA234","COLLC234"]
    }}))
    .expect("item")
}

fn options(confirm: bool) -> LocalWriteArgs {
    LocalWriteArgs {
        write_backend: WriteBackendArg::Local,
        confirm,
        expected_server_id: confirm.then(|| "server".to_string()),
        expected_version: confirm.then_some(7),
    }
}

fn plan() -> MutationReport {
    let item = item();
    let mut report =
        MutationReport::new("server".to_string(), &LibraryScope::User, &item, "update");
    report.change(&item, "DOI", json!("10.1000/test"));
    report
}

#[derive(Debug)]
struct Captured {
    method: String,
    path: String,
    body: Value,
}

fn server(responses: Vec<(u16, Value)>) -> (LocalApiClient, JoinHandle<Vec<Captured>>) {
    let server = Server::http("127.0.0.1:0").expect("server");
    let client = LocalApiClient::with_base_url_for_tests(
        &format!("http://{}/", server.server_addr()),
        Duration::from_secs(1),
    )
    .expect("client");
    let handle = std::thread::spawn(move || {
        let mut captured = Vec::new();
        for (status, body) in responses {
            let mut request = server
                .recv_timeout(Duration::from_secs(3))
                .expect("recv")
                .expect("request");
            let mut text = String::new();
            request.as_reader().read_to_string(&mut text).expect("body");
            captured.push(Captured {
                method: request.method().to_string(),
                path: request.url().to_string(),
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
            request.respond(response).expect("respond");
        }
        captured
    });
    (client, handle)
}

fn readback(doi: &str) -> Value {
    let mut data = item().data;
    data.insert("DOI".to_string(), json!(doi));
    data.insert("version".to_string(), json!(8));
    json!({"key":"ITEMABCD","version":8,"data":data})
}

#[test]
fn membership_preserves_other_collections_and_handles_repeats() {
    let item = item();
    assert_eq!(
        membership_patch(&item, Some("COLLB234"), Some("COLLA234")).expect("move"),
        ["COLLC234", "COLLB234"]
    );
    assert_eq!(
        membership_patch(&item, Some("COLLC234"), Some("COLLA234")).expect("move"),
        ["COLLC234"]
    );
    assert_eq!(
        membership_patch(&item, Some("COLLA234"), Some("COLLA234")).expect("same"),
        ["COLLA234", "COLLC234"]
    );
    assert_eq!(
        membership_patch(&item, Some("COLLA234"), Some("MISSING2")).expect("already target"),
        ["COLLA234", "COLLC234"]
    );
    assert!(membership_patch(&item, Some("COLLB234"), Some("MISSING2")).is_err());
    assert_eq!(
        membership_patch(&item, None, Some("MISSING2")).expect("remove"),
        ["COLLA234", "COLLC234"]
    );
    assert_eq!(
        membership_patch(&item, Some("COLLA234"), None).expect("add"),
        ["COLLA234", "COLLC234"]
    );
    assert_eq!(item.data["title"], "Existing title");
}

#[test]
fn field_fill_preserves_nonempty_values_and_overwrite_is_explicit() {
    let item = item();
    let fields = BTreeMap::from([
        ("DOI".to_string(), "10.1000/test".to_string()),
        ("title".to_string(), "Replacement".to_string()),
    ]);
    let allowed = vec!["DOI".to_string(), "title".to_string()];
    let mut report =
        MutationReport::new("server".to_string(), &LibraryScope::User, &item, "update");
    plan_fields(
        &mut report,
        &item,
        fields.clone(),
        None,
        &allowed,
        &[],
        false,
    )
    .expect("fill");
    assert_eq!(
        report.after,
        json!({"DOI":"10.1000/test"})
            .as_object()
            .expect("object")
            .clone()
    );
    assert_eq!(report.skipped_fields, ["title"]);
    let mut report =
        MutationReport::new("server".to_string(), &LibraryScope::User, &item, "update");
    plan_fields(&mut report, &item, fields, None, &allowed, &[], true).expect("overwrite");
    assert_eq!(report.after["title"], "Replacement");
    for field in ["unknown", "collections", "version", "itemType"] {
        assert!(
            plan_fields(
                &mut report,
                &item,
                BTreeMap::from([(field.to_string(), "bad".to_string())]),
                None,
                &allowed,
                &[],
                true
            )
            .is_err()
        );
    }
}

#[test]
fn structured_creators_preserve_roles_order_and_require_explicit_replacement() {
    let types = vec!["author".to_string(), "editor".to_string()];
    let creators = json!([{"creatorType":"author","firstName":"Ada","lastName":"Lovelace"},{"creatorType":"editor","name":"Research Institute"}]);
    let mut item = item();
    let mut report =
        MutationReport::new("server".to_string(), &LibraryScope::User, &item, "update");
    plan_fields(
        &mut report,
        &item,
        BTreeMap::new(),
        Some(creators.clone()),
        &[],
        &types,
        false,
    )
    .expect("creators");
    assert_eq!(report.after["creators"], creators);
    item.data.insert(
        "creators".to_string(),
        json!([{"creatorType":"author","name":"Existing"}]),
    );
    let mut report =
        MutationReport::new("server".to_string(), &LibraryScope::User, &item, "update");
    plan_fields(
        &mut report,
        &item,
        BTreeMap::new(),
        Some(creators.clone()),
        &[],
        &types,
        false,
    )
    .expect("skip");
    assert!(report.after.is_empty());
    assert_eq!(report.skipped_fields, ["creators"]);
    plan_fields(
        &mut report,
        &item,
        BTreeMap::new(),
        Some(creators.clone()),
        &[],
        &types,
        true,
    )
    .expect("replace");
    assert_eq!(report.after["creators"], creators);
    for value in [
        json!([]),
        json!("[]"),
        json!([{"creatorType":"bad","name":"Name"}]),
        json!([{"creatorType":"author","name":" "}]),
        json!([{"creatorType":"author","name":"Org","lastName":"X"}]),
        json!([{"creatorType":"author","firstName":"","lastName":""}]),
    ] {
        assert!(validate_creators(&value, &types).is_err(), "{value}");
    }
}

#[test]
fn collection_order_is_not_a_readback_failure_but_author_order_matters() {
    let mut item = item();
    assert!(matches_patch(
        &item,
        json!({"collections":["COLLC234","COLLA234"]})
            .as_object()
            .expect("map")
    ));
    assert!(!matches_patch(
        &item,
        json!({"collections":["COLLC234"]})
            .as_object()
            .expect("map")
    ));
    let authors = json!([
        {"creatorType":"author","firstName":"Ada","lastName":"Lovelace"},
        {"creatorType":"author","name":"Research Institute"}
    ]);
    item.data.insert("creators".to_string(), authors.clone());
    assert!(matches_patch(
        &item,
        json!({"creators":authors}).as_object().expect("map")
    ));
    let mut reversed = authors.as_array().expect("authors").clone();
    reversed.reverse();
    assert!(!matches_patch(
        &item,
        json!({"creators":reversed}).as_object().expect("map")
    ));
}

#[test]
fn clap_preserves_web_default_and_requires_preview_guards() {
    let parse = |tail: &[&str]| {
        Cli::try_parse_from(
            ["zot", "item", "update", "ITEMABCD"]
                .into_iter()
                .chain(tail.iter().copied()),
        )
    };
    let cli = parse(&["--title", "New"]).expect("legacy");
    let Commands::Item {
        command: ItemCommand::Update(args),
    } = cli.command
    else {
        panic!("update")
    };
    assert_eq!(args.write.write_backend, WriteBackendArg::Web);
    assert!(parse(&["--write-backend", "local", "--confirm"]).is_err());
    let cli = parse(&[
        "--write-backend",
        "local",
        "--confirm",
        "--expected-server-id",
        "server",
        "--expected-version",
        "7",
        "--creators-file",
        "authors.json",
    ])
    .expect("local");
    let Commands::Item {
        command: ItemCommand::Update(args),
    } = cli.command
    else {
        panic!("update")
    };
    assert_eq!(args.write.write_backend, WriteBackendArg::Local);
    assert!(validate_web_options(&args.write).is_err());
    assert!(
        Cli::try_parse_from([
            "zot",
            "collection",
            "move-item",
            "ITEMABCD",
            "--from",
            "COLLA234",
            "--to",
            "COLLB234",
            "--write-backend",
            "local"
        ])
        .is_ok()
    );
    for field in ["broken", "=value", "title= "] {
        let cli = parse(&["--field", field]).expect("parse");
        let Commands::Item {
            command: ItemCommand::Update(args),
        } = cli.command
        else {
            panic!("update")
        };
        assert!(parse_fields(&args).is_err());
    }
}

#[tokio::test]
async fn preview_and_noop_never_authorize_or_read_credentials() {
    let (client, handle) = server(vec![]);
    let dir = tempfile::tempdir().expect("temp");
    let path = dir.path().join("broken.json");
    std::fs::write(&path, "not-json").expect("write");
    assert_eq!(
        execute(&client, plan(), &options(false), &path, None)
            .await
            .expect("preview")
            .state,
        "preview"
    );
    let mut noop = plan();
    noop.after.clear();
    assert_eq!(
        execute(&client, noop, &options(true), &path, None)
            .await
            .expect("noop")
            .state,
        "unchanged"
    );
    assert_eq!(handle.join().expect("join").len(), 0);
}

#[tokio::test]
async fn confirmed_write_authorizes_patches_once_and_reads_back() {
    let (client, handle) = server(vec![
        (200, json!({"key":"test-secret","remember":false})),
        (204, Value::Null),
        (200, readback("10.1000/test")),
    ]);
    let dir = tempfile::tempdir().expect("temp");
    let path = dir.path().join("credentials.json");
    let report = execute(&client, plan(), &options(true), &path, None)
        .await
        .expect("apply");
    assert_eq!(report.state, "applied");
    assert!(report.verified);
    assert!(!path.exists());
    assert!(
        !serde_json::to_string(&report)
            .expect("serialize")
            .contains("test-secret")
    );
    let requests = handle.join().expect("join");
    assert_eq!(
        requests
            .iter()
            .map(|r| r.method.as_str())
            .collect::<Vec<_>>(),
        ["POST", "PATCH", "GET"]
    );
    assert_eq!(requests[1].path, "/api/users/0/items/ITEMABCD");
    assert_eq!(requests[1].body, json!({"version":7,"DOI":"10.1000/test"}));
}

#[tokio::test]
async fn denial_and_conflict_stop_without_retry() {
    for (responses, code) in [
        (vec![(403, json!({"denied":true}))], "local-api-forbidden"),
        (
            vec![
                (200, json!({"key":"once","remember":false})),
                (412, Value::Null),
            ],
            "local-api-conflict",
        ),
    ] {
        let count = responses.len();
        let (client, handle) = server(responses);
        let dir = tempfile::tempdir().expect("temp");
        assert_eq!(
            execute(
                &client,
                plan(),
                &options(true),
                &dir.path().join("keys"),
                None
            )
            .await
            .expect_err("denied")
            .payload()
            .code,
            code
        );
        assert_eq!(handle.join().expect("join").len(), count);
    }
}

#[tokio::test]
async fn submitted_failure_remains_unknown_even_if_readback_matches() {
    let (client, handle) = server(vec![
        (200, json!({"key":"once","remember":false})),
        (500, Value::Null),
        (200, readback("10.1000/test")),
    ]);
    let dir = tempfile::tempdir().expect("temp");
    let report = execute(
        &client,
        plan(),
        &options(true),
        &dir.path().join("keys"),
        None,
    )
    .await
    .expect("report");
    assert_eq!(report.state, "unknown");
    assert!(!report.verified);
    assert_eq!(report.observed_expected_state, Some(true));
    assert_eq!(
        handle
            .join()
            .expect("join")
            .iter()
            .filter(|r| r.method == "PATCH")
            .count(),
        1
    );
}

#[tokio::test]
async fn accepted_write_with_unverified_readback_is_reported_distinctly() {
    for response in [(200, readback("different")), (503, Value::Null)] {
        let (client, handle) = server(vec![
            (200, json!({"key":"once","remember":false})),
            (204, Value::Null),
            response,
        ]);
        let dir = tempfile::tempdir().expect("temp");
        let report = execute(
            &client,
            plan(),
            &options(true),
            &dir.path().join("keys"),
            None,
        )
        .await
        .expect("report");
        assert_eq!(report.state, "verification-failed");
        assert!(report.write_accepted);
        assert!(!report.verified);
        handle.join().expect("join");
    }
}

#[tokio::test]
async fn expired_cached_key_reauthorizes_once_and_preserves_profile_isolation() {
    let (client, handle) = server(vec![
        (401, Value::Null),
        (200, json!({"key":"new-key","remember":true})),
        (204, Value::Null),
        (200, readback("10.1000/test")),
    ]);
    let dir = tempfile::tempdir().expect("temp");
    let path = dir.path().join("keys");
    local_credentials::save(&path, Some("work"), "server", Some("old-key".into())).expect("save");
    let report = execute(&client, plan(), &options(true), &path, Some("work"))
        .await
        .expect("report");
    assert_eq!(report.state, "applied");
    assert_eq!(
        local_credentials::load(&path, Some("work"), "server")
            .expect("load")
            .expect("key")
            .expose_secret(),
        "new-key"
    );
    let requests = handle.join().expect("join");
    assert_eq!(
        requests
            .iter()
            .filter(|r| r.path.ends_with("authorize"))
            .count(),
        1
    );
}

#[tokio::test]
async fn stale_preview_or_wrong_server_prevents_authorization() {
    for (server_id, version, count) in [("other", 7, 1), ("server", 6, 2)] {
        let mut responses = vec![(200, Value::Null)];
        if count == 2 {
            responses.push((
                200,
                json!({"key":"ITEMABCD","version":7,"data":item().data}),
            ));
        }
        let (client, handle) = server(responses);
        let mut args = options(true);
        args.expected_server_id = Some(server_id.to_string());
        args.expected_version = Some(version);
        let err = read_target(&client, &LibraryScope::User, "ITEMABCD", &args)
            .await
            .expect_err("stale");
        assert_eq!(err.payload().code, "local-api-conflict");
        assert!(
            handle
                .join()
                .expect("join")
                .iter()
                .all(|r| r.method == "GET")
        );
    }
}
