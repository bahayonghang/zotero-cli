//! Minimal localhost HTTP server for the interactive graph viewer.
//!
//! The graph is built once at startup; this server only hands out the static
//! single-page app plus one `graph.json` snapshot. It binds `127.0.0.1`,
//! never an external interface, and answers only GET requests.

use std::io::Cursor;
use std::sync::Arc;

use anyhow::{Result, anyhow};
use tiny_http::{Header, Response, Server};
use zot_core::GraphOptions;

use crate::cli::GraphServeArgs;
use crate::context::AppContext;
use crate::util::{open_target, run_local};

const INDEX_HTML: &str = include_str!("../../../assets/graph/index.html");
const APP_JS: &str = include_str!("../../../assets/graph/app.js");
const CYTOSCAPE_JS: &str = include_str!("../../../assets/graph/cytoscape.min.js");

pub(crate) async fn run(ctx: &AppContext, args: GraphServeArgs) -> Result<()> {
    let edge_budget = super::validate_edge_budget(args.edge_budget)?;
    let opts = GraphOptions {
        collection: args.collection.clone(),
        edge_budget,
        ..GraphOptions::default()
    };
    let graph = run_local(ctx.config.clone(), ctx.scope.clone(), move |library| {
        library.build_knowledge_graph(&opts)
    })
    .await?;
    let node_count = graph.nodes.len();
    let edge_count = graph.edges.len();
    let graph_json = serde_json::to_string(&graph)?;

    let server = Arc::new(bind_server(args.port)?);
    let url = match server.server_addr().to_ip() {
        Some(addr) => format!("http://{addr}"),
        None => format!("http://127.0.0.1:{}", args.port),
    };

    println!("zot graph: serving {node_count} nodes / {edge_count} edges at {url}");
    println!("Press Ctrl-C to stop.");
    if !args.no_open
        && let Err(err) = open_target(&url)
    {
        eprintln!("Could not open the browser automatically: {err}");
    }

    let worker = {
        let server = Arc::clone(&server);
        let graph_json = graph_json.clone();
        std::thread::spawn(move || serve_loop(&server, &graph_json))
    };

    tokio::signal::ctrl_c().await?;
    println!("\nShutting down.");
    server.unblock();
    if worker.join().is_err() {
        eprintln!("Server worker thread terminated abnormally.");
    }
    Ok(())
}

/// Bind `127.0.0.1:port`, falling back to an OS-assigned free port if that
/// port is already in use.
fn bind_server(port: u16) -> Result<Server> {
    if let Ok(server) = Server::http(("127.0.0.1", port)) {
        return Ok(server);
    }
    Server::http(("127.0.0.1", 0)).map_err(|err| anyhow!("failed to bind local server: {err}"))
}

fn serve_loop(server: &Server, graph_json: &str) {
    for request in server.incoming_requests() {
        let path = request.url().split('?').next().unwrap_or("/").to_string();
        let _ = request.respond(route(&path, graph_json));
    }
}

fn route(path: &str, graph_json: &str) -> Response<Cursor<Vec<u8>>> {
    match path {
        "/" => asset(INDEX_HTML, "text/html; charset=utf-8"),
        "/app.js" => asset(APP_JS, "application/javascript; charset=utf-8"),
        "/cytoscape.min.js" => asset(CYTOSCAPE_JS, "application/javascript; charset=utf-8"),
        "/graph.json" => asset(graph_json, "application/json; charset=utf-8"),
        _ => Response::from_string("Not found").with_status_code(404),
    }
}

fn asset(body: &str, content_type: &'static str) -> Response<Cursor<Vec<u8>>> {
    let mut response = Response::from_string(body);
    if let Ok(header) = Header::from_bytes(b"Content-Type".as_ref(), content_type.as_bytes()) {
        response = response.with_header(header);
    }
    response
}
