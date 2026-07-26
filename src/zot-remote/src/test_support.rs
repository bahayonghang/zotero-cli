//! Test-only fake HTTP server for exercising remote clients without a live
//! service. Binds `127.0.0.1:0` and replays a fixed script of responses, so a
//! client constructed via `with_base_url` can be driven end-to-end (request
//! shape and response mapping) with no network and no persistence.

use std::thread::JoinHandle;

use tiny_http::{Header, Response, Server};

pub(crate) type ScriptedResponse = (u16, &'static str, Vec<(&'static str, &'static str)>);

/// One request observed by the fake server: HTTP method, url (path + query),
/// and the headers as sent on the wire.
pub(crate) struct Captured {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
}

impl Captured {
    /// Case-insensitive header lookup. HTTP header names are case-insensitive
    /// and reqwest lowercases them on the wire, so callers assert by intent.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(field, _)| field.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

/// Bind a fake server on an OS-assigned port and answer exactly
/// `responses.len()` requests with the given `(status, body)` pairs, in order.
/// Returns the base URL to point a client at and a handle that yields the
/// captured requests once the scripted responses are exhausted.
///
/// The response count must match the client's request count; a shorter script
/// leaves the worker blocked in `recv`, which surfaces as a hung `join`.
pub(crate) fn spawn_server(
    responses: Vec<(u16, &'static str)>,
) -> (String, JoinHandle<Vec<Captured>>) {
    spawn_server_with_headers(
        responses
            .into_iter()
            .map(|(status, body)| (status, body, Vec::new()))
            .collect(),
    )
}

/// Header-aware variant used by retry, redirect, and content validation
/// tests. Header names and values are static fixture data.
pub(crate) fn spawn_server_with_headers(
    responses: Vec<ScriptedResponse>,
) -> (String, JoinHandle<Vec<Captured>>) {
    let server = Server::http(("127.0.0.1", 0)).expect("bind fake server");
    let port = match server.server_addr().to_ip() {
        Some(addr) => addr.port(),
        None => panic!("fake server bound without an IP address"),
    };
    let base_url = format!("http://127.0.0.1:{port}");
    let handle = std::thread::spawn(move || {
        let mut captured = Vec::new();
        for (status, body, headers) in responses {
            let mut request = match server.recv() {
                Ok(request) => request,
                Err(_) => break,
            };
            // Drain the body so the connection is fully consumed before we
            // reply, even when the test does not assert on the payload.
            let mut drained = Vec::new();
            let _ = request.as_reader().read_to_end(&mut drained);
            captured.push(Captured {
                method: request.method().as_str().to_string(),
                url: request.url().to_string(),
                headers: request
                    .headers()
                    .iter()
                    .map(|header| (header.field.to_string(), header.value.to_string()))
                    .collect(),
            });
            let mut response = Response::from_string(body).with_status_code(status);
            for (name, value) in headers {
                response.add_header(
                    Header::from_bytes(name.as_bytes(), value.as_bytes())
                        .expect("valid fake-server response header"),
                );
            }
            let _ = request.respond(response);
        }
        captured
    });
    (base_url, handle)
}
