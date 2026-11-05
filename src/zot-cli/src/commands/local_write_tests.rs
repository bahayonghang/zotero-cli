use clap::Parser;
use std::thread::JoinHandle;
use std::time::Duration;
use tiny_http::{Header, Response, Server};

use super::*;
use crate::cli::{Cli, Commands, ItemCommand, ItemTagCommand, WriteBackendArg};

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
    headers: Vec<(String, String)>,
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
    for field in ["unknown", "collections", "tags", "version", "itemType"] {
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

fn names(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_string()).collect()
}

fn tag_error(current: Value, requested: &[&str], remove: bool) -> String {
    plan_tags(Some(&current), &names(requested), remove)
        .expect_err("rejected tags")
        .payload()
        .code
}

fn sample_tags() -> Value {
    json!([
        {"tag":"自动标签","type":1,"meta":{"source":"bbt","k":1}},
        {"tag":"手工","type":0},
        {"tag":"省略类型"},
        {"tag":"重复","type":0,"note":"keep"},
        {"tag":"重复","type":1}
    ])
}

fn object_with_tags(tags: Value, version: u64) -> Value {
    let mut data = item().data;
    data.insert("tags".to_string(), tags);
    data.insert("version".to_string(), json!(version));
    json!({"key":"ITEMABCD","version":version,"data":data})
}

fn header_value<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.as_str())
}

#[test]
fn tag_diff_preserves_types_duplicates_and_rejects_blank_or_illegal_input() {
    let current = sample_tags();
    let added = plan_tags(
        Some(&current),
        &names(&["待读", "待读", "C++/安全", " priority", "Priority"]),
        false,
    )
    .expect("add");
    assert_eq!(added.len(), 9);
    assert_eq!(added[0], current[0]);
    assert_eq!(added[1], json!({"tag":"手工","type":0}));
    assert_eq!(added[2], json!({"tag":"省略类型"}));
    assert_eq!(added[3], json!({"tag":"重复","type":0,"note":"keep"}));
    assert_eq!(added[4], json!({"tag":"重复","type":1}));
    assert_eq!(added[5], json!({"tag":"待读","type":0}));
    assert_eq!(added[6], json!({"tag":"C++/安全","type":0}));
    assert_eq!(added[7], json!({"tag":" priority","type":0}));
    assert_eq!(added[8], json!({"tag":"Priority","type":0}));
    assert!(
        plan_tags(
            Some(&current),
            &names(&["自动标签", "手工", "省略类型"]),
            false
        )
        .expect("existing names")
            == current.as_array().expect("tags").clone()
    );
    assert!(
        plan_tags(Some(&current), &names(&["priority"]), false)
            .expect("case sensitive")
            .iter()
            .any(|tag| tag["tag"] == "priority")
    );

    let removed = plan_tags(Some(&current), &names(&["重复", "不存在"]), true).expect("remove");
    assert_eq!(removed.len(), 3);
    assert!(removed.iter().all(|tag| tag["tag"] != "重复"));
    assert_eq!(removed[0]["meta"], json!({"source":"bbt","k":1}));
    assert_eq!(
        plan_tags(Some(&current), &names(&["不存在"]), true).expect("missing"),
        current.as_array().expect("tags").clone()
    );
    assert_eq!(
        plan_tags(None, &names(&["待读"]), false).expect("missing field"),
        vec![json!({"tag":"待读","type":0})]
    );
    assert!(
        plan_tags(Some(&json!([])), &[], false)
            .expect("empty request")
            .is_empty()
    );

    for blank in ["", "   ", "\t", "\u{3000}"] {
        assert_eq!(
            tag_error(current.clone(), &[blank], false),
            "local-write-tag",
            "{blank:?}"
        );
    }
    for illegal in [
        json!(null),
        json!("tags"),
        json!({"tag":"x"}),
        json!([1]),
        json!([{"type":1}]),
        json!([{"tag":1}]),
        json!([{"tag":"x","type":2}]),
        json!([{"tag":"x","type":"1"}]),
        json!([{"tag":"ok"}, {"tag":"bad","type":2}]),
    ] {
        assert_eq!(tag_error(illegal, &["待读"], false), "local-api-protocol");
    }
}

