use anyhow::Result;
use zot_core::GraphOptions;

use crate::cli::{GraphArgs, GraphCommand};
use crate::context::AppContext;
use crate::format::{EnvelopeMetaSeed, print_enveloped, print_graph_summary};

mod server;

pub(crate) async fn handle(ctx: &AppContext, args: GraphArgs) -> Result<()> {
    match args.command {
        Some(GraphCommand::Serve(serve)) => server::run(ctx, serve).await,
        None => build_and_print(ctx, args.collection).await,
    }
}

async fn build_and_print(ctx: &AppContext, collection: Option<String>) -> Result<()> {
    let opts = GraphOptions {
        collection,
        ..GraphOptions::default()
    };
    let graph = ctx.local_library()?.build_knowledge_graph(&opts)?;
    if ctx.json {
        let count = graph.nodes.len();
        print_enveloped(
            ctx,
            &graph,
            Some(EnvelopeMetaSeed {
                count: Some(count),
                total: Some(count),
            }),
        )?;
    } else {
        print_graph_summary(&graph);
    }
    Ok(())
}
