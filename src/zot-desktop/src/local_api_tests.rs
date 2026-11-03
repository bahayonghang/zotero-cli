use std::thread::JoinHandle;
use tiny_http::{Header, Response as FakeResponse, Server};

use super::*;

type Reply = (u16, Value, Vec<(&'static str, &'static str)>);
type Captured = (String, String, Vec<(String, String)>, Value);

fn server(responses: Vec<Reply>) -> (LocalApiClient, JoinHandle<Vec<Captured>>) {
    let server = Server::http("127.0.0.1:0").expect("server");
    let client = LocalApiClient::with_base_url_for_tests(
        &format!("http://{}/", server.server_addr()),
        Duration::from_secs(2),
    )
    .expect("client");
    let handle = std::thread::spawn(move || {
        let mut captured = Vec::new();
        for (status, body, headers) in responses {
            let mut request = server
                .recv_timeout(Duration::from_secs(3))
                .expect("recv")
                .expect("request");
            let mut text = String::new();
            request.as_reader().read_to_string(&mut text).expect("body");
            captured.push((
                request.method().to_string(),
                request.url().to_string(),
                request
                    .headers()
                    .iter()
                    .map(|h| (h.field.to_string().to_lowercase(), h.value.to_string()))
                    .collect(),
                serde_json::from_str(&text).unwrap_or(Value::Null),
            ));
            let mut response = FakeResponse::from_string(body.to_string()).with_status_code(status);
            for (name, value) in headers {
                response.add_header(Header::from_bytes(name, value).expect("header"));
            }
            request.respond(response).expect("response");
        }
        captured
    });
    (client, handle)
}

#[tokio::test]
async fn probe_requires_version_api_and_identity() {
    for (version, api, id, expected) in [
        ("10.0.6", "3", "server", true),
        ("9.0.6", "3", "server", false),
        ("10.0.6", "2", "server", false),
        ("10.0.6", "3", "", false),
    ] {
        let (client, handle) = server(vec![(
            200,
            Value::Null,
            vec![
                ("X-Zotero-Version", version),
                ("Zotero-API-Version", api),
                (SERVER_ID, id),
            ],
        )]);
        let probe = client.probe().await.expect("probe");
        assert_eq!(probe.supported, expected);
        assert_eq!(probe.writable_server_id().is_ok(), expected);
        assert_eq!(handle.join().expect("join")[0].0, "GET");
    }
}

#[tokio::test]
async fn group_patch_binds_local_identity_version_and_key() {
    let (client, handle) = server(vec![(204, Value::Null, vec![(SERVER_ID, "server")])]);
    let patch = json!({"collections":["COLLB234","COLLC234"]})
        .as_object()
        .expect("map")
        .clone();
    assert_eq!(
        client
            .patch_item(
                "server",
                &LibraryScope::Group { group_id: 42 },
                "ITEMABCD",
                7,
                &patch,
                &"local-secret".into()
            )
            .await
            .expect("patch"),
        PatchReceipt::Accepted
    );
    let captured = handle.join().expect("join");
    let (method, url, headers, body) = &captured[0];
    assert_eq!(method, "PATCH");
    assert_eq!(url, "/api/groups/42/items/ITEMABCD");
    for (key, value) in [
        ("zotero-server-id", "server"),
        ("zotero-api-key", "local-secret"),
        ("if-unmodified-since-version", "7"),
    ] {
        assert!(headers.contains(&(key.to_string(), value.to_string())));
    }
    assert_eq!(
        body,
        &json!({"collections":["COLLB234","COLLC234"],"version":7})
    );
}

