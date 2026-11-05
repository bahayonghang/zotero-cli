use serde_json::{Map, json};
use zot_core::{LibraryScope, ZotError};
use zot_desktop::LocalApiClient;

use super::*;
use crate::context::AppContext;

fn request(items: Vec<ItemRequest>) -> OrganizeRequest {
    OrganizeRequest {
        schema_version: 1,
        items,
    }
}

fn item(key: &str) -> ItemRequest {
    ItemRequest {
        item_key: key.to_string(),
        add_tags: vec!["待读".to_string()],
        remove_tags: Vec::new(),
        add_collections: Vec::new(),
        remove_collections: Vec::new(),
        move_collection: None,
        reason: Some("review".to_string()),
    }
}

fn code(error: ZotError) -> String {
    error.payload().code
}

#[test]
fn request_whitelist_rejects_conflicts_and_counts_changed_items_once() {
    assert_eq!(
        code(
            validate_request(&request(vec![item("ITEMABCD"), item("ITEMABCD")])).expect_err("dup")
        ),
        "local-organize-conflict"
    );
    let mut conflicting = item("ITEMABCD");
    conflicting.remove_tags = vec!["待读".to_string()];
    assert_eq!(
        code(validate_item_request(&conflicting).expect_err("tag conflict")),
        "local-organize-conflict"
    );
    let mut moved = item("ITEMABCD");
    moved.add_tags.clear();
    moved.add_collections = vec!["COLLA234".to_string()];
    moved.move_collection = Some(MoveRequest {
        from: "COLLA234".to_string(),
        to: "COLLB234".to_string(),
    });
    assert_eq!(
        code(validate_item_request(&moved).expect_err("move mix")),
        "local-organize-conflict"
    );
    let mut blank = item("ITEMABCD");
    blank.add_tags = vec!["   ".to_string()];
    assert_eq!(
        code(validate_item_request(&blank).expect_err("blank")),
        "local-write-tag"
    );
    assert_eq!(exit_code("applied"), 0);
    assert_eq!(exit_code("unchanged"), 0);
    assert_eq!(exit_code("partial"), 1);
    assert_eq!(exit_code("unknown"), 1);
}

#[test]
fn aggregate_state_keeps_partial_and_unknown_distinct() {
    let mut report = OrganizeReport {
        schema_version: 1,
        plan_sha256: "abc".to_string(),
        server_id: "server".to_string(),
        library: "user".to_string(),
        state: String::new(),
        started_at: String::new(),
        updated_at: String::new(),
        item_count: 3,
        applied_count: 0,
        unchanged_count: 0,
        failed_count: 0,
        unknown_count: 0,
        not_attempted_count: 0,
        items: vec![
            status_item("A", "applied"),
            status_item("B", "conflict"),
            status_item("C", "not-attempted"),
        ],
        error: None,
    };
    assert_eq!(aggregate_state(&report), "partial");
    report.items[0].status = "unknown".to_string();
    assert_eq!(aggregate_state(&report), "unknown");
    report.items[0].status = "unchanged".to_string();
    report.items[1].status = "unchanged".to_string();
    report.items[2].status = "unchanged".to_string();
    assert_eq!(aggregate_state(&report), "unchanged");
}

fn status_item(key: &str, status: &str) -> ReportItem {
    ReportItem {
        item_key: key.to_string(),
        status: status.to_string(),
        before: Map::new(),
        after: Map::new(),
        expected_version: 1,
        observed_version: None,
        write_accepted: false,
        verified: false,
        observed_expected_state: None,
        error: None,
    }
}