#[test]
fn tag_readback_ignores_order_and_manual_type_shape_but_keeps_duplicates() {
    let mut item = item();
    item.data.insert(
        "tags".to_string(),
        json!([
            {"tag":"手工","type":0,"color":"#fff"},
            {"tag":"自动","type":1,"meta":{"source":"bbt","k":1}},
            {"tag":"自动","type":1,"meta":{"k":1,"source":"bbt"}}
        ]),
    );
    assert!(matches_patch(
        &item,
        json!({"tags":[
            {"tag":"自动","type":1,"meta":{"k":1,"source":"bbt"}},
            {"tag":"手工","color":"#fff"},
            {"tag":"自动","type":1,"meta":{"source":"bbt","k":1}}
        ]})
        .as_object()
        .expect("map")
    ));
    assert!(!matches_patch(
        &item,
        json!({"tags":[{"tag":"自动"},{"tag":"手工","type":0,"color":"#fff"},{"tag":"自动","type":1}]})
            .as_object()
            .expect("map")
    ));
    assert!(!matches_patch(
        &item,
        json!({"tags":[{"tag":"自动","type":1},{"tag":"手工","type":0,"color":"#fff"}]})
            .as_object()
            .expect("map")
    ));
    assert!(!matches_patch(
        &item,
        json!({"tags":[
            {"tag":"手工","type":0},
            {"tag":"自动","type":1,"meta":{"source":"bbt","k":1}},
            {"tag":"自动","type":1,"meta":{"source":"bbt","k":1}}
        ]})
        .as_object()
        .expect("map")
    ));
    assert!(tags_match(None, &json!([])));
    assert!(!tags_match(None, &json!([{"tag":"待读","type":0}])));
}

#[test]
fn tag_clap_defaults_to_web_and_does_not_extend_batch() {
    let cli = Cli::try_parse_from(["zot", "item", "tag", "add", "ITEMABCD", "--tag", "priority"])
        .expect("web add");
    let Commands::Item {
        command: ItemCommand::Tag {
            command: ItemTagCommand::Add(args),
        },
    } = cli.command
    else {
        panic!("add")
    };
    assert_eq!(args.write.write_backend, WriteBackendArg::Web);
    assert!(validate_web_options(&args.write).is_ok());

    let cli = Cli::try_parse_from([
        "zot",
        "item",
        "tag",
        "remove",
        "ITEMABCD",
        "--tag",
        "priority",
        "--confirm",
        "--expected-server-id",
        "server",
        "--expected-version",
        "7",
    ])
    .expect("web confirm parses");
    let Commands::Item {
        command: ItemCommand::Tag {
            command: ItemTagCommand::Remove(args),
        },
    } = cli.command
    else {
        panic!("remove")
    };
    assert!(validate_web_options(&args.write).is_err());
    assert!(
        Cli::try_parse_from([
            "zot",
            "item",
            "tag",
            "add",
            "ITEMABCD",
            "--tag",
            "priority",
            "--write-backend",
            "local",
            "--confirm",
        ])
        .is_err()
    );
    assert!(
        Cli::try_parse_from([
            "zot",
            "item",
            "tag",
            "batch",
            "--add-tag",
            "reviewed",
            "--write-backend",
            "local",
        ])
        .is_err()
    );
}

#[tokio::test]
async fn tag_preview_and_noop_never_authorize_or_patch() {
    let current = sample_tags();
    let (client, handle) = server(vec![
        (200, Value::Null),
        (200, object_with_tags(current.clone(), 7)),
    ]);
    let dir = tempfile::tempdir().expect("temp");
    let path = dir.path().join("broken.json");
    std::fs::write(&path, "not-json").expect("write");
    let report = tag_report(
        &client,
        &LibraryScope::User,
        "ITEMABCD",
        &names(&["待读", "待读"]),
        false,
        &options(false),
    )
    .await
    .expect("preview plan");
    let report = execute(&client, report, &options(false), &path, None)
        .await
        .expect("preview");
    assert_eq!(report.state, "preview");
    assert_eq!(report.operation, "tag-add");
    assert_eq!(report.after["tags"].as_array().expect("tags").len(), 6);
    assert_eq!(report.after["tags"][5], json!({"tag":"待读","type":0}));
    assert!(report.before["tags"].as_array().expect("before").len() == 5);
    assert!(!report.write_accepted);
    let requests = handle.join().expect("join");
    assert!(requests.iter().all(|request| request.method == "GET"));

    let (client, handle) = server(vec![
        (200, Value::Null),
        (200, object_with_tags(current, 7)),
    ]);
    let report = tag_report(
        &client,
        &LibraryScope::User,
        "ITEMABCD",
        &names(&["自动标签"]),
        false,
        &options(true),
    )
    .await
    .expect("noop plan");
    let report = execute(&client, report, &options(true), &path, None)
        .await
        .expect("noop");
    assert_eq!(report.state, "unchanged");
    assert!(report.after.is_empty());
    assert!(
        handle
            .join()
            .expect("join")
            .iter()
            .all(|request| request.method == "GET")
    );
}

