use std::collections::HashSet;

use anyhow::Result;
use zot_local::{
    HybridMode, SearchOptions, WorkspaceName, WorkspaceRagStore, WorkspaceReindexOpts,
    WorkspaceStore,
};
use zot_remote::EmbeddingClient;

use crate::cli::{
    WorkspaceCommand, WorkspaceExportArgs, WorkspaceImportArgs, WorkspaceQueryArgs,
    WorkspaceSearchArgs,
};
use crate::context::AppContext;
use crate::format::{print_items, print_query_chunks, print_workspace, to_pretty_json};
use crate::output::CommandOutput;
use crate::util::{maybe_embed_query, run_pdf};

pub(crate) async fn handle(ctx: &AppContext, command: WorkspaceCommand) -> Result<CommandOutput> {
    let store = WorkspaceStore::new(None);
    match command {
        WorkspaceCommand::New(args) => {
            let name = WorkspaceName::parse(&args.name)?;
            let workspace = store.create(&name, &args.description)?;
            CommandOutput::new(ctx, workspace, None, print_workspace)
        }
        WorkspaceCommand::Delete(args) => {
            let name = WorkspaceName::parse(&args.name)?;
            store.delete(&name)?;
            let payload = serde_json::json!({ "deleted": args.name });
            CommandOutput::new(ctx, payload, None, |_| println!("Workspace deleted."))
        }
        WorkspaceCommand::List => {
            let workspaces = store.list()?;
            CommandOutput::new(ctx, workspaces, None, |workspaces| {
                for workspace in workspaces {
                    print_workspace(workspace);
                    println!();
                }
            })
        }
        WorkspaceCommand::Show(args) => {
            let name = WorkspaceName::parse(&args.name)?;
            let workspace = store.load(&name)?;
            CommandOutput::new(ctx, workspace, None, print_workspace)
        }
        WorkspaceCommand::Add(args) => {
            let name = WorkspaceName::parse(&args.name)?;
            let mut workspace = store.load(&name)?;
            let library = ctx.local_library()?;
            let mut items = Vec::new();
            for key in args.keys {
                if let Some(item) = library.get_item(&key)? {
                    items.push(item);
                }
            }
            let added = store.add_items(&mut workspace, &items);
            store.save(&workspace)?;
            let payload = serde_json::json!({ "added": added });
            CommandOutput::new(ctx, payload, None, move |_| {
                println!("Added {added} item(s).")
            })
        }
        WorkspaceCommand::Remove(args) => {
            let name = WorkspaceName::parse(&args.name)?;
            let mut workspace = store.load(&name)?;
            let removed = store.remove_keys(&mut workspace, &args.keys);
            store.save(&workspace)?;
            let payload = serde_json::json!({ "removed": removed });
            CommandOutput::new(ctx, payload, None, move |_| {
                println!("Removed {removed} item(s).")
            })
        }
        WorkspaceCommand::Import(args) => import_items(ctx, &store, args).await,
        WorkspaceCommand::Search(args) => search_workspace(ctx, &store, args).await,
        WorkspaceCommand::Export(args) => export_workspace(ctx, &store, args).await,
        WorkspaceCommand::Index(args) => index_workspace(ctx, &store, args).await,
        WorkspaceCommand::Query(args) => query_workspace(ctx, &store, args).await,
    }
}

async fn import_items(
    ctx: &AppContext,
    store: &WorkspaceStore,
    args: WorkspaceImportArgs,
) -> Result<CommandOutput> {
    let name = WorkspaceName::parse(&args.name)?;
    let mut workspace = store.load(&name)?;
    let library = ctx.local_library()?;
    let items = if let Some(collection) = args.collection.as_deref() {
        library.get_collection_items(collection)?
    } else if let Some(tag) = args.tag.as_deref() {
        library
            .list_items(None, 10_000, 0)?
            .into_iter()
            .filter(|item| item.tags.iter().any(|existing| existing == tag))
            .collect()
    } else if let Some(query) = args.search.as_deref() {
        library
            .search(SearchOptions {
                query: query.to_string(),
                limit: 10_000,
                ..SearchOptions::default()
            })?
            .items
    } else {
        return Err(zot_core::ZotError::InvalidInput {
            code: "workspace-import".to_string(),
            message: "Provide --collection, --tag, or --search".to_string(),
            hint: None,
        }
        .into());
    };
    let added = store.add_items(&mut workspace, &items);
    store.save(&workspace)?;
    let payload = serde_json::json!({ "added": added });
    CommandOutput::new(ctx, payload, None, move |_| {
        println!("Imported {added} item(s).")
    })
}

async fn search_workspace(
    ctx: &AppContext,
    store: &WorkspaceStore,
    args: WorkspaceSearchArgs,
) -> Result<CommandOutput> {
    let name = WorkspaceName::parse(&args.name)?;
    let workspace = store.load(&name)?;
    let allowed = workspace
        .items
        .iter()
        .map(|item| item.key.clone())
        .collect::<HashSet<_>>();
    let result = ctx.local_library()?.search(SearchOptions {
        query: args.query,
        limit: 10_000,
        ..SearchOptions::default()
    })?;
    let filtered = result
        .items
        .into_iter()
        .filter(|item| allowed.contains(&item.key))
        .collect::<Vec<_>>();
    CommandOutput::new(ctx, filtered, None, |filtered| print_items(filtered))
}