#[tokio::test]
async fn second_unknown_stops_and_leaves_the_next_item_not_attempted() {
    let server = tiny_http::Server::http("127.0.0.1:0").expect("server");
    let client = LocalApiClient::with_base_url_for_tests(
        &format!("http://{}/", server.server_addr()),
        std::time::Duration::from_secs(1),
    )
    .expect("client");
    let responses = vec![
        (200, serde_json::Value::Null),
        (200, json!([])),
        (200, live_item("ITEMABCD", 7, json!([]))),
        (200, live_item("ITEMBCDE", 7, json!([]))),
        (200, live_item("ITEMCDEF", 7, json!([]))),
        (200, live_item("ITEMABCD", 7, json!([]))),
        (200, json!({"key": "once", "remember": false})),
        (204, serde_json::Value::Null),
        (
            200,
            live_item("ITEMABCD", 8, json!([{"tag": "待读", "type": 0}])),
        ),
        (200, live_item("ITEMBCDE", 7, json!([]))),
        (500, serde_json::Value::Null),
        (200, live_item("ITEMBCDE", 7, json!([]))),
    ];
    let handle = std::thread::spawn(move || {
        let mut count = 0;
        for (status, body) in responses {
            let mut request = server
                .recv_timeout(std::time::Duration::from_secs(3))
                .expect("recv")
                .expect("request");
            let mut text = String::new();
            request.as_reader().read_to_string(&mut text).expect("body");
            count += 1;
            let mut response =
                tiny_http::Response::from_string(body.to_string()).with_status_code(status);
            for (name, value) in [
                ("Zotero-Server-ID", "server"),
                ("Zotero-API-Version", "3"),
                ("X-Zotero-Version", "10.0.6"),
                ("Last-Modified-Version", "4"),
            ] {
                response.add_header(tiny_http::Header::from_bytes(name, value).expect("header"));
            }
            if body.as_array().is_some() {
                response.add_header(
                    tiny_http::Header::from_bytes("Total-Results", "0").expect("total"),
                );
            }
            request.respond(response).expect("respond");
        }
        count
    });
    let ctx = test_context();
    let request = request(vec![item("ITEMABCD"), item("ITEMBCDE"), item("ITEMCDEF")]);
    let plan = build_plan(&client, &ctx, &request, 50).await.expect("plan");
    assert_eq!(plan.changed_count, 3);
    let mut report = initial_report(&plan, "hash");
    let dir = tempfile::tempdir().expect("temp");
    let report_path = dir.path().join("report.json");
    write_json(&report_path, &report, false).expect("create report");
    let report = execute_plan(
        &client,
        &ctx,
        &plan,
        &mut report,
        &report_path,
        &dir.path().join("keys"),
    )
    .await
    .expect("execute");
    assert_eq!(report.state, "partial");
    assert_eq!(report.items[0].status, "applied");
    assert_eq!(report.items[1].status, "unknown");
    assert!(!report.items[1].verified);
    assert_eq!(report.items[2].status, "not-attempted");
    assert_eq!(exit_code(&report.state), 1);
    assert_eq!(handle.join().expect("join"), 12);
    let saved = std::fs::read_to_string(&report_path).expect("saved");
    assert!(!saved.contains("once"));
    assert!(saved.contains("not-attempted"));
}

fn live_item(key: &str, version: u64, tags: serde_json::Value) -> serde_json::Value {
    json!({
        "key": key,
        "version": version,
        "data": {
            "key": key,
            "version": version,
            "itemType": "journalArticle",
            "title": "Paper",
            "tags": tags,
            "collections": []
        }
    })
}

fn test_context() -> AppContext {
    AppContext {
        json: true,
        profile: None,
        scope: LibraryScope::User,
        config: zot_core::AppConfig::default(),
        http: std::sync::Arc::new(zot_remote::HttpRuntime::new().expect("http")),
        pdf: std::sync::Arc::new(zot_local::PdfiumBackend),
    }
}

#[test]
fn hash_uses_exact_bytes_and_plan_file_is_not_overwritten() {
    let dir = tempfile::tempdir().expect("temp");
    let path = dir.path().join("plan.json");
    std::fs::write(&path, b"{\"schema_version\":1}\n").expect("write");
    let first = sha256_hex(&std::fs::read(&path).expect("read"));
    std::fs::write(&path, b"{\"schema_version\":1}").expect("rewrite");
    assert_ne!(first, sha256_hex(&std::fs::read(&path).expect("read")));
    let error = write_json(&path, &json!({"ok": true}), false).expect_err("exists");
    assert_eq!(error.payload().code, "local-organize-file");
    assert_eq!(
        std::fs::read(&path).expect("unchanged"),
        b"{\"schema_version\":1}"
    );
}

fn fixture_plan(requests: Vec<ItemRequest>) -> OrganizePlan {
    let snapshot = LocalCollectionSnapshot {
        library_version: 4,
        collections: Vec::new(),
    };
    let items = requests
        .iter()
        .map(|request| {
            let live = serde_json::from_value(live_item(&request.item_key, 7, json!([])))
                .expect("live item");
            plan_item(&live, request, &snapshot).expect("plan item")
        })
        .collect::<Vec<_>>();
    OrganizePlan {
        schema_version: 1,
        backend: "local".into(),
        server_id: "server".into(),
        library: "user".into(),
        profile: None,
        max_affected: 50,
        item_count: items.len(),
        action_count: requests.iter().map(action_count_of).sum(),
        changed_count: items.iter().filter(|item| item.changed).count(),
        unchanged_count: items.iter().filter(|item| !item.changed).count(),
        items,
    }
}

