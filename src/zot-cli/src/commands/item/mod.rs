pub(crate) mod annotation;
pub(crate) mod merge;
pub(crate) mod note;
pub(crate) mod read;
pub(crate) mod scite;
pub(crate) mod tag;
pub(crate) mod write;

use anyhow::Result;

use crate::cli::ItemCommand;
use crate::context::AppContext;
use crate::output::CommandOutput;

/// Migration-era adapter for item subcommands that still return `Result<()>`
/// and print their own output. Removed once every arm returns `CommandOutput`.
async fn legacy<F>(fut: F) -> Result<CommandOutput>
where
    F: std::future::Future<Output = Result<()>>,
{
    fut.await.map(|()| CommandOutput::silent())
}

pub(crate) async fn handle(ctx: &AppContext, command: ItemCommand) -> Result<CommandOutput> {
    match command {
        ItemCommand::Get(args) => legacy(read::handle_get(ctx, args)).await,
        ItemCommand::Related(args) => legacy(read::handle_related(ctx, args)).await,
        ItemCommand::Open(args) => legacy(read::handle_open(ctx, args)).await,
        ItemCommand::Pdf(args) => legacy(read::handle_pdf(ctx, args)).await,
        ItemCommand::Fulltext(args) => legacy(read::handle_pdf(ctx, args)).await,
        ItemCommand::Children(args) => legacy(read::handle_children(ctx, args)).await,
        ItemCommand::Download(args) => legacy(read::handle_download(ctx, args)).await,
        ItemCommand::Deleted(args) => legacy(read::handle_deleted(ctx, args)).await,
        ItemCommand::Versions(args) => legacy(read::handle_versions(ctx, args)).await,
        ItemCommand::Outline(args) => legacy(read::handle_outline(ctx, &args.key)).await,
        ItemCommand::Export(args) => legacy(read::handle_export(ctx, args)).await,
        ItemCommand::Cite(args) => legacy(read::handle_cite(ctx, args)).await,
        ItemCommand::Create(args) => write::handle_create(ctx, args).await,
        ItemCommand::AddDoi(args) => write::handle_add_doi(ctx, args).await,
        ItemCommand::AddUrl(args) => write::handle_add_url(ctx, args).await,
        ItemCommand::AddFile(args) => write::handle_add_file(ctx, args).await,
        ItemCommand::Merge(args) => write::handle_merge(ctx, args).await,
        ItemCommand::Update(args) => write::handle_update(ctx, args).await,
        ItemCommand::Trash(args) => write::handle_trash(ctx, args).await,
        ItemCommand::Restore(args) => write::handle_restore(ctx, args).await,
        ItemCommand::Attach(args) => write::handle_attach(ctx, args).await,
        ItemCommand::Note { command } => note::handle(ctx, command).await,
        ItemCommand::Tag { command } => tag::handle(ctx, command).await,
        ItemCommand::Annotation { command } => annotation::handle(ctx, command).await,
        ItemCommand::Scite { command } => scite::handle(ctx, command).await,
    }
}
