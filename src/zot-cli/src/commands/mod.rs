pub(crate) mod collection;
pub(crate) mod config;
pub(crate) mod doctor;
pub(crate) mod graph;
pub(crate) mod item;
pub(crate) mod library;
pub(crate) mod mcp;
pub(crate) mod sync;
pub(crate) mod workspace;

use anyhow::Result;
use clap::CommandFactory;

use crate::cli::{Cli, Commands};
use crate::context::AppContext;
use crate::output::CommandOutput;

/// Migration-era adapter: wraps a not-yet-migrated handler that still prints
/// its own output and returns `Result<()>`. Removed in batch 6 once all
/// handlers return `CommandOutput`.
async fn legacy<F>(fut: F) -> Result<CommandOutput>
where
    F: std::future::Future<Output = Result<()>>,
{
    fut.await.map(|()| CommandOutput::silent())
}

pub(crate) async fn dispatch(ctx: &AppContext, command: Commands) -> Result<()> {
    let output = match command {
        Commands::Doctor => legacy(doctor::handle(ctx)).await?,
        Commands::Config { command } => legacy(config::handle(ctx, command)).await?,
        Commands::Library { command } => legacy(library::handle(ctx, command)).await?,
        Commands::Item { command } => legacy(item::handle(ctx, command)).await?,
        Commands::Collection { command } => legacy(collection::handle(ctx, command)).await?,
        Commands::Graph(args) => graph::handle(ctx, args).await?,
        Commands::Workspace { command } => legacy(workspace::handle(ctx, command)).await?,
        Commands::Sync { command } => legacy(sync::handle(ctx, command)).await?,
        Commands::Mcp { command } => legacy(mcp::handle(ctx, command)).await?,
        Commands::Completions { shell } => {
            clap_complete::generate(shell, &mut Cli::command(), "zot", &mut std::io::stdout());
            CommandOutput::silent()
        }
    };
    output.emit();
    Ok(())
}
