use std::collections::{BTreeMap, HashSet};

use anyhow::Result;
use chrono::Utc;
use zot_core::{EnvelopeMeta, Item, SavedSearchCondition, SemanticHit, SemanticIndexStatus};
use zot_local::{
    HybridMode, LocalLibrary, PdfBackend, PdfCache, PdfiumBackend, RagIndex, SearchOptions,
    build_metadata_chunk, chunk_text, compute_term_frequencies, tokenize,
};
use zot_remote::{BetterBibTexClient, EmbeddingClient};

use crate::cli::{
    LibraryCommand, LibrarySavedSearchCommand, LibrarySavedSearchCreateArgs,
    LibrarySavedSearchDeleteArgs, LibrarySemanticIndexArgs, LibrarySemanticSearchArgs,
};
use crate::commands::item::merge::merge_item_set;
use crate::context::AppContext;
use crate::format::{print_enveloped, print_items, print_stats};
use crate::util::{maybe_embed_query, parse_json_input};

pub(crate) async fn handle(ctx: &AppContext, command: LibraryCommand) -> Result<()> {
    let library = ctx.local_library()?;
    match command {
        LibraryCommand::Search(args) => {
            let result = library.search(SearchOptions {
                query: args.query,
                collection: args.collection,
                item_type: args.item_type,
                tag: args.tag,
                creator: args.creator,
                year: args.year,
                sort: args.sort.map(Into::into),
                direction: args.direction.into(),
                limit: args.limit,
                offset: args.offset,
            })?;
            if ctx.json {
                print_enveloped(
                    &result.items,
                    Some(EnvelopeMeta {
                        count: Some(result.items.len()),
                        total: Some(result.total),
                        profile: ctx.profile.clone(),
                    }),
                )?;
            } else {
                print_items(&result.items);
            }
        }
        LibraryCommand::List(args) => {
            let items = library.list_items(args.collection.as_deref(), args.limit, args.offset)?;
            if ctx.json {
                print_enveloped(
                    &items,
                    Some(EnvelopeMeta {
                        count: Some(items.len()),
                        total: None,
                        profile: ctx.profile.clone(),
                    }),
                )?;
            } else {
                print_items(&items);
            }
        }
        LibraryCommand::Recent(args) => {
            let items = if let Some(count) = args.count {
                library.get_recent_items_by_count(count)?
            } else if let Some(since) = args.since.as_deref() {
                library.get_recent_items(since, args.sort.into(), args.limit)?
            } else {
                library.get_recent_items_by_count(10)?
            };
            if ctx.json {
                print_enveloped(&items, None)?;
            } else {
                print_items(&items);
            }
        }
        LibraryCommand::Stats => {
            let stats = library.get_stats()?;
            if ctx.json {
                print_enveloped(stats, None)?;
            } else {
                print_stats(&stats);
            }
        }
        LibraryCommand::Citekey(args) => {
            let item = if let Some(result) = library.search_by_citation_key(&args.citekey)? {
                Some(result)
            } else {
                let bbt = BetterBibTexClient::new();
                if bbt.probe().await {
                    bbt.search(&args.citekey)
                        .await?
                        .into_iter()
                        .find(|candidate| candidate.citekey == args.citekey)
                        .and_then(|candidate| library.get_item(&candidate.item_key).ok().flatten())
                        .map(|item| zot_core::CitationKeyMatch {
                            citekey: args.citekey.clone(),
                            source: "better-bibtex".to_string(),
                            item,
                        })
                } else {
                    None
                }
            }
            .ok_or_else(|| zot_core::ZotError::InvalidInput {
                code: "citation-key-not-found".to_string(),
                message: format!("Citation key '{}' not found", args.citekey),
                hint: None,
            })?;
            if ctx.json {
                print_enveloped(&item, None)?;
            } else {
                print_items(std::slice::from_ref(&item.item));
            }
        }
        LibraryCommand::Tags => {
            let tags = library.get_tags()?;
            if ctx.json {
                print_enveloped(&tags, None)?;
            } else {
                for tag in tags {
                    println!("{} ({})", tag.name, tag.count);
                }
            }
        }
        LibraryCommand::Libraries => {
            let libraries = library.get_libraries()?;
            if ctx.json {
                print_enveloped(&libraries, None)?;
            } else {
                for entry in libraries {
                    println!(
                        "{} [{}] items={}{}{}",
                        entry.library_id,
                        entry.library_type,
                        entry.item_count,
                        entry
                            .group_name
                            .as_deref()
                            .map(|name| format!(" name={name}"))
                            .unwrap_or_default(),
                        entry
                            .feed_name
                            .as_deref()
                            .map(|name| format!(" feed={name}"))
                            .unwrap_or_default()
                    );
                }
            }
        }
        LibraryCommand::Feeds => {
            let feeds = library.get_feeds()?;
            if ctx.json {
                print_enveloped(&feeds, None)?;
            } else if feeds.is_empty() {
                println!("No RSS feeds found.");
            } else {
                for feed in feeds {
                    println!("{} [{}] {}", feed.library_id, feed.item_count, feed.name);
                    println!("  URL: {}", feed.url);
                }
            }
        }
        LibraryCommand::FeedItems(args) => {
            let items = library.get_feed_items(args.library_id, args.limit)?;
            if ctx.json {
                print_enveloped(&items, None)?;
            } else {
                print_items(&items);
            }
        }
        LibraryCommand::SemanticSearch(args) => {
            let hits = semantic_search(ctx, &library, args).await?;
            if ctx.json {
                print_enveloped(&hits, None)?;
            } else {
                for hit in hits {
                    println!("{} [{:.3}] {}", hit.item.key, hit.score, hit.item.title);
                    if let Some(chunk) = hit.matched_chunk {
                        println!("  {}", chunk);
                    }
                }
            }
        }
        LibraryCommand::SemanticIndex(args) => {
            let payload = semantic_index(ctx, &library, args).await?;
            if ctx.json {
                print_enveloped(payload, None)?;
            } else {
                println!("Library semantic index updated.");
            }
        }
        LibraryCommand::SemanticStatus => {
            let status = semantic_status(ctx).await?;
            if ctx.json {
                print_enveloped(status, None)?;
            } else {
                println!(
                    "{} chunks={} items={} embeddings={}",
                    status.path,
                    status.indexed_chunks,
                    status.indexed_items,
                    status.chunks_with_embeddings
                );
            }
        }
        LibraryCommand::Duplicates(args) => {
            let groups = library.find_duplicates(
                args.method.into(),
                args.collection.as_deref(),
                args.limit,
            )?;
            if ctx.json {
                print_enveloped(&groups, None)?;
            } else {
                for group in groups {
                    println!("{} ({:.2})", group.match_type, group.score);
                    print_items(&group.items);
                    println!();
                }
            }
        }
        LibraryCommand::DuplicatesMerge(args) => {
            let payload =
                merge_duplicates(ctx, &args.keeper, &args.duplicates, args.confirm).await?;
            if ctx.json {
                print_enveloped(payload, None)?;
            } else {
                println!("{}", serde_json::to_string_pretty(&payload)?);
            }
        }
        LibraryCommand::SavedSearch { command } => handle_saved_search(ctx, command).await?,
    }
    Ok(())
}