#[test]
fn edited_plan_cannot_bypass_counts_requests_or_changed_flags() {
    let ctx = test_context();
    let original = fixture_plan(vec![item("ITEMABCD")]);
    validate_plan_shape(&original, &ctx, 50).expect("valid");
    let mut variants = Vec::new();
    let mut plan = original.clone();
    plan.changed_count = 0;
    variants.push(plan);
    let mut plan = original.clone();
    plan.items[0].changed = false;
    plan.changed_count = 0;
    plan.unchanged_count = 1;
    variants.push(plan);
    let mut plan = original.clone();
    plan.items[0].original_request.item_key = "ITEMBCDE".into();
    variants.push(plan);
    let mut plan = original.clone();
    plan.items[0]
        .original_request
        .remove_tags
        .push("待读".into());
    variants.push(plan);
    let mut plan = original.clone();
    plan.items.push(plan.items[0].clone());
    plan.item_count = 2;
    plan.changed_count = 2;
    plan.action_count = 2;
    variants.push(plan);
    let mut plan = original.clone();
    plan.items[0]
        .after
        .insert("title".into(), json!("unreviewed"));
    variants.push(plan);
    let mut plan = original;
    plan.action_count = 0;
    variants.push(plan);
    for (index, plan) in variants.iter().enumerate() {
        assert!(
            validate_plan_shape(plan, &ctx, 50).is_err(),
            "edited plan {index}"
        );
    }
}

#[test]
fn report_save_failure_keeps_known_outcome_and_forces_failure_exit() {
    let plan = fixture_plan(vec![item("ITEMABCD")]);
    let mut report = initial_report(&plan, "hash");
    apply_outcome(
        &mut report.items[0],
        Ok(SubmitOutcome::Readback {
            accepted: true,
            version: 8,
            matches: true,
        }),
    );
    let report = stop_for_report_error(&mut report, 0, invalid("io", "save failed"), true);
    assert_eq!(report.items[0].status, "applied");
    assert!(report.items[0].verified);
    assert_ne!(exit_code(&report.state), 0);
}

struct Reply {
    method: &'static str,
    path: String,
    status: u16,
    body: Value,
}

fn reply(method: &'static str, path: &str, status: u16, body: Value) -> Reply {
    Reply {
        method,
        path: path.into(),
        status,
        body,
    }
}

fn respond(request: tiny_http::Request, status: u16, body: Value) {
    let mut response = tiny_http::Response::from_string(body.to_string()).with_status_code(status);
    for (name, value) in [
        ("Zotero-Server-ID", "server"),
        ("Zotero-API-Version", "3"),
        ("X-Zotero-Version", "10.0.7"),
        ("Last-Modified-Version", "4"),
    ] {
        response.add_header(tiny_http::Header::from_bytes(name, value).expect("header"));
    }
    if let Some(array) = body.as_array() {
        response.add_header(
            tiny_http::Header::from_bytes("Total-Results", array.len().to_string()).expect("total"),
        );
    }
    request.respond(response).expect("respond");
}

fn scripted_server(replies: Vec<Reply>) -> (LocalApiClient, std::thread::JoinHandle<Vec<Value>>) {
    let server = tiny_http::Server::http("127.0.0.1:0").expect("server");
    let client = LocalApiClient::with_base_url_for_tests(
        &format!("http://{}/", server.server_addr()),
        std::time::Duration::from_secs(2),
    )
    .expect("client");
    let handle = std::thread::spawn(move || {
        let mut patches = Vec::new();
        for expected in replies {
            let mut request = server
                .recv_timeout(std::time::Duration::from_secs(5))
                .expect("recv")
                .expect("expected request");
            assert_eq!(request.method().as_str(), expected.method);
            assert_eq!(request.url(), expected.path);
            if expected.method == "PATCH" {
                let mut body = String::new();
                request.as_reader().read_to_string(&mut body).expect("body");
                let body: Value = serde_json::from_str(&body).expect("patch json");
                assert_eq!(body["version"], 7);
                assert!(
                    request
                        .headers()
                        .iter()
                        .any(|header| header.field.equiv("If-Unmodified-Since-Version")
                            && header.value.as_str() == "7")
                );
                patches.push(body);
            }
            respond(request, expected.status, expected.body);
        }
        assert!(
            server
                .recv_timeout(std::time::Duration::from_millis(100))
                .expect("extra recv")
                .is_none(),
            "unexpected retry or write"
        );
        patches
    });
    (client, handle)
}

fn get_item(key: &str, version: u64, tags: Value) -> Reply {
    reply(
        "GET",
        &format!("/api/users/0/items/{key}"),
        200,
        live_item(key, version, tags),
    )
}

fn auth() -> Reply {
    reply(
        "POST",
        "/api/local/authorize",
        200,
        json!({"key": "test-only-secret", "remember": false}),
    )
}

