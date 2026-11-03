pub(crate) mod annotation;
pub(crate) mod import;
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

pub(crate) async fn handle(ctx: &AppContext, command: ItemCommand) -> Result<CommandOutput> {
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
        // Ingestion futures are large; embedding them makes every item command
        // exhaust the Windows debug stack when it reaches nested HTTP polling.
        ItemCommand::Create(args) => Box::pin(write::handle_create(ctx, args)).await,
        ItemCommand::AddDoi(args) => Box::pin(write::handle_add_doi(ctx, args)).await,
        ItemCommand::AddUrl(args) => Box::pin(write::handle_add_url(ctx, args)).await,
        ItemCommand::AddFile(args) => Box::pin(write::handle_add_file(ctx, args)).await,
        ItemCommand::Merge(args) => write::handle_merge(ctx, args).await,
        ItemCommand::Import(args) => import::handle_import(ctx, args).await,
        ItemCommand::Update(args) => write::handle_update(ctx, args).await,
        ItemCommand::Trash(args) => write::handle_trash(ctx, args).await,
        ItemCommand::Restore(args) => write::handle_restore(ctx, args).await,
        ItemCommand::Attach(args) => Box::pin(write::handle_attach(ctx, args)).await,
        ItemCommand::Note { command } => note::handle(ctx, command).await,
        ItemCommand::Tag { command } => tag::handle(ctx, command).await,
        ItemCommand::Annotation { command } => annotation::handle(ctx, command).await,
        ItemCommand::Scite { command } => scite::handle(ctx, command).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::{Cli, Commands};
    use clap::Parser;

    #[test]
    fn dispatcher_future_does_not_embed_large_ingestion_workflows() {
        let ctx = AppContext {
            json: true,
            profile: None,
            scope: zot_core::LibraryScope::User,
            config: zot_core::AppConfig::default(),
            http: std::sync::Arc::new(zot_remote::HttpRuntime::new().expect("http")),
            pdf: std::sync::Arc::new(zot_local::PdfiumBackend),
        };
        let cli = Cli::try_parse_from([
            "zot",
            "item",
            "tag",
            "add",
            "ITEMABCD",
            "--tag",
            "test",
            "--write-backend",
            "local",
        ])
        .expect("parse");
        let Commands::Item { command } = cli.command else {
            panic!("item command");
        };
        let future = handle(&ctx, command);
        // A 68 KiB inline ingestion state caused even local tag previews to
        // overflow the Windows main-thread stack in debug builds. Leave room
        // for construction temporaries and nested HTTP polling, not just self.
        assert!(
            std::mem::size_of_val(&future) <= 16 * 1024,
            "item dispatcher embeds {} bytes",
            std::mem::size_of_val(&future)
        );
    }
}