#[tokio::test]
async fn tag_patch_keeps_complete_array_unknown_properties_and_headers() {
    let tags = json!([
        {"tag":"自动标签","type":1,"meta":{"source":"bbt"}},
        {"tag":"手工 标签","type":0}
    ]);
    let patch = json!({"tags": tags.clone()})
        .as_object()
        .expect("map")
        .clone();
    let (client, handle) = server(vec![(204, Value::Null, vec![(SERVER_ID, "server")])]);
    assert_eq!(
        client
            .patch_item(
                "server",
                &LibraryScope::User,
                "ITEMABCD",
                7,
                &patch,
                &"local-secret".into()
            )
            .await
            .expect("patch"),
        PatchReceipt::Accepted
    );
    let captured = handle.join().expect("join");
    let (method, url, headers, body) = &captured[0];
    assert_eq!(method, "PATCH");
    assert_eq!(url, "/api/users/0/items/ITEMABCD");
    for (key, value) in [
        ("zotero-server-id", "server"),
        ("zotero-api-key", "local-secret"),
        ("if-unmodified-since-version", "7"),
        ("zotero-api-version", "3"),
    ] {
        assert!(
            headers.contains(&(key.to_string(), value.to_string())),
            "missing {key}"
        );
    }
    assert!(!headers.iter().any(|(key, _)| key == "zotero-write-token"));
    assert_eq!(body, &json!({"tags": tags, "version": 7}));
}

fn collection_object(key: &str, name: &str, parent: Value) -> Value {
    json!({
        "key": key,
        "version": 3,
        "data": {
            "key": key,
            "version": 3,
            "name": name,
            "parentCollection": parent
        }
    })
}

#[tokio::test]
async fn collection_snapshot_requires_library_version_and_closes_pagination() {
    let (client, handle) = server(vec![(
        200,
        json!([]),
        vec![
            (SERVER_ID, "server"),
            ("Last-Modified-Version", "0"),
            ("Total-Results", "0"),
        ],
    )]);
    let snapshot = client
        .collection_snapshot("server", &LibraryScope::User)
        .await
        .expect("empty");
    assert_eq!(snapshot.library_version, 0);
    assert!(snapshot.collections.is_empty());
    assert_eq!(
        handle.join().expect("join")[0].1,
        "/api/users/0/collections"
    );

    let (client, handle) = server(vec![(
        200,
        json!([collection_object("COLLA234", "Root", Value::Bool(false))]),
        vec![(SERVER_ID, "server"), ("Total-Results", "1")],
    )]);
    let error = client
        .collection_snapshot("server", &LibraryScope::User)
        .await
        .expect_err("missing version");
    assert_eq!(error.payload().code, "local-api-protocol");
    handle.join().expect("join");

    let (client, handle) = server(vec![
        (
            200,
            json!([collection_object("COLLA234", "Root", Value::Bool(false))]),
            vec![
                (SERVER_ID, "server"),
                ("Last-Modified-Version", "12"),
                ("Total-Results", "2"),
                (
                    "Link",
                    "<http://example.test/api/users/0/collections?start=1>; rel=\"next\"",
                ),
            ],
        ),
        (
            200,
            json!([collection_object("COLLB234", "Child", json!("COLLA234"))]),
            vec![
                (SERVER_ID, "server"),
                ("Last-Modified-Version", "12"),
                ("Total-Results", "2"),
            ],
        ),
    ]);
    let snapshot = client
        .collection_snapshot("server", &LibraryScope::Group { group_id: 42 })
        .await
        .expect("pages");
    assert_eq!(snapshot.library_version, 12);
    assert_eq!(snapshot.collections.len(), 2);
    assert_eq!(snapshot.collections[1].parent.as_deref(), Some("COLLA234"));
    let urls = handle.join().expect("join");
    assert_eq!(urls[0].1, "/api/groups/42/collections");
    assert_eq!(urls[1].1, "/api/groups/42/collections?start=1");

    let (client, handle) = server(vec![
        (
            200,
            json!([collection_object("COLLA234", "Root", Value::Bool(false))]),
            vec![
                (SERVER_ID, "server"),
                ("Last-Modified-Version", "12"),
                ("Link", "</api/users/0/collections?start=1>; rel=\"next\""),
            ],
        ),
        (
            200,
            json!([]),
            vec![(SERVER_ID, "server"), ("Last-Modified-Version", "13")],
        ),
    ]);
    let error = client
        .collection_snapshot("server", &LibraryScope::User)
        .await
        .expect_err("version changed");
    assert_eq!(error.payload().code, "local-api-conflict");
    assert_eq!(handle.join().expect("join").len(), 2);

    let (client, handle) = server(vec![(
        200,
        json!([collection_object("COLLA234", "Root", Value::Bool(false))]),
        vec![
            (SERVER_ID, "server"),
            ("Last-Modified-Version", "12"),
            ("Total-Results", "2"),
        ],
    )]);
    let error = client
        .collection_snapshot("server", &LibraryScope::User)
        .await
        .expect_err("incomplete");
    assert_eq!(error.payload().code, "local-api-protocol");
    handle.join().expect("join");
}

