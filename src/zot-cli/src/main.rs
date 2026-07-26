mod app_error;
mod cli;
mod commands;
mod context;
mod format;
mod output;
mod util;

use std::ffi::OsString;

use anyhow::Result;
use clap::Parser;
use clap::error::ErrorKind;

use crate::app_error::AppError;
use crate::cli::Cli;
use crate::context::AppContext;
use crate::format::print_error;

#[tokio::main]
async fn main() {
    let args = std::env::args_os().collect::<Vec<_>>();
    let json_requested = flag_requested(&args, "--json");
    let verbose_requested = flag_requested(&args, "--verbose");
    let cli = match Cli::try_parse_from(&args) {
        Ok(cli) => cli,
        Err(error) => {
            if json_requested && !is_documentation_action(error.kind()) {
                let exit_code = error.exit_code();
                let error = AppError::cli_parse(error.to_string());
                emit_error(&error, true, verbose_requested, None);
                std::process::exit(exit_code);
            }
            error.exit();
        }
    };
    let mut json = cli.json;
    let verbose = cli.verbose;
    let mut profile = cli.profile.clone();
    if let Err(err) = run(cli, &mut json, &mut profile).await {
        let error = AppError::runtime(err);
        emit_error(&error, json, verbose, profile.as_deref());
        std::process::exit(1);
    }
}

async fn run(mut cli: Cli, json: &mut bool, profile: &mut Option<String>) -> Result<()> {
    let ctx = AppContext::from_cli(&cli)?;
    *json = ctx.json;
    *profile = ctx.profile.clone();
    cli.json = ctx.json;
    cli.validate_output_protocol()?;
    cli.resolve_effective_options(ctx.config.output.limit)?;
    commands::dispatch(&ctx, cli.command).await
}

fn flag_requested(args: &[OsString], flag: &str) -> bool {
    args.iter().skip(1).any(|arg| arg == flag)
}

fn is_documentation_action(kind: ErrorKind) -> bool {
    matches!(kind, ErrorKind::DisplayHelp | ErrorKind::DisplayVersion)
}

fn emit_error(error: &AppError, json: bool, verbose: bool, profile: Option<&str>) {
    if let Err(render_error) = print_error(error, json, verbose, profile) {
        eprintln!("Failed to render error output: {render_error}");
    }
}