#[tokio::test]
async fn confirmed_tag_write_patches_complete_array_once_and_accepts_reordered_readback() {
    let current = sample_tags();
    let planned = plan_tags(Some(&current), &names(&["待读", "C++/安全"]), false).expect("plan");
    let mut readback_tags = planned.clone();
    readback_tags.reverse();
    for tag in &mut readback_tags {
        if tag.get("type").and_then(Value::as_u64) == Some(0) {
            tag.as_object_mut().expect("object").remove("type");
        }
    }
    let (client, handle) = server(vec![
        (200, Value::Null),
        (200, object_with_tags(current, 7)),
        (200, json!({"key":"test-secret","remember":false})),
        (204, Value::Null),
        (200, object_with_tags(Value::Array(readback_tags), 8)),
    ]);
    let dir = tempfile::tempdir().expect("temp");
    let report = tag_report(
        &client,
        &LibraryScope::User,
        "ITEMABCD",
        &names(&["待读", "C++/安全"]),
        false,
        &options(true),
    )
    .await
    .expect("plan");
    let report = execute(
        &client,
        report,
        &options(true),
        &dir.path().join("keys"),
        None,
    )
    .await
    .expect("apply");
    assert_eq!(report.state, "applied");
    assert!(report.verified);
    assert!(report.write_accepted);
    assert!(
        !serde_json::to_string(&report)
            .expect("serialize")
            .contains("test-secret")
    );
    let requests = handle.join().expect("join");
    let patches: Vec<_> = requests
        .iter()
        .filter(|request| request.method == "PATCH")
        .collect();
    assert_eq!(patches.len(), 1);
    assert_eq!(patches[0].path, "/api/users/0/items/ITEMABCD");
    assert_eq!(patches[0].body["version"], 7);
    assert_eq!(patches[0].body["tags"], json!(planned));
    assert_eq!(patches[0].body.as_object().expect("body").len(), 2);
    assert_eq!(
        header_value(&patches[0].headers, "if-unmodified-since-version"),
        Some("7")
    );
    assert_eq!(
        header_value(&patches[0].headers, "zotero-api-key"),
        Some("test-secret")
    );
    assert_eq!(
        header_value(&patches[0].headers, "zotero-server-id"),
        Some("server")
    );
    assert!(
        patches[0]
            .headers
            .iter()
            .all(|(key, _)| key != "zotero-write-token")
    );
}

#[tokio::test]
async fn tag_unknown_and_verification_failure_are_not_success() {
    let current = sample_tags();
    let planned = plan_tags(Some(&current), &names(&["待读"]), false).expect("plan");
    for (patch_status, readback, state, observed) in [
        (
            500,
            object_with_tags(Value::Array(planned.clone()), 8),
            "unknown",
            Some(true),
        ),
        (
            204,
            object_with_tags(json!([{"tag":"待读"}]), 8),
            "verification-failed",
            Some(false),
        ),
    ] {
        let (client, handle) = server(vec![
            (200, Value::Null),
            (200, object_with_tags(current.clone(), 7)),
            (200, json!({"key":"once","remember":false})),
            (patch_status, Value::Null),
            (200, readback),
        ]);
        let dir = tempfile::tempdir().expect("temp");
        let report = tag_report(
            &client,
            &LibraryScope::User,
            "ITEMABCD",
            &names(&["待读"]),
            false,
            &options(true),
        )
        .await
        .expect("plan");
        let report = execute(
            &client,
            report,
            &options(true),
            &dir.path().join("keys"),
            None,
        )
        .await
        .expect("report");
        assert_eq!(report.state, state);
        assert!(!report.verified);
        assert_eq!(report.observed_expected_state, observed);
        assert_eq!(
            handle
                .join()
                .expect("join")
                .iter()
                .filter(|request| request.method == "PATCH")
                .count(),
            1
        );
    }
}

