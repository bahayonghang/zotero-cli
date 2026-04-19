mod cli;
mod commands;
mod context;
mod format;
mod util;

use anyhow::Result;
use clap::Parser;

use crate::cli::Cli;
use crate::context::AppContext;
use crate::format::print_error;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let json = cli.json;
    if let Err(err) = run(cli).await {
        if let Some(zot_error) = err.downcast_ref::<zot_core::ZotError>() {
            print_error(zot_error, json)?;
            std::process::exit(1);
        }
        eprintln!("{err}");
        std::process::exit(1);
    }
    Ok(())
}

async fn run(cli: Cli) -> Result<()> {
    let ctx = AppContext::from_cli(&cli)?;
    commands::dispatch(&ctx, cli.command).await
}