async fn execute_fixture(client: &LocalApiClient, plan: &OrganizePlan) -> OrganizeReport {
    let dir = tempfile::tempdir().expect("temp");
    let path = dir.path().join("report.json");
    let mut report = initial_report(plan, "fixture-hash");
    write_json(&path, &report, false).expect("initial report");
    let report = execute_plan(
        client,
        &test_context(),
        plan,
        &mut report,
        &path,
        &dir.path().join("credentials"),
    )
    .await
    .expect("execute");
    let saved: Value =
        serde_json::from_slice(&std::fs::read(path).expect("saved")).expect("report JSON");
    assert_eq!(saved, serde_json::to_value(&report).expect("serialize"));
    assert!(!saved.to_string().contains("test-only-secret"));
    assert_eq!(
        report.item_count,
        report.applied_count
            + report.unchanged_count
            + report.failed_count
            + report.unknown_count
            + report.not_attempted_count
    );
    report
}

#[tokio::test]
async fn fifty_one_changes_require_explicit_matching_limit_and_never_write_in_plan() {
    let requests = request(
        (0..51)
            .map(|index| item(&format!("ITEM{index:04}")))
            .collect(),
    );
    let replies = || {
        let mut replies = vec![
            reply("GET", "/api/", 200, Value::Null),
            reply("GET", "/api/users/0/collections", 200, json!([])),
        ];
        replies.extend(
            requests
                .items
                .iter()
                .map(|item| get_item(&item.item_key, 7, json!([]))),
        );
        replies
    };
    let ctx = test_context();
    let (client, handle) = scripted_server(replies());
    assert_eq!(
        code(
            build_plan(&client, &ctx, &requests, 50)
                .await
                .expect_err("ceiling")
        ),
        "local-organize-max-affected"
    );
    assert!(handle.join().expect("join").is_empty());
    let (client, handle) = scripted_server(replies());
    let plan = build_plan(&client, &ctx, &requests, 51)
        .await
        .expect("raised ceiling");
    assert_eq!(plan.changed_count, 51);
    assert_eq!(plan.item_count, 51);
    validate_plan_shape(&plan, &ctx, 51).expect("explicit apply ceiling");
    assert_eq!(
        code(validate_plan_shape(&plan, &ctx, 50).expect_err("mismatch")),
        "local-organize-max-affected"
    );
    let mut forged = plan.clone();
    forged.max_affected = 50;
    forged.changed_count = 0;
    assert!(validate_plan_shape(&forged, &ctx, 50).is_err());
    assert!(handle.join().expect("join").is_empty());
    let mut replies = Vec::new();
    for (index, item) in plan.items.iter().enumerate() {
        replies.push(get_item(&item.item_key, 7, json!([])));
        if index == 0 {
            replies.push(auth());
        }
        replies.push(reply(
            "PATCH",
            &format!("/api/users/0/items/{}", item.item_key),
            204,
            Value::Null,
        ));
        replies.push(get_item(&item.item_key, 8, json!([{"tag":"待读"}])));
    }
    let (client, handle) = scripted_server(replies);
    let report = execute_fixture(&client, &plan).await;
    assert_eq!(report.state, "applied");
    assert_eq!(report.applied_count, 51);
    assert_eq!(handle.join().expect("join").len(), 51);
}

fn collection_body(key: &str, name: &str, version: u64) -> Value {
    json!({"key": key, "version": version, "data": {"key": key, "version": version, "name": name, "parentCollection": false}})
}

#[test]
fn membership_comparison_rejects_duplicate_substitution() {
    assert!(same_json_keys(
        &json!(["COLLA234", "COLLB234"]),
        &json!(["COLLB234", "COLLA234"])
    ));
    assert!(!same_json_keys(
        &json!(["COLLA234", "COLLB234"]),
        &json!(["COLLA234", "COLLA234"])
    ));
}

