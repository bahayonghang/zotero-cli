use std::collections::HashSet;

use anyhow::Result;
use zot_local::{
    HybridMode, PdfBackend, PdfCache, PdfiumBackend, RagIndex, SearchOptions, WorkspaceStore,
    build_metadata_chunk, chunk_text, compute_term_frequencies, tokenize,
};
use zot_remote::EmbeddingClient;

use crate::cli::{
    WorkspaceCommand, WorkspaceExportArgs, WorkspaceImportArgs, WorkspaceQueryArgs,
    WorkspaceSearchArgs,
};
use crate::context::AppContext;
use crate::format::{
    print_enveloped, print_items, print_json, print_query_chunks, print_workspace,
};
use crate::util::maybe_embed_query;

pub(crate) async fn handle(ctx: &AppContext, command: WorkspaceCommand) -> Result<()> {
    let store = WorkspaceStore::new(None);
    match command {
        WorkspaceCommand::New(args) => {
            let workspace = store.create(&args.name, &args.description)?;
            if ctx.json {
                print_enveloped(workspace, None)?;
            } else {
                print_workspace(&workspace);
            }
        }
        WorkspaceCommand::Delete(args) => {
            store.delete(&args.name)?;
            if ctx.json {
                print_enveloped(serde_json::json!({ "deleted": args.name }), None)?;
            } else {
                println!("Workspace deleted.");
            }
        }
        WorkspaceCommand::List => {
            let workspaces = store.list()?;
            if ctx.json {
                print_enveloped(&workspaces, None)?;
            } else {
                for workspace in workspaces {
                    print_workspace(&workspace);
                    println!();
                }
            }
        }
        WorkspaceCommand::Show(args) => {
            let workspace = store.load(&args.name)?;
            if ctx.json {
                print_enveloped(&workspace, None)?;
            } else {
                print_workspace(&workspace);
            }
        }
        WorkspaceCommand::Add(args) => {
            let mut workspace = store.load(&args.name)?;
            let library = ctx.local_library()?;
            let mut items = Vec::new();
            for key in args.keys {
                if let Some(item) = library.get_item(&key)? {
                    items.push(item);
                }
            }
            let added = store.add_items(&mut workspace, &items);
            store.save(&workspace)?;
            if ctx.json {
                print_enveloped(serde_json::json!({ "added": added }), None)?;
            } else {
                println!("Added {added} item(s).");
            }
        }
        WorkspaceCommand::Remove(args) => {
            let mut workspace = store.load(&args.name)?;
            let removed = store.remove_keys(&mut workspace, &args.keys);
            store.save(&workspace)?;
            if ctx.json {
                print_enveloped(serde_json::json!({ "removed": removed }), None)?;
            } else {
                println!("Removed {removed} item(s).");
            }
        }
        WorkspaceCommand::Import(args) => import_items(ctx, &store, args).await?,
        WorkspaceCommand::Search(args) => search_workspace(ctx, &store, args).await?,
        WorkspaceCommand::Export(args) => export_workspace(ctx, &store, args).await?,
        WorkspaceCommand::Index(args) => index_workspace(ctx, &store, &args.name).await?,
        WorkspaceCommand::Query(args) => query_workspace(ctx, &store, args).await?,
    }
    Ok(())
}

async fn import_items(
    ctx: &AppContext,
    store: &WorkspaceStore,
    args: WorkspaceImportArgs,
) -> Result<()> {
    let mut workspace = store.load(&args.name)?;
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
    if ctx.json {
        print_enveloped(serde_json::json!({ "added": added }), None)?;
    } else {
        println!("Imported {added} item(s).");
    }
    Ok(())
}

async fn search_workspace(
    ctx: &AppContext,
    store: &WorkspaceStore,
    args: WorkspaceSearchArgs,
) -> Result<()> {
    let workspace = store.load(&args.name)?;
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
    if ctx.json {
        print_enveloped(&filtered, None)?;
    } else {
        print_items(&filtered);
    }
    Ok(())
}

