pub(crate) mod collection;
pub(crate) mod config;
mod desktop_read;
pub(crate) mod doctor;
pub(crate) mod graph;
pub(crate) mod item;
pub(crate) mod library;
pub(crate) mod library_dedupe;
mod local_create;
mod local_credentials;
mod local_delete;
pub(crate) mod local_write;
pub(crate) mod mcp;
mod organize;
pub(crate) mod sync;
pub(crate) mod workspace;

use anyhow::Result;
use clap::CommandFactory;

use crate::cli::{Cli, Commands};
use crate::context::AppContext;
use crate::output::CommandOutput;

pub(crate) async fn dispatch(ctx: &AppContext, command: Commands) -> Result<i32> {
    let output = match command {
        Commands::Doctor => doctor::handle(ctx).await?,
        Commands::Config { command } => config::handle(ctx, command).await?,
        Commands::Library { command } => library::handle(ctx, command).await?,
        // Keep the large nested item future off the Windows main-thread stack.
        Commands::Item { command } => Box::pin(item::handle(ctx, command)).await?,
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
    let code = output.exit_code();
    output.emit();
    Ok(code)
}