#[tokio::test]
async fn three_distinct_decisions_preview_then_apply_one_minimal_patch_each() {
    let collection = collection_body("COLLA234", "待读", 4);
    let mut requests = vec![item("ITEMABCD"), item("ITEMBCDE"), item("ITEMCDEF")];
    requests[0].add_collections.push("COLLA234".into());
    requests[1].add_tags.clear();
    requests[1].remove_tags.push("obsolete".into());
    requests[1].remove_collections.push("COLLA234".into());
    requests[2].add_tags = vec!["已读".into()];
    let before: Vec<Value> = requests
        .iter()
        .enumerate()
        .map(|(index, request)| {
            let mut live = live_item(
                &request.item_key,
                7,
                json!([
                    {"tag":"automatic", "type":1, "meta":"preserved"}, {"tag":"obsolete"}
                ]),
            );
            live["data"]["collections"] = if index == 1 {
                json!(["OTHER234", "COLLA234"])
            } else {
                json!(["OTHER234"])
            };
            live["data"]["creators"] = json!([{"lastName":"Unchanged", "creatorType":"author"}]);
            live
        })
        .collect();
    let mut replies = vec![
        reply("GET", "/api/", 200, Value::Null),
        reply(
            "GET",
            "/api/users/0/collections",
            200,
            json!([collection.clone()]),
        ),
    ];
    for live in &before {
        replies.push(reply(
            "GET",
            &format!("/api/users/0/items/{}", live["key"].as_str().expect("key")),
            200,
            live.clone(),
        ));
    }
    let (client, handle) = scripted_server(replies);
    let ctx = test_context();
    let plan = build_plan(&client, &ctx, &request(requests), 50)
        .await
        .expect("plan");
    assert!(handle.join().expect("preview zero writes").is_empty());
    validate_plan_shape(&plan, &ctx, 50).expect("shape");
    let mut replies = vec![
        reply("GET", "/api/", 200, Value::Null),
        reply(
            "GET",
            "/api/users/0/collections",
            200,
            json!([collection.clone()]),
        ),
    ];
    for live in &before {
        replies.push(reply(
            "GET",
            &format!("/api/users/0/items/{}", live["key"].as_str().expect("key")),
            200,
            live.clone(),
        ));
    }
    for (index, item) in plan.items.iter().enumerate() {
        let path = format!("/api/users/0/items/{}", item.item_key);
        replies.push(reply("GET", &path, 200, before[index].clone()));
        if !item.collections.is_empty() {
            replies.push(reply(
                "GET",
                "/api/users/0/collections/COLLA234",
                200,
                collection.clone(),
            ));
        }
        if index == 0 {
            replies.push(auth());
        }
        if !item.collections.is_empty() {
            replies.push(reply(
                "GET",
                "/api/users/0/collections/COLLA234",
                200,
                collection.clone(),
            ));
        }
        replies.push(reply("PATCH", &path, 204, Value::Null));
        let mut after = before[index].clone();
        after["version"] = json!(8);
        after["data"]["version"] = json!(8);
        for (field, value) in &item.after {
            after["data"][field] = value.clone();
        }
        replies.push(reply("GET", &path, 200, after));
    }
    let (client, handle) = scripted_server(replies);
    preflight(&client, &ctx, &plan).await.expect("preflight");
    let report = execute_fixture(&client, &plan).await;
    assert_eq!(report.state, "applied");
    assert_eq!(report.applied_count, 3);
    assert_eq!(exit_code(&report.state), 0);
    let patches = handle.join().expect("join");
    assert_eq!(patches.len(), 3);
    for (patch, item) in patches.iter().zip(&plan.items) {
        let mut expected = item.after.clone();
        expected.insert("version".into(), json!(7));
        assert_eq!(patch, &json!(expected));
    }
    assert_eq!(patches[0]["tags"][0], before[0]["data"]["tags"][0]);
    assert_eq!(patches[1]["collections"], json!(["OTHER234"]));
    assert!(patches[2].get("collections").is_none());
}

#[tokio::test]
async fn noop_rechecks_version_without_credentials_or_authorization() {
    for stale in [false, true] {
        let mut request = item("ITEMABCD");
        request.add_tags.clear();
        request.remove_tags.push("missing".into());
        let plan = fixture_plan(vec![request]);
        let (client, handle) = scripted_server(vec![get_item(
            "ITEMABCD",
            if stale { 8 } else { 7 },
            json!([]),
        )]);
        let report = execute_fixture(&client, &plan).await;
        assert_eq!(
            report.items[0].status,
            if stale { "conflict" } else { "unchanged" }
        );
        assert_eq!(exit_code(&report.state), i32::from(stale));
        assert!(handle.join().expect("join").is_empty());
    }
}

