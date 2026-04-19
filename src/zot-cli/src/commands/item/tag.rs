use anyhow::Result;
use zot_local::SearchOptions;

use crate::cli::ItemTagCommand;
use crate::context::AppContext;
use crate::format::print_enveloped;

pub(crate) async fn handle(ctx: &AppContext, command: ItemTagCommand) -> Result<()> {
    match command {
        ItemTagCommand::List(args) => {
            let item = ctx.local_library()?.get_item(&args.key)?.ok_or_else(|| {
                zot_core::ZotError::InvalidInput {
                    code: "item-not-found".to_string(),
                    message: format!("Item '{}' not found", args.key),
                    hint: None,
                }
            })?;
            if ctx.json {
                print_enveloped(&item.tags, None)?;
            } else {
                for tag in item.tags {
                    println!("{tag}");
                }
            }
        }
        ItemTagCommand::Add(args) => {
            ctx.remote()?.add_tags(&args.key, &args.tags).await?;
            if ctx.json {
                print_enveloped(
                    serde_json::json!({ "key": args.key, "added": args.tags }),
                    None,
                )?;
            } else {
                println!("Tags added.");
            }
        }
        ItemTagCommand::Remove(args) => {
            ctx.remote()?.remove_tags(&args.key, &args.tags).await?;
            if ctx.json {
                print_enveloped(
                    serde_json::json!({ "key": args.key, "removed": args.tags }),
                    None,
                )?;
            } else {
                println!("Tags removed.");
            }
        }
        ItemTagCommand::Batch(args) => {
            let payload = batch_update_tags(ctx, args).await?;
            if ctx.json {
                print_enveloped(payload, None)?;
            } else {
                println!("{}", serde_json::to_string_pretty(&payload)?);
            }
        }
    }
    Ok(())
}

async fn batch_update_tags(
    ctx: &AppContext,
    args: crate::cli::ItemTagBatchArgs,
) -> Result<serde_json::Value> {
    if args.query.trim().is_empty() && args.tag.is_none() {
        return Err(zot_core::ZotError::InvalidInput {
            code: "batch-tags-filter".to_string(),
            message: "Provide --query and/or --tag".to_string(),
            hint: None,
        }
        .into());
    }
    if args.add_tags.is_empty() && args.remove_tags.is_empty() {
        return Err(zot_core::ZotError::InvalidInput {
            code: "batch-tags-op".to_string(),
            message: "Provide --add-tag and/or --remove-tag".to_string(),
            hint: None,
        }
        .into());
    }
    let library = ctx.local_library()?;
    let result = library.search(SearchOptions {
        query: args.query,
        tag: args.tag,
        limit: args.limit,
        ..SearchOptions::default()
    })?;
    let remote = ctx.remote()?;
    for item in &result.items {
        if !args.add_tags.is_empty() {
            remote.add_tags(&item.key, &args.add_tags).await?;
        }
        if !args.remove_tags.is_empty() {
            remote.remove_tags(&item.key, &args.remove_tags).await?;
        }
    }
    Ok(serde_json::json!({
        "matched": result.items.len(),
        "keys": result.items.iter().map(|item| item.key.clone()).collect::<Vec<_>>(),
        "added": args.add_tags,
        "removed": args.remove_tags,
    }))
}
