pub(crate) mod bridge;
pub(crate) mod collection;
pub(crate) mod config;
pub(crate) mod doctor;
pub(crate) mod graph;
pub(crate) mod item;
pub(crate) mod library;
pub(crate) mod library_dedupe;
pub(crate) mod mcp;
pub(crate) mod sync;
pub(crate) mod workspace;

use anyhow::Result;
use clap::CommandFactory;

use crate::cli::{Cli, Commands};
use crate::context::AppContext;
use crate::output::CommandOutput;

pub(crate) async fn dispatch(ctx: &AppContext, command: Commands) -> Result<()> {
    let output = match command {
        Commands::Doctor => doctor::handle(ctx).await?,
        Commands::Config { command } => config::handle(ctx, command).await?,
        Commands::Bridge { command } => bridge::handle(ctx, command).await?,
        Commands::Library { command } => library::handle(ctx, command).await?,
        Commands::Item { command } => item::handle(ctx, command).await?,
        Commands::Collection { command } => collection::handle(ctx, command).await?,
        Commands::Graph(args) => graph::handle(ctx, args).await?,
        Commands::Workspace { command } => workspace::handle(ctx, command).await?,
        Commands::Sync { command } => sync::handle(ctx, command).await?,
        Commands::Mcp { command } => mcp::handle(ctx, command).await?,
        Commands::Completions { shell } => {
            clap_complete::generate(shell, &mut Cli::command(), "zot", &mut std::io::stdout());
            CommandOutput::silent()
        }
    };
    output.emit();
    Ok(())
}
