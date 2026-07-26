use anyhow::Result;
use zot_core::GraphOptions;

use crate::cli::{GraphArgs, GraphCommand};
use crate::context::AppContext;
use crate::format::{EnvelopeMetaSeed, print_graph_summary};
use crate::output::CommandOutput;
use crate::util::run_local;

mod server;

pub(crate) async fn handle(ctx: &AppContext, args: GraphArgs) -> Result<CommandOutput> {
    match args.command {
        Some(GraphCommand::Serve(serve)) => {
            server::run(ctx, serve).await?;
            Ok(CommandOutput::silent())
        }
        None => build_and_print(ctx, args.collection, args.edge_budget).await,
    }
}

async fn build_and_print(
    ctx: &AppContext,
    collection: Option<String>,
    edge_budget: usize,
) -> Result<CommandOutput> {
    let edge_budget = validate_edge_budget(edge_budget)?;
    let opts = GraphOptions {
        collection,
        edge_budget,
        ..GraphOptions::default()
    };
    let graph = run_local(ctx.config.clone(), ctx.scope.clone(), move |library| {
        library.build_knowledge_graph(&opts)
    })
    .await?;
    let count = graph.nodes.len();
    let seed = Some(EnvelopeMetaSeed {
        count: Some(count),
        total: Some(count),
        trash_policy: None,
    });
    CommandOutput::new(ctx, graph, seed, print_graph_summary)
}

pub(super) fn validate_edge_budget(edge_budget: usize) -> zot_core::ZotResult<usize> {
    if edge_budget == 0 {
        return Err(zot_core::ZotError::InvalidInput {
            code: "graph-edge-budget".to_string(),
            message: "Graph edge budget must be greater than zero".to_string(),
            hint: Some("Pass --edge-budget with a positive integer".to_string()),
        });
    }
    Ok(edge_budget)
}

#[cfg(test)]
mod tests {
    #[test]
    fn zero_edge_budget_fails_before_local_database_open() {
        let error = super::validate_edge_budget(0).expect_err("zero budget must fail");
        assert_eq!(error.payload().code, "graph-edge-budget");
    }
}