#[tokio::test]
async fn create_collection_posts_one_object_without_write_token() {
    let (client, handle) = server(vec![(
        200,
        json!({
            "successful": {"0": {"key": "NEWCOLL1", "data": {"key": "NEWCOLL1"}}},
            "unchanged": {},
            "failed": {}
        }),
        vec![(SERVER_ID, "server")],
    )]);
    let receipt = client
        .create_collection(
            "server",
            &LibraryScope::User,
            12,
            "待读",
            None,
            &"local-secret".into(),
        )
        .await
        .expect("create");
    assert_eq!(
        receipt,
        CreateReceipt::Accepted {
            key: "NEWCOLL1".to_string()
        }
    );
    let (method, url, headers, body) = &handle.join().expect("join")[0];
    assert_eq!(method, "POST");
    assert_eq!(url, "/api/users/0/collections");
    assert_eq!(body, &json!([{"name": "待读", "parentCollection": false}]));
    for (key, value) in [
        ("zotero-server-id", "server"),
        ("zotero-api-key", "local-secret"),
        ("if-unmodified-since-version", "12"),
    ] {
        assert!(headers.contains(&(key.to_string(), value.to_string())));
    }
    assert!(!headers.iter().any(|(key, _)| key == "zotero-write-token"));

    let (client, handle) = server(vec![(
        200,
        json!({
            "successful": {},
            "unchanged": {},
            "failed": {"0": {"code": 400, "message": "bad name"}}
        }),
        vec![(SERVER_ID, "server")],
    )]);
    let error = client
        .create_collection(
            "server",
            &LibraryScope::User,
            12,
            "Child",
            Some("PARENT12"),
            &"local-secret".into(),
        )
        .await
        .expect_err("failed object");
    assert_eq!(error.payload().code, "local-api-http");
    assert_eq!(handle.join().expect("join").len(), 1);

    for (status, id, expected) in [
        (500, "server", CreateReceipt::Unknown),
        (200, "other", CreateReceipt::Unknown),
        (204, "server", CreateReceipt::Unknown),
    ] {
        let (client, handle) = server(vec![(status, Value::Null, vec![(SERVER_ID, id)])]);
        assert_eq!(
            client
                .create_collection(
                    "server",
                    &LibraryScope::User,
                    12,
                    "待读",
                    None,
                    &"secret".into()
                )
                .await
                .expect("receipt"),
            expected
        );
        assert_eq!(handle.join().expect("join").len(), 1);
    }
}

#[tokio::test]
async fn authorization_is_redacted_and_instance_bound() {
    let (client, handle) = server(vec![(
        200,
        json!({"key":"auth-secret-canary", "remember":false}),
        vec![(SERVER_ID, "server")],
    )]);
    let auth = client.authorize("server").await.expect("authorize");
    assert!(!auth.remember);
    assert!(!format!("{auth:?}").contains("auth-secret-canary"));
    let captured = handle.join().expect("join");
    assert_eq!(captured[0].1, "/api/local/authorize");
    assert_eq!(captured[0].3, json!({"appName":"zot CLI"}));
    assert!(!captured[0].2.iter().any(|(k, _)| k == "zotero-api-key"));
}

#[test]
fn create_receipt_rejects_contradictory_or_malformed_result_buckets() {
    for body in [
        json!({"successful":{"0":"NEWCOLL1"},"unchanged":{"0":"NEWCOLL1"},"failed":{}}),
        json!({"successful":{"0":"NEWCOLL1"},"unchanged":{"1":"OTHER234"},"failed":{}}),
        json!({"successful":{"0":"NEWCOLL1"},"unchanged":[],"failed":{}}),
        json!({"successful":{},"unchanged":{},"failed":{"0":null}}),
        json!({"successful":{},"unchanged":{"0":"NEWCOLL1"},"failed":{"0":{"code":400}}}),
    ] {
        assert_eq!(
            interpret_create_body(&body).expect("unreliable response"),
            CreateReceipt::Unknown
        );
    }
}