async fn handle_saved_search(ctx: &AppContext, command: LibrarySavedSearchCommand) -> Result<()> {
    match command {
        LibrarySavedSearchCommand::List => {
            let searches = ctx.remote()?.list_saved_searches().await?;
            if ctx.json {
                print_enveloped(
                    &searches,
                    Some(EnvelopeMeta {
                        count: Some(searches.len()),
                        total: Some(searches.len()),
                        profile: ctx.profile.clone(),
                    }),
                )?;
            } else if searches.is_empty() {
                println!("No saved searches found.");
            } else {
                for search in searches {
                    println!("{} {}", search.key, search.name);
                }
            }
        }
        LibrarySavedSearchCommand::Create(args) => {
            let payload = create_saved_search(ctx, args).await?;
            if ctx.json {
                print_enveloped(payload, None)?;
            } else {
                println!("Saved search created.");
            }
        }
        LibrarySavedSearchCommand::Delete(args) => {
            let payload = delete_saved_searches(ctx, args).await?;
            if ctx.json {
                print_enveloped(payload, None)?;
            } else {
                println!("Saved search deleted.");
            }
        }
    }
    Ok(())
}

async fn create_saved_search(
    ctx: &AppContext,
    args: LibrarySavedSearchCreateArgs,
) -> Result<serde_json::Value> {
    let value = parse_json_input(&args.conditions, "saved search conditions")?;
    let conditions: Vec<SavedSearchCondition> =
        serde_json::from_value(value).map_err(|err| zot_core::ZotError::InvalidInput {
            code: "saved-search-conditions".to_string(),
            message: format!("Invalid saved search conditions: {err}"),
            hint: Some("Pass a JSON array of {condition, operator, value?} objects".to_string()),
        })?;
    if conditions.is_empty() {
        return Err(zot_core::ZotError::InvalidInput {
            code: "saved-search-conditions".to_string(),
            message: "Saved search conditions cannot be empty".to_string(),
            hint: Some("Add at least one condition".to_string()),
        }
        .into());
    }
    let key = ctx
        .remote()?
        .create_saved_search(&args.name, &conditions)
        .await?;
    Ok(serde_json::json!({
        "search_key": key,
        "name": args.name,
        "conditions": conditions,
    }))
}

