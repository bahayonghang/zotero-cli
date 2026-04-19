use std::collections::{BTreeMap, HashSet};

use anyhow::Result;
use chrono::Utc;
use zot_core::{EnvelopeMeta, Item, SemanticHit, SemanticIndexStatus};
use zot_local::{
    HybridMode, LocalLibrary, PdfBackend, PdfCache, PdfiumBackend, RagIndex, SearchOptions,
    build_metadata_chunk, chunk_text, compute_term_frequencies, tokenize,
};
use zot_remote::{BetterBibTexClient, EmbeddingClient};

use crate::cli::{LibraryCommand, LibrarySemanticIndexArgs, LibrarySemanticSearchArgs};
use crate::context::AppContext;
use crate::format::{print_enveloped, print_items, print_stats};
use crate::util::maybe_embed_query;

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
            let items = library.get_recent_items(&args.since, args.sort.into(), args.limit)?;
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
    }
    Ok(())
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
) -> Result<serde_json::Value> {
    let remote = ctx.remote()?;
    let mut duplicates = duplicate_keys
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

    let mut keeper = remote.get_item_json(keeper_key).await?;
    let keeper_children = remote.list_children(keeper_key).await?;
    let mut tags = keeper
        .get("tags")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|tag| {
            tag.get("tag")
                .and_then(|value| value.as_str())
                .map(ToOwned::to_owned)
        })
        .collect::<HashSet<_>>();
    let mut collections = keeper
        .get("collections")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|value| value.as_str().map(ToOwned::to_owned))
        .collect::<HashSet<_>>();
    let keeper_signatures = keeper_children
        .iter()
        .filter_map(attachment_signature)
        .collect::<HashSet<_>>();
    let mut child_items = Vec::new();
    let mut skipped_attachments = 0usize;
    for key in &duplicates {
        let item = remote.get_item_json(key).await?;
        let children = remote.list_children(key).await?;
        for tag in item
            .get("tags")
            .and_then(|value| value.as_array())
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|tag| {
                tag.get("tag")
                    .and_then(|value| value.as_str())
                    .map(ToOwned::to_owned)
            })
        {
            tags.insert(tag);
        }
        for collection in item
            .get("collections")
            .and_then(|value| value.as_array())
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|value| value.as_str().map(ToOwned::to_owned))
        {
            collections.insert(collection);
        }
        for child in children {
            if let Some(signature) = attachment_signature(&child) {
                if keeper_signatures.contains(&signature) {
                    skipped_attachments += 1;
                    continue;
                }
            }
            child_items.push(child);
        }
    }

    if !confirm {
        duplicates.sort();
        return Ok(serde_json::json!({
            "keeper": keeper_key,
            "duplicates": duplicates,
            "tags": tags,
            "collections": collections,
            "child_items_to_reparent": child_items.len(),
            "skipped_duplicate_attachments": skipped_attachments,
            "confirm_required": true,
        }));
    }

    keeper["tags"] = serde_json::Value::Array(
        tags.into_iter()
            .map(|tag| serde_json::json!({ "tag": tag }))
            .collect(),
    );
    remote.update_item_value(&keeper).await?;
    for collection in collections {
        remote
            .add_item_to_collection(keeper_key, &collection)
            .await?;
    }
    for mut child in child_items {
        child["parentItem"] = serde_json::Value::String(keeper_key.to_string());
        remote.update_item_value(&child).await?;
    }
    for key in &duplicates {
        remote.set_deleted(key, true).await?;
    }
    Ok(serde_json::json!({
        "keeper": keeper_key,
        "duplicates_trashed": duplicates,
        "skipped_duplicate_attachments": skipped_attachments,
    }))
}

fn attachment_signature(value: &serde_json::Value) -> Option<(String, String, String, String)> {
    (value
        .get("itemType")
        .and_then(|item_type| item_type.as_str())
        == Some("attachment"))
    .then(|| {
        (
            value
                .get("contentType")
                .and_then(|entry| entry.as_str())
                .unwrap_or_default()
                .to_string(),
            value
                .get("filename")
                .and_then(|entry| entry.as_str())
                .unwrap_or_default()
                .to_string(),
            value
                .get("md5")
                .and_then(|entry| entry.as_str())
                .unwrap_or_default()
                .to_string(),
            value
                .get("url")
                .and_then(|entry| entry.as_str())
                .unwrap_or_default()
                .to_string(),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::{
        effective_semantic_index_limit, select_stale_item_keys, truncate_semantic_index_items,
    };

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
}
