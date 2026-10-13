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