async fn delete_saved_searches(
    ctx: &AppContext,
    args: LibrarySavedSearchDeleteArgs,
) -> Result<serde_json::Value> {
    if args.keys.is_empty() {
        return Err(zot_core::ZotError::InvalidInput {
            code: "saved-search-delete".to_string(),
            message: "At least one saved search key is required".to_string(),
            hint: None,
        }
        .into());
    }
    ctx.remote()?.delete_saved_searches(&args.keys).await?;
    Ok(serde_json::json!({ "deleted": args.keys }))
}

pub(crate) async fn semantic_status(ctx: &AppContext) -> Result<SemanticIndexStatus> {
    let path = ctx.library_index_path();
    if !path.exists() {
        return Ok(SemanticIndexStatus {
            exists: false,
            path: path.display().to_string(),
            indexed_items: 0,
            indexed_chunks: 0,
            chunks_with_embeddings: 0,
            last_indexed_at: None,
        });
    }
    let index = RagIndex::open(&path)?;
    Ok(SemanticIndexStatus {
        exists: true,
        path: path.display().to_string(),
        indexed_items: index.indexed_keys()?.len(),
        indexed_chunks: index.chunk_count()?,
        chunks_with_embeddings: index.embedding_count()?,
        last_indexed_at: index.get_meta("indexed_at")?,
    })
}