async fn export_workspace(
    ctx: &AppContext,
    store: &WorkspaceStore,
    args: WorkspaceExportArgs,
) -> Result<CommandOutput> {
    let name = WorkspaceName::parse(&args.name)?;
    let workspace = store.load(&name)?;
    let library = ctx.local_library()?;
    let mut items = Vec::with_capacity(workspace.items.len());
    for entry in &workspace.items {
        if let Some(item) = library.get_item(&entry.key)? {
            items.push(item);
        }
    }
    match args.format.as_str() {
        "json" => {
            // JSON mode wraps the items in the envelope; the plain-text pipeline
            // (no `--json`) still emits the bare pretty-printed array unchanged.
            let rendered = to_pretty_json(&items)?;
            CommandOutput::new(ctx, items, None, move |_| println!("{rendered}"))
        }
        "bibtex" => {
            let mut exports = Vec::new();
            for item in items {
                if let Some(export) = library.export_citation(&item.key, "bibtex")? {
                    exports.push(export);
                }
            }
            let content = exports.join("\n\n");
            let payload = serde_json::json!({ "format": "bibtex", "content": content });
            CommandOutput::new(ctx, payload, None, move |_| println!("{content}"))
        }
        _ => {
            let mut content = format!("# Workspace {}", workspace.name);
            if !workspace.description.is_empty() {
                content.push_str(&format!("\n\n{}", workspace.description));
            }
            for item in items {
                content.push_str(&format!("\n\n## {} ({})", item.title, item.key));
                if let Some(abstract_note) = item.abstract_note.as_deref() {
                    content.push_str(&format!("\n{abstract_note}"));
                }
            }
            let payload = serde_json::json!({ "format": "markdown", "content": content });
            CommandOutput::new(ctx, payload, None, move |_| println!("{content}"))
        }
    }
}

async fn index_workspace(
    ctx: &AppContext,
    store: &WorkspaceStore,
    args: crate::cli::WorkspaceIndexArgs,
) -> Result<CommandOutput> {
    let name = WorkspaceName::parse(&args.name)?;
    let workspace = store.load(&name)?;
    let library = ctx.local_library()?;
    let rag = WorkspaceRagStore::open(store, &name)?;
    let backend = ctx.pdf_backend();
    let embedding_client = EmbeddingClient::new(ctx.http(), ctx.config.embedding.clone());
    let opts = WorkspaceReindexOpts {
        fulltext: !args.no_fulltext,
        force_rebuild: args.force_rebuild,
    };
    let (rag, stats, pending) = run_pdf(move || {
        let (stats, pending) = rag.reindex_workspace(&library, &workspace, &backend, opts)?;
        Ok::<_, zot_core::ZotError>((rag, stats, pending))
    })
    .await?;
    if embedding_client.configured() && !pending.is_empty() {
        let texts = pending
            .iter()
            .map(|chunk| chunk.text.clone())
            .collect::<Vec<_>>();
        let embeddings = embedding_client.embed(&texts).await?;
        rag.apply_pending_embeddings(pending, embeddings)?;
    }
    let payload =
        serde_json::json!({ "indexed": true, "items": stats.items, "chunks": stats.chunks });
    CommandOutput::new(ctx, payload, None, |_| println!("Workspace indexed."))
}

async fn query_workspace(
    ctx: &AppContext,
    store: &WorkspaceStore,
    args: WorkspaceQueryArgs,
) -> Result<CommandOutput> {
    let name = WorkspaceName::parse(&args.name)?;
    let workspace = store.load(&name)?;
    let rag = WorkspaceRagStore::open(store, &name)?;
    let mode: HybridMode = args.mode.into();
    let embedding = if matches!(mode, HybridMode::Semantic | HybridMode::Hybrid) {
        maybe_embed_query(ctx.http(), &ctx.config.embedding, &args.question).await?
    } else {
        None
    };
    let chunks = rag.query_workspace(
        &workspace,
        &args.question,
        mode,
        embedding.as_deref(),
        args.limit,
    )?;
    CommandOutput::new(ctx, chunks, None, |chunks| print_query_chunks(chunks))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use zot_core::{AppConfig, LibraryScope};
    use zot_local::PdfiumBackend;
    use zot_remote::HttpRuntime;

    fn ctx(json: bool) -> AppContext {
        AppContext {
            json,
            profile: Some("default".to_string()),
            scope: LibraryScope::User,
            config: AppConfig::default(),
            http: Arc::new(HttpRuntime::default()),
            pdf: Arc::new(PdfiumBackend),
        }
    }

    #[test]
    fn export_bibtex_json_envelope_exposes_format_and_content() {
        // Mirrors the `--json` payload the bibtex export arm emits.
        let payload = serde_json::json!({
            "format": "bibtex",
            "content": "@article{key, title={Sample}}",
        });
        let out = CommandOutput::new(&ctx(true), payload, None, |_| unreachable!())
            .expect("build output");
        let json = out.as_json().expect("json payload");
        assert!(json.contains("\"ok\": true"));
        assert!(json.contains("\"format\": \"bibtex\""));
        assert!(json.contains("\"content\""));
    }

    #[test]
    fn export_markdown_human_mode_has_no_json_payload() {
        // In the default (non-json) mode the markdown export renders bare text,
        // so `as_json()` must be absent.
        let content = "# Workspace Demo".to_string();
        let payload = serde_json::json!({ "format": "markdown", "content": content });
        let out = CommandOutput::new(&ctx(false), payload, None, move |_| println!("{content}"))
            .expect("build output");
        assert_eq!(out.as_json(), None);
    }
}