#[tokio::test]
async fn expired_tag_key_reauthorizes_once() {
    let current = sample_tags();
    let planned = plan_tags(Some(&current), &names(&["重复"]), true).expect("plan");
    let (client, handle) = server(vec![
        (200, Value::Null),
        (200, object_with_tags(current, 7)),
        (401, Value::Null),
        (200, json!({"key":"new-key","remember":true})),
        (204, Value::Null),
        (200, object_with_tags(Value::Array(planned), 8)),
    ]);
    let dir = tempfile::tempdir().expect("temp");
    let path = dir.path().join("keys");
    local_credentials::save(&path, Some("work"), "server", Some("old-key".into())).expect("save");
    let report = tag_report(
        &client,
        &LibraryScope::User,
        "ITEMABCD",
        &names(&["重复"]),
        true,
        &options(true),
    )
    .await
    .expect("plan");
    let report = execute(&client, report, &options(true), &path, Some("work"))
        .await
        .expect("apply");
    assert_eq!(report.state, "applied");
    assert_eq!(report.operation, "tag-remove");
    let requests = handle.join().expect("join");
    let patches: Vec<_> = requests
        .iter()
        .filter(|request| request.method == "PATCH")
        .collect();
    assert_eq!(patches.len(), 2);
    assert_eq!(
        header_value(&patches[0].headers, "zotero-api-key"),
        Some("old-key")
    );
    assert_eq!(
        header_value(&patches[1].headers, "zotero-api-key"),
        Some("new-key")
    );
    assert_eq!(
        requests
            .iter()
            .filter(|request| request.path.ends_with("authorize"))
            .count(),
        1
    );
}

#[tokio::test]
async fn tag_rejects_invalid_targets_and_illegal_tags_before_patch() {
    let mut child = item().data;
    child.insert("parentItem".to_string(), json!("PARENT12"));
    let mut trashed = item().data;
    trashed.insert("deleted".to_string(), json!(true));
    let mut note = item().data;
    note.insert("itemType".to_string(), json!("note"));
    for data in [child, trashed, note] {
        let (client, handle) = server(vec![
            (200, Value::Null),
            (200, json!({"key":"ITEMABCD","version":7,"data":data})),
        ]);
        let error = tag_report(
            &client,
            &LibraryScope::User,
            "ITEMABCD",
            &names(&["待读"]),
            false,
            &options(false),
        )
        .await
        .expect_err("rejected target");
        assert_eq!(error.payload().code, "local-write-target");
        assert!(
            handle
                .join()
                .expect("join")
                .iter()
                .all(|request| request.method == "GET")
        );
    }

    let (client, handle) = server(vec![(200, Value::Null)]);
    let error = tag_report(
        &client,
        &LibraryScope::User,
        "bad-key",
        &names(&["待读"]),
        false,
        &options(false),
    )
    .await
    .expect_err("bad key");
    assert_eq!(error.payload().code, "local-api-key");
    assert_eq!(handle.join().expect("join").len(), 1);

    let (client, handle) = server(vec![
        (200, Value::Null),
        (200, object_with_tags(json!("not-an-array"), 7)),
    ]);
    let dir = tempfile::tempdir().expect("temp");
    std::fs::write(dir.path().join("broken.json"), "not-json").expect("write");
    let error = tag_report(
        &client,
        &LibraryScope::User,
        "ITEMABCD",
        &names(&["待读"]),
        false,
        &options(true),
    )
    .await
    .expect_err("illegal tags");
    assert_eq!(error.payload().code, "local-api-protocol");
    assert!(
        handle
            .join()
            .expect("join")
            .iter()
            .all(|request| request.method == "GET")
    );

    let (client, handle) = server(vec![
        (200, Value::Null),
        (200, object_with_tags(sample_tags(), 7)),
    ]);
    let mut stale = options(true);
    stale.expected_version = Some(3);
    let error = tag_report(
        &client,
        &LibraryScope::User,
        "ITEMABCD",
        &names(&["待读"]),
        false,
        &stale,
    )
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
}
