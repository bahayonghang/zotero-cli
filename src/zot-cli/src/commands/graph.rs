use anyhow::Result;
use zot_core::GraphOptions;

use crate::cli::{GraphArgs, GraphCommand};
use crate::context::AppContext;
use crate::format::{EnvelopeMetaSeed, print_graph_summary};
use crate::output::CommandOutput;

mod server;

pub(crate) async fn handle(ctx: &AppContext, args: GraphArgs) -> Result<CommandOutput> {
    match args.command {
        Some(GraphCommand::Serve(serve)) => {
            server::run(ctx, serve).await?;
            Ok(CommandOutput::silent())
        }
        None => build_and_print(ctx, args.collection).await,
    }
}

async fn build_and_print(ctx: &AppContext, collection: Option<String>) -> Result<CommandOutput> {
    let opts = GraphOptions {
        collection,
        ..GraphOptions::default()
    };
    let graph = ctx.local_library()?.build_knowledge_graph(&opts)?;
    let count = graph.nodes.len();
    let seed = Some(EnvelopeMetaSeed {
        count: Some(count),
        total: Some(count),
    });
    CommandOutput::new(ctx, graph, seed, print_graph_summary)
}