async fn semantic_index(
    ctx: &AppContext,
    library: &LocalLibrary,
    args: LibrarySemanticIndexArgs,
) -> Result<serde_json::Value> {
    let path = ctx.library_index_path();
    let index = RagIndex::open(&path)?;
    if args.force_rebuild {
        index.clear()?;
    }
    let backend = PdfiumBackend;
    let cache = PdfCache::new(Some(
        zot_core::AppConfig::config_dir()
            .join("cache")
            .join("library_md_cache.sqlite"),
    ))?;
    let embedding_client = EmbeddingClient::new(ctx.config.embedding.clone());
    let limit = effective_semantic_index_limit(args.limit);
    let items = load_semantic_index_items(library, args.collection.as_deref(), limit)?;
    if !args.force_rebuild {
        for item in &items {
            index.remove_item_chunks(&item.key)?;
        }
        let stale_keys = collect_deleted_indexed_keys(library, &index)?;
        for item_key in stale_keys {
            index.remove_item_chunks(&item_key)?;
        }
    }

    let mut all_texts = Vec::new();
    let mut chunk_ids = Vec::new();
    for item in &items {
        let metadata_chunk = build_metadata_chunk(item);
        let chunk_id = index.insert_chunk(&item.key, "metadata", &metadata_chunk)?;
        index.insert_terms(
            chunk_id,
            &compute_term_frequencies(&tokenize(&metadata_chunk)),
        )?;
        all_texts.push(metadata_chunk);
        chunk_ids.push(chunk_id);

        if args.fulltext
            && let Some(attachment) = library.get_pdf_attachment(&item.key)?
        {
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

    if embedding_client.configured() && !all_texts.is_empty() {
        let embeddings = embedding_client.embed(&all_texts).await?;
        for (chunk_id, embedding) in chunk_ids.into_iter().zip(embeddings) {
            index.set_embedding(chunk_id, &embedding)?;
        }
    }

    index.set_meta("indexed_at", &Utc::now().to_rfc3339())?;
    let status = semantic_status(ctx).await?;
    Ok(serde_json::json!({
        "indexed": true,
        "items": items.len(),
        "fulltext": args.fulltext,
        "status": status,
    }))
}

fn effective_semantic_index_limit(limit: usize) -> usize {
    if limit == 0 { 10_000 } else { limit }
}

fn load_semantic_index_items(
    library: &LocalLibrary,
    collection: Option<&str>,
    limit: usize,
) -> zot_core::ZotResult<Vec<Item>> {
    let items = if let Some(collection) = collection {
        library.get_collection_items(collection)?
    } else {
        library.list_items(None, limit, 0)?
    };
    Ok(truncate_semantic_index_items(items, limit))
}

fn truncate_semantic_index_items<T>(mut items: Vec<T>, limit: usize) -> Vec<T> {
    items.truncate(limit);
    items
}

fn collect_deleted_indexed_keys(
    library: &LocalLibrary,
    index: &RagIndex,
) -> zot_core::ZotResult<Vec<String>> {
    select_stale_item_keys(index.indexed_keys()?, |item_key| {
        Ok(library.get_item(item_key)?.is_some())
    })
}

fn select_stale_item_keys<F>(
    indexed_keys: Vec<String>,
    mut item_exists: F,
) -> zot_core::ZotResult<Vec<String>>
where
    F: FnMut(&str) -> zot_core::ZotResult<bool>,
{
    let mut stale_keys = Vec::new();
    for item_key in indexed_keys {
        if !item_exists(&item_key)? {
            stale_keys.push(item_key);
        }
    }
    Ok(stale_keys)
}

async fn semantic_search(
    ctx: &AppContext,
    library: &LocalLibrary,
    args: LibrarySemanticSearchArgs,
) -> Result<Vec<SemanticHit>> {
    let index = RagIndex::open(ctx.library_index_path())?;
    let mut mode: HybridMode = args.mode.into();
    let embedding = if matches!(mode, HybridMode::Semantic | HybridMode::Hybrid) {
        match maybe_embed_query(&ctx.config.embedding, &args.query).await? {
            Some(embedding) => Some(embedding),
            None => {
                mode = HybridMode::Bm25;
                None
            }
        }
    } else {
        None
    };

    let allowed = if let Some(collection) = args.collection.as_deref() {
        library
            .get_collection_items(collection)?
            .into_iter()
            .map(|item| item.key)
            .collect::<HashSet<_>>()
    } else {
        HashSet::new()
    };

    let chunks = index.query(
        &args.query,
        mode,
        embedding.as_deref(),
        args.limit.saturating_mul(5),
    )?;
    let mut deduped = BTreeMap::<String, SemanticHit>::new();
    for chunk in chunks {
        if !allowed.is_empty() && !allowed.contains(&chunk.item_key) {
            continue;
        }
        if let Some(item) = library.get_item(&chunk.item_key)? {
            let entry = deduped
                .entry(item.key.clone())
                .or_insert_with(|| SemanticHit {
                    item: item.clone(),
                    score: chunk.score,
                    source: chunk.source.clone(),
                    matched_chunk: Some(chunk.content.clone()),
                });
            if chunk.score > entry.score {
                entry.score = chunk.score;
                entry.source = chunk.source.clone();
                entry.matched_chunk = Some(chunk.content.clone());
            }
        }
        if deduped.len() >= args.limit {
            break;
        }
    }
    Ok(deduped.into_values().collect())
}

async fn merge_duplicates(
    ctx: &AppContext,
    keeper_key: &str,
    duplicate_keys: &[String],
    confirm: bool,
) -> Result<zot_core::MergeOperation> {
    let duplicates = duplicate_keys
        .iter()
        .filter(|key| key.as_str() != keeper_key)
        .cloned()
        .collect::<Vec<_>>();
    if duplicates.is_empty() {
        return Err(zot_core::ZotError::InvalidInput {
            code: "duplicate-keys".to_string(),
            message: "No duplicate keys to merge".to_string(),
            hint: None,
        }
        .into());
    }
    merge_item_set(&ctx.remote()?, keeper_key, &duplicates, confirm).await
}

#[cfg(test)]
mod tests {
    use super::{
        create_saved_search, effective_semantic_index_limit, select_stale_item_keys,
        truncate_semantic_index_items,
    };
    use crate::cli::LibrarySavedSearchCreateArgs;
    use crate::context::AppContext;
    use zot_core::{AppConfig, LibraryScope};

    #[test]
    fn semantic_index_limit_defaults_to_ten_thousand() {
        assert_eq!(effective_semantic_index_limit(0), 10_000);
        assert_eq!(effective_semantic_index_limit(25), 25);
    }

    #[test]
    fn select_stale_item_keys_returns_missing_entries_only() {
        let stale = select_stale_item_keys(
            vec!["ATTN001".to_string(), "DEAD999".to_string()],
            |item_key| Ok(item_key == "ATTN001"),
        )
        .expect("select stale keys");

        assert_eq!(stale, vec!["DEAD999"]);
    }

    #[test]
    fn truncate_semantic_index_items_applies_collection_limit() {
        let limited = truncate_semantic_index_items(vec![1, 2, 3], 2);
        assert_eq!(limited, vec![1, 2]);
    }

    #[tokio::test]
    async fn rejects_empty_saved_search_conditions_before_remote_calls() {
        let ctx = AppContext {
            json: true,
            profile: None,
            scope: LibraryScope::User,
            config: AppConfig::default(),
        };
        let err = create_saved_search(
            &ctx,
            LibrarySavedSearchCreateArgs {
                name: "Recent".to_string(),
                conditions: "[]".to_string(),
            },
        )
        .await
        .expect_err("empty conditions should fail");
        let err = err.downcast_ref::<zot_core::ZotError>().expect("zot error");
        match err {
            zot_core::ZotError::InvalidInput { code, .. } => {
                assert_eq!(code, "saved-search-conditions")
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