async fn export_workspace(
    ctx: &AppContext,
    store: &WorkspaceStore,
    args: WorkspaceExportArgs,
) -> Result<()> {
    let workspace = store.load(&args.name)?;
    let library = ctx.local_library()?;
    let mut items = Vec::with_capacity(workspace.items.len());
    for entry in &workspace.items {
        if let Some(item) = library.get_item(&entry.key)? {
            items.push(item);
        }
    }
    match args.format.as_str() {
        "json" => {
            if ctx.json {
                print_enveloped(&items, None)?;
            } else {
                print_json(&items)?;
            }
        }
        "bibtex" => {
            let mut exports = Vec::new();
            for item in items {
                if let Some(export) = library.export_citation(&item.key, "bibtex")? {
                    exports.push(export);
                }
            }
            println!("{}", exports.join("\n\n"));
        }
        _ => {
            println!("# Workspace {}", workspace.name);
            if !workspace.description.is_empty() {
                println!("\n{}", workspace.description);
            }
            for item in items {
                println!("\n## {} ({})", item.title, item.key);
                if let Some(abstract_note) = item.abstract_note.as_deref() {
                    println!("{abstract_note}");
                }
            }
        }
    }
    Ok(())
}

async fn index_workspace(ctx: &AppContext, store: &WorkspaceStore, name: &str) -> Result<()> {
    let workspace = store.load(name)?;
    let library = ctx.local_library()?;
    let index = RagIndex::open(store.root().join(format!("{name}.idx.sqlite")))?;
    index.clear()?;
    let backend = PdfiumBackend;
    let cache = PdfCache::new(Some(store.root().join(".md_cache.sqlite")))?;
    let embedding_client = EmbeddingClient::new(ctx.http(), ctx.config.embedding.clone());
    let mut all_texts = Vec::new();
    let mut chunk_ids = Vec::new();
    for entry in workspace.items {
        if let Some(item) = library.get_item(&entry.key)? {
            let metadata_chunk = build_metadata_chunk(&item);
            let chunk_id = index.insert_chunk(&item.key, "metadata", &metadata_chunk)?;
            index.insert_terms(
                chunk_id,
                &compute_term_frequencies(&tokenize(&metadata_chunk)),
            )?;
            all_texts.push(metadata_chunk);
            chunk_ids.push(chunk_id);
            if let Some(attachment) = library.get_pdf_attachment(&item.key)? {
                let pdf_path = library.pdf_path(&attachment);
                let text = if let Some(cached) = cache.get(&pdf_path)? {
                    cached
                } else {
                    let extracted = backend.extract_text(&pdf_path, None)?;
                    cache.put(&pdf_path, &extracted)?;
                    extracted
                };
                for chunk in chunk_text(&text, &item.title, 500, 50) {
                    let chunk_id = index.insert_chunk(&item.key, "pdf", &chunk)?;
                    index.insert_terms(chunk_id, &compute_term_frequencies(&tokenize(&chunk)))?;
                    all_texts.push(chunk);
                    chunk_ids.push(chunk_id);
                }
            }
        }
    }
    if embedding_client.configured() && !all_texts.is_empty() {
        let embeddings = embedding_client.embed(&all_texts).await?;
        for (chunk_id, embedding) in chunk_ids.into_iter().zip(embeddings) {
            index.set_embedding(chunk_id, &embedding)?;
        }
    }
    if ctx.json {
        print_enveloped(serde_json::json!({ "indexed": true }), None)?;
    } else {
        println!("Workspace indexed.");
    }
    Ok(())
}

async fn query_workspace(
    ctx: &AppContext,
    store: &WorkspaceStore,
    args: WorkspaceQueryArgs,
) -> Result<()> {
    let index = RagIndex::open(store.root().join(format!("{}.idx.sqlite", args.name)))?;
    let mode: HybridMode = args.mode.into();
    let embedding = if matches!(mode, HybridMode::Semantic | HybridMode::Hybrid) {
        maybe_embed_query(ctx.http(), &ctx.config.embedding, &args.question).await?
    } else {
        None
    };
    let chunks = index.query(&args.question, mode, embedding.as_deref(), args.limit)?;
    if ctx.json {
        print_enveloped(&chunks, None)?;
    } else {
        print_query_chunks(&chunks);
    }
    Ok(())
}
