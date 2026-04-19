pub(crate) mod collection;
pub(crate) mod config;
pub(crate) mod doctor;
pub(crate) mod item;
pub(crate) mod library;
pub(crate) mod mcp;
pub(crate) mod sync;
pub(crate) mod workspace;

use anyhow::Result;

use crate::cli::Commands;
use crate::context::AppContext;

pub(crate) async fn dispatch(ctx: &AppContext, command: Commands) -> Result<()> {
    match command {
        Commands::Doctor => doctor::handle(ctx).await,
        Commands::Config { command } => config::handle(ctx, command).await,
        Commands::Library { command } => library::handle(ctx, command).await,
        Commands::Item { command } => item::handle(ctx, command).await,
        Commands::Collection { command } => collection::handle(ctx, command).await,
        Commands::Workspace { command } => workspace::handle(ctx, command).await,
        Commands::Sync { command } => sync::handle(ctx, command).await,
        Commands::Mcp { command } => mcp::handle(ctx, command).await,
    }
}