#[tokio::test]
async fn refuses_redirects_conflicts_permissions_and_missing_preconditions() {
    for (status, code) in [
        (301, "local-api-http"),
        (401, "local-api-unauthorized"),
        (403, "local-api-forbidden"),
        (412, "local-api-conflict"),
        (428, "local-api-precondition"),
        (429, "local-api-rate-limit"),
    ] {
        let (client, handle) = server(vec![(
            status,
            json!({"key":"do-not-leak"}),
            vec![("Location", "http://example.com/"), ("Retry-After", "10")],
        )]);
        let err = client.authorize("server").await.expect_err("rejected");
        assert_eq!(err.payload().code, code);
        assert!(!format!("{err:?}").contains("do-not-leak"));
        assert_eq!(err.payload().hint.as_deref(), Some("Retry-After: 10"));
        assert_eq!(handle.join().expect("join").len(), 1);
    }
}

#[tokio::test]
async fn read_rejects_wrong_instance_or_version_shape() {
    for (id, version) in [("other", 7), ("server", 8)] {
        let (client, handle) = server(vec![(
            200,
            json!({"key":"ITEMABCD", "version":7,"data":{"key":"ITEMABCD","version":version}}),
            vec![(SERVER_ID, id)],
        )]);
        let err = client
            .item("server", &LibraryScope::User, "ITEMABCD")
            .await
            .expect_err("bad response");
        assert!(matches!(
            err.payload().code.as_str(),
            "local-api-conflict" | "local-api-protocol"
        ));
        handle.join().expect("join");
    }
}

#[tokio::test]
async fn server_failure_and_wrong_success_identity_are_unknown_not_retried() {
    for (status, id) in [(500, "server"), (204, "other")] {
        let (client, handle) = server(vec![(status, Value::Null, vec![(SERVER_ID, id)])]);
        let receipt = client
            .patch_item(
                "server",
                &LibraryScope::User,
                "ITEMABCD",
                7,
                &Map::new(),
                &"secret".into(),
            )
            .await
            .expect("receipt");
        assert_eq!(receipt, PatchReceipt::Unknown);
        assert_eq!(handle.join().expect("join").len(), 1);
    }
}

#[test]
fn rejects_external_origins_credentials_and_path_overrides() {
    for url in [
        "https://localhost/",
        "http://example.com/",
        "http://user:secret@localhost/",
        "http://localhost/other/",
        "http://localhost/?key=secret",
    ] {
        assert!(LocalApiClient::with_base_url_for_tests(url, Duration::from_secs(1)).is_err());
    }
    for key in ["../items", "SHORT", "ABCD?EFG"] {
        assert!(validate_key(key).is_err());
    }
}

#[tokio::test]
async fn timed_out_submission_is_unknown_and_is_never_retried() {
    let server = Server::http("127.0.0.1:0").expect("server");
    let client = LocalApiClient::with_base_url_for_tests(
        &format!("http://{}/", server.server_addr()),
        Duration::from_millis(100),
    )
    .expect("client");
    let handle = std::thread::spawn(move || {
        let request = server
            .recv_timeout(Duration::from_secs(3))
            .expect("recv")
            .expect("request");
        assert_eq!(request.method().as_str(), "PATCH");
        // Keep the request open past the client deadline, without a response.
        assert!(
            server
                .recv_timeout(Duration::from_millis(300))
                .expect("recv")
                .is_none()
        );
        drop(request);
    });
    let receipt = client
        .patch_item(
            "server",
            &LibraryScope::User,
            "ITEMABCD",
            7,
            &Map::new(),
            &"test-key".into(),
        )
        .await
        .expect("receipt");
    assert_eq!(receipt, PatchReceipt::Unknown);
    handle.join().expect("join");
}
