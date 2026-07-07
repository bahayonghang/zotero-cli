use anyhow::Result;

use crate::cli::CollectionCommand;
use crate::context::AppContext;
use crate::format::{print_collections, print_items};
use crate::output::CommandOutput;

pub(crate) async fn handle(ctx: &AppContext, command: CollectionCommand) -> Result<CommandOutput> {
    match command {
        CollectionCommand::List => {
            let collections = ctx.local_library()?.get_collections()?;
            CommandOutput::new(ctx, collections, None, |collections| {
                print_collections(collections, 0)
            })
        }
        CollectionCommand::Get(args) => {
            let collection = ctx
                .local_library()?
                .get_collection(&args.key)?
                .ok_or_else(|| zot_core::ZotError::InvalidInput {
                    code: "collection-not-found".to_string(),
                    message: format!("Collection '{}' not found", args.key),
                    hint: Some("Use 'zot collection list' to inspect collection keys".to_string()),
                })?;
            CommandOutput::new(ctx, collection, None, |collection| {
                print_collections(std::slice::from_ref(collection), 0)
            })
        }
        CollectionCommand::Subcollections(args) => {
            let collections = ctx.local_library()?.get_subcollections(&args.key)?;
            CommandOutput::new(ctx, collections, None, |collections| {
                if collections.is_empty() {
                    println!("No subcollections found.");
                } else {
                    print_collections(collections, 0);
                }
            })
        }
        CollectionCommand::Items(args) => {
            let items = ctx.local_library()?.get_collection_items(&args.key)?;
            CommandOutput::new(ctx, items, None, |items| print_items(items))
        }
        CollectionCommand::Search(args) => {
            let collections = ctx
                .local_library()?
                .search_collections(&args.query, args.limit)?;
            CommandOutput::new(ctx, collections, None, |collections| {
                print_collections(collections, 0)
            })
        }
        CollectionCommand::ItemCount(args) => {
            let count = ctx.local_library()?.get_collection_item_count(&args.key)?;
            let payload = serde_json::json!({ "collection_key": args.key, "item_count": count });
            CommandOutput::new(ctx, payload, None, move |_| println!("{count}"))
        }
        CollectionCommand::Tags(args) => {
            let tags = ctx.local_library()?.get_collection_tags(&args.key)?;
            CommandOutput::new(ctx, tags, None, |tags| {
                if tags.is_empty() {
                    println!("No tags found.");
                } else {
                    for tag in tags {
                        println!("{} ({})", tag.name, tag.count);
                    }
                }
            })
        }
        CollectionCommand::Create(args) => {
            let key = ctx
                .remote()?
                .create_collection(&args.name, args.parent.as_deref())
                .await?;
            let payload = serde_json::json!({ "collection_key": key });
            CommandOutput::new(ctx, payload, None, move |_| {
                println!("Collection created: {key}")
            })
        }
        CollectionCommand::Rename(args) => {
            ctx.remote()?
                .rename_collection(&args.key, &args.new_name)
                .await?;
            let payload = serde_json::json!({ "renamed": args.key, "name": args.new_name });
            CommandOutput::new(ctx, payload, None, |_| println!("Collection renamed."))
        }
        CollectionCommand::Delete(args) => {
            ctx.remote()?.delete_collection(&args.key).await?;
            let payload = serde_json::json!({ "deleted": args.key });
            CommandOutput::new(ctx, payload, None, |_| println!("Collection deleted."))
        }
        CollectionCommand::AddItem(args) => {
            ctx.remote()?
                .add_item_to_collection(&args.item_key, &args.collection_key)
                .await?;
            let payload = serde_json::json!({
                "item_key": args.item_key,
                "collection_key": args.collection_key,
            });
            CommandOutput::new(ctx, payload, None, |_| println!("Item added to collection."))
        }
        CollectionCommand::RemoveItem(args) => {
            ctx.remote()?
                .remove_item_from_collection(&args.item_key, &args.collection_key)
                .await?;
            let payload = serde_json::json!({
                "item_key": args.item_key,
                "collection_key": args.collection_key,
            });
            CommandOutput::new(ctx, payload, None, |_| {
                println!("Item removed from collection.")
            })
        }
    }
}