#[tokio::test]
async fn first_rejection_readback_failure_and_401_stop_or_reauthorize_once() {
    let plan = fixture_plan(vec![item("ITEMABCD"), item("ITEMBCDE")]);
    for scenario in ["denied", "forbidden", "conflict", "readback", "401-twice"] {
        let mut replies = vec![get_item("ITEMABCD", 7, json!([]))];
        if scenario == "denied" {
            replies.push(reply("POST", "/api/local/authorize", 403, Value::Null));
        } else {
            replies.push(auth());
            let status = match scenario {
                "forbidden" => 403,
                "conflict" => 412,
                "401-twice" => 401,
                _ => 204,
            };
            replies.push(reply(
                "PATCH",
                "/api/users/0/items/ITEMABCD",
                status,
                Value::Null,
            ));
            if scenario == "401-twice" {
                replies.push(auth());
                replies.push(reply(
                    "PATCH",
                    "/api/users/0/items/ITEMABCD",
                    401,
                    Value::Null,
                ));
            } else if scenario == "readback" {
                replies.push(reply(
                    "GET",
                    "/api/users/0/items/ITEMABCD",
                    500,
                    Value::Null,
                ));
            }
        }
        let (client, handle) = scripted_server(replies);
        let report = execute_fixture(&client, &plan).await;
        assert_eq!(
            report.items[0].status,
            match scenario {
                "conflict" => "conflict",
                "readback" => "verification-failed",
                _ => "failed",
            }
        );
        assert_eq!(report.items[1].status, "not-attempted");
        assert_eq!(exit_code(&report.state), 1);
        assert_eq!(
            handle.join().expect("join").len(),
            match scenario {
                "denied" => 0,
                "401-twice" => 2,
                _ => 1,
            }
        );
    }
    let single = fixture_plan(vec![item("ITEMABCD")]);
    let (client, handle) = scripted_server(vec![
        get_item("ITEMABCD", 7, json!([])),
        auth(),
        reply("PATCH", "/api/users/0/items/ITEMABCD", 401, Value::Null),
        auth(),
        reply("PATCH", "/api/users/0/items/ITEMABCD", 204, Value::Null),
        get_item("ITEMABCD", 8, json!([{"tag":"待读"}])),
    ]);
    assert_eq!(execute_fixture(&client, &single).await.state, "applied");
    assert_eq!(handle.join().expect("join").len(), 2);
}

#[tokio::test]
async fn preflight_rejects_drift_in_any_target_before_authorization() {
    let original = fixture_plan(vec![item("ITEMABCD"), item("ITEMBCDE")]);
    for scenario in ["server", "last-version", "before", "after", "title"] {
        let mut plan = original.clone();
        let mut replies = vec![reply("GET", "/api/", 200, Value::Null)];
        if scenario == "server" {
            plan.server_id = "other-instance".into();
        } else {
            replies.push(reply("GET", "/api/users/0/collections", 200, json!([])));
            replies.push(get_item("ITEMABCD", 7, json!([])));
            match scenario {
                "before" => {
                    plan.items[0]
                        .before
                        .insert("tags".into(), json!([{"tag":"forged"}]));
                }
                "after" => {
                    plan.items[0]
                        .after
                        .insert("tags".into(), json!([{"tag":"forged"}]));
                }
                "title" => plan.items[0].title = "forged".into(),
                _ => replies.push(get_item("ITEMBCDE", 8, json!([]))),
            }
        }
        let (client, handle) = scripted_server(replies);
        assert!(
            preflight(&client, &test_context(), &plan).await.is_err(),
            "{scenario}"
        );
        assert!(handle.join().expect("join").is_empty());
    }
    for scenario in ["library", "profile"] {
        let mut plan = original.clone();
        if scenario == "library" {
            plan.library = "group:12".into();
        } else {
            plan.profile = Some("other".into());
        }
        assert_eq!(
            code(validate_plan_shape(&plan, &test_context(), 50).expect_err("scope")),
            "local-organize-scope"
        );
    }
}

#[tokio::test]
async fn collection_drift_after_preflight_or_authorization_prevents_patch() {
    let snapshot = LocalCollectionSnapshot {
        library_version: 4,
        collections: vec![LocalCollection {
            key: "COLLA234".into(),
            name: "Original".into(),
            version: 4,
            parent: None,
            deleted: false,
        }],
    };
    let mut request = item("ITEMABCD");
    request.add_collections.push("COLLA234".into());
    let mut plan = fixture_plan(vec![item("ITEMABCD")]);
    plan.items[0] = plan_item(
        &serde_json::from_value(live_item("ITEMABCD", 7, json!([]))).expect("live"),
        &request,
        &snapshot,
    )
    .expect("item plan");
    plan.action_count = 2;
    validate_plan_shape(&plan, &test_context(), 50).expect("shape");
    for after_auth in [false, true] {
        let mut replies = vec![get_item("ITEMABCD", 7, json!([]))];
        if after_auth {
            replies.push(reply(
                "GET",
                "/api/users/0/collections/COLLA234",
                200,
                collection_body("COLLA234", "Original", 4),
            ));
            replies.push(auth());
        }
        replies.push(reply(
            "GET",
            "/api/users/0/collections/COLLA234",
            200,
            collection_body("COLLA234", "Renamed", 5),
        ));
        let (client, handle) = scripted_server(replies);
        assert_eq!(
            execute_fixture(&client, &plan).await.items[0].status,
            "conflict"
        );
        assert!(handle.join().expect("zero patches").is_empty());
    }
    let mut edited = plan;
    edited.items[0].collections.clear();
    assert!(validate_plan_shape(&edited, &test_context(), 50).is_err());
}

