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

pub(crate) async fn handle(ctx: &AppContext, command: ItemCommand) -> Result<()> {
    match command {
        ItemCommand::Get(args) => read::handle_get(ctx, args).await,
        ItemCommand::Related(args) => read::handle_related(ctx, args).await,
        ItemCommand::Open(args) => read::handle_open(ctx, args).await,
        ItemCommand::Pdf(args) => read::handle_pdf(ctx, args).await,
        ItemCommand::Fulltext(args) => read::handle_pdf(ctx, args).await,
        ItemCommand::Children(args) => read::handle_children(ctx, args).await,
        ItemCommand::Download(args) => read::handle_download(ctx, args).await,
        ItemCommand::Deleted(args) => read::handle_deleted(ctx, args).await,
        ItemCommand::Versions(args) => read::handle_versions(ctx, args).await,
        ItemCommand::Outline(args) => read::handle_outline(ctx, &args.key).await,
        ItemCommand::Export(args) => read::handle_export(ctx, args).await,
        ItemCommand::Cite(args) => read::handle_cite(ctx, args).await,
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