#[tokio::test]
async fn report_failure_before_submit_sends_no_requests() {
    let plan = fixture_plan(vec![item("ITEMABCD"), item("ITEMBCDE")]);
    let dir = tempfile::tempdir().expect("temp");
    let (client, handle) = scripted_server(Vec::new());
    let mut report = initial_report(&plan, "hash");
    let result = execute_plan(
        &client,
        &test_context(),
        &plan,
        &mut report,
        &dir.path().join("missing-parent/report.json"),
        &dir.path().join("credentials"),
    )
    .await
    .expect("report result");
    assert_eq!(result.state, "failed");
    assert_eq!(result.not_attempted_count, 2);
    assert_eq!(
        result.error.expect("save error").code,
        "local-organize-file"
    );
    assert!(handle.join().expect("zero requests").is_empty());
}

#[tokio::test]
async fn report_failure_after_commit_keeps_disk_inflight_and_stdout_known_result() {
    let dir = tempfile::tempdir().expect("temp");
    let report_dir = dir.path().join("reports");
    let evidence_dir = dir.path().join("preserved");
    std::fs::create_dir(&report_dir).expect("report dir");
    let report_path = report_dir.join("report.json");
    let plan = fixture_plan(vec![item("ITEMABCD"), item("ITEMBCDE")]);
    let mut report = initial_report(&plan, "hash");
    write_json(&report_path, &report, false).expect("initial");
    let server = tiny_http::Server::http("127.0.0.1:0").expect("server");
    let client = LocalApiClient::with_base_url_for_tests(
        &format!("http://{}/", server.server_addr()),
        std::time::Duration::from_secs(2),
    )
    .expect("client");
    let moved = evidence_dir.clone();
    let handle = std::thread::spawn(move || {
        for expected in [
            get_item("ITEMABCD", 7, json!([])),
            auth(),
            reply("PATCH", "/api/users/0/items/ITEMABCD", 204, Value::Null),
            get_item("ITEMABCD", 8, json!([{"tag":"待读"}])),
        ] {
            let request = server
                .recv_timeout(std::time::Duration::from_secs(5))
                .expect("recv")
                .expect("request");
            assert_eq!(request.method().as_str(), expected.method);
            assert_eq!(request.url(), expected.path);
            if expected.method == "PATCH" {
                // Preserve the last durable file, then make its parent path unwritable.
                std::fs::rename(&report_dir, &moved).expect("preserve disk report");
                std::fs::write(&report_dir, b"fixture blocks directory access")
                    .expect("inject I/O failure");
            }
            respond(request, expected.status, expected.body);
        }
        assert!(
            server
                .recv_timeout(std::time::Duration::from_millis(100))
                .expect("recv")
                .is_none()
        );
    });
    let report = execute_plan(
        &client,
        &test_context(),
        &plan,
        &mut report,
        &report_path,
        &dir.path().join("credentials"),
    )
    .await
    .expect("result");
    handle.join().expect("join");
    assert_eq!(report.state, "partial");
    assert_eq!(report.items[0].status, "applied");
    assert!(report.items[0].verified);
    assert_eq!(report.items[1].status, "not-attempted");
    assert_eq!(exit_code(&report.state), 1);
    let saved: Value = serde_json::from_slice(
        &std::fs::read(evidence_dir.join("report.json")).expect("disk evidence"),
    )
    .expect("JSON");
    assert_eq!(saved["items"][0]["status"], "in-flight");
    assert_eq!(saved["unknown_count"], 1);
    assert_eq!(saved["not_attempted_count"], 1);
    assert!(!saved.to_string().contains("test-only-secret"));
    let output = CommandOutput::new(&test_context(), report.clone(), None, |_| unreachable!())
        .expect("output")
        .with_exit_code(exit_code(&report.state));
    assert_eq!(output.exit_code(), 1);
    let envelope: Value =
        serde_json::from_str(output.as_json().expect("json output")).expect("one document");
    assert_eq!(envelope["ok"], true);
    assert_eq!(envelope["data"]["items"][0]["status"], "applied");
}

// Launched only by the interruption regression. This is a test-harness process,
// not a production endpoint override or a recovery command.
#[test]
fn interrupted_execution_child() {
    let Ok(root) = std::env::var("ZOT_ORGANIZE_TEST_CHILD_DIR") else {
        return;
    };
    let root = std::path::PathBuf::from(root);
    let endpoint = std::fs::read_to_string(root.join("endpoint")).expect("endpoint");
    let client =
        LocalApiClient::with_base_url_for_tests(&endpoint, std::time::Duration::from_secs(30))
            .expect("client");
    let plan: OrganizePlan =
        serde_json::from_slice(&std::fs::read(root.join("plan.json")).expect("plan"))
            .expect("plan JSON");
    let mut report = initial_report(&plan, "hash");
    let path = root.join("report.json");
    write_json(&path, &report, false).expect("initial");
    tokio::runtime::Runtime::new()
        .expect("runtime")
        .block_on(execute_plan(
            &client,
            &test_context(),
            &plan,
            &mut report,
            &path,
            &root.join("credentials"),
        ))
        .expect("execute");
    panic!("parent should terminate this process during the second PATCH");
}

#[test]
fn killed_process_preserves_second_inflight_and_does_not_replay() {
    let dir = tempfile::tempdir().expect("temp");
    let plan = fixture_plan(vec![item("ITEMABCD"), item("ITEMBCDE"), item("ITEMCDEF")]);
    write_json(&dir.path().join("plan.json"), &plan, false).expect("plan");
    let server = tiny_http::Server::http("127.0.0.1:0").expect("server");
    std::fs::write(
        dir.path().join("endpoint"),
        format!("http://{}/", server.server_addr()),
    )
    .expect("endpoint");
    let mut child = std::process::Command::new(std::env::current_exe().expect("test exe"))
        .args([
            "--exact",
            "commands::organize::tests::interrupted_execution_child",
            "--nocapture",
        ])
        .env("ZOT_ORGANIZE_TEST_CHILD_DIR", dir.path())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("spawn child");
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        for expected in [
            get_item("ITEMABCD", 7, json!([])),
            auth(),
            reply("PATCH", "/api/users/0/items/ITEMABCD", 204, Value::Null),
            get_item("ITEMABCD", 8, json!([{"tag":"待读"}])),
            get_item("ITEMBCDE", 7, json!([])),
        ] {
            let request = server
                .recv_timeout(std::time::Duration::from_secs(10))
                .expect("recv")
                .expect("child request");
            assert_eq!(request.method().as_str(), expected.method);
            assert_eq!(request.url(), expected.path);
            respond(request, expected.status, expected.body);
        }
        let inflight = server
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("recv")
            .expect("second PATCH");
        assert_eq!(inflight.method().as_str(), "PATCH");
        assert_eq!(inflight.url(), "/api/users/0/items/ITEMBCDE");
        child.kill().expect("kill child");
        assert!(!child.wait().expect("wait").success());
        drop(inflight);
        let saved: Value = serde_json::from_slice(
            &std::fs::read(dir.path().join("report.json")).expect("disk report"),
        )
        .expect("JSON");
        assert_eq!(saved["items"][0]["status"], "applied");
        assert_eq!(saved["items"][1]["status"], "in-flight");
        assert_eq!(saved["items"][2]["status"], "not-attempted");
        assert_eq!(saved["applied_count"], 1);
        assert_eq!(saved["unknown_count"], 1);
        assert_eq!(saved["not_attempted_count"], 1);
        assert_eq!(saved["state"], "in-progress");
        assert!(
            server
                .recv_timeout(std::time::Duration::from_millis(100))
                .expect("recv")
                .is_none()
        );
    }));
    let _ = child.kill();
    let _ = child.wait();
    if let Err(panic) = outcome {
        std::panic::resume_unwind(panic);
    }
}

#[tokio::test]
async fn timed_out_patch_is_unknown_even_when_readback_matches() {
    let server = tiny_http::Server::http("127.0.0.1:0").expect("server");
    let client = LocalApiClient::with_base_url_for_tests(
        &format!("http://{}/", server.server_addr()),
        std::time::Duration::from_millis(250),
    )
    .expect("client");
    let handle = std::thread::spawn(move || {
        for expected in [get_item("ITEMABCD", 7, json!([])), auth()] {
            let request = server
                .recv_timeout(std::time::Duration::from_secs(5))
                .expect("recv")
                .expect("request");
            assert_eq!(request.method().as_str(), expected.method);
            respond(request, expected.status, expected.body);
        }
        let held = server
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("recv")
            .expect("PATCH");
        assert_eq!(held.method().as_str(), "PATCH");
        let readback = server
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("recv")
            .expect("readback after timeout");
        assert_eq!(readback.method().as_str(), "GET");
        assert_eq!(readback.url(), "/api/users/0/items/ITEMABCD");
        respond(
            readback,
            200,
            live_item("ITEMABCD", 8, json!([{"tag":"待读"}])),
        );
        drop(held);
        assert!(
            server
                .recv_timeout(std::time::Duration::from_millis(100))
                .expect("recv")
                .is_none()
        );
    });
    let plan = fixture_plan(vec![item("ITEMABCD"), item("ITEMBCDE")]);
    let report = execute_fixture(&client, &plan).await;
    assert_eq!(report.items[0].status, "unknown");
    assert_eq!(report.items[0].observed_expected_state, Some(true));
    assert!(!report.items[0].verified);
    assert_eq!(report.items[1].status, "not-attempted");
    assert_eq!(exit_code(&report.state), 1);
    handle.join().expect("join");
}
