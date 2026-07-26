use anyhow::Result;
use zot_local::{CollectionContent, CollectionNav};

use crate::cli::{CollectionCommand, resolved_output_limit};
use crate::context::AppContext;
use crate::format::{print_collections, print_items};
use crate::output::CommandOutput;

pub(crate) async fn handle(ctx: &AppContext, command: CollectionCommand) -> Result<CommandOutput> {
    match command {
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
            CommandOutput::new(ctx, payload, None, |_| {
                println!("Item added to collection.")
            })
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
        read => handle_read(ctx, &ctx.local_library()?, read),
    }
}

/// Read-side arms, generic over the two collection faces so tests can drive
/// the full output path with a fake library. Write arms stay in [`handle`]
/// because they go through `ctx.remote()`.
fn handle_read<L: CollectionNav + CollectionContent>(
    ctx: &AppContext,
    library: &L,
    command: CollectionCommand,
) -> Result<CommandOutput> {
    match command {
        CollectionCommand::List => {
            let collections = library.get_collections()?;
            CommandOutput::new(ctx, collections, None, |collections| {
                print_collections(collections, 0)
            })
        }
        CollectionCommand::Get(args) => {
            let collection = library.get_collection(&args.key)?.ok_or_else(|| {
                zot_core::ZotError::InvalidInput {
                    code: "collection-not-found".to_string(),
                    message: format!("Collection '{}' not found", args.key),
                    hint: Some("Use 'zot collection list' to inspect collection keys".to_string()),
                }
            })?;
            CommandOutput::new(ctx, collection, None, |collection| {
                print_collections(std::slice::from_ref(collection), 0)
            })
        }
        CollectionCommand::Subcollections(args) => {
            let collections = library.get_subcollections(&args.key)?;
            CommandOutput::new(ctx, collections, None, |collections| {
                if collections.is_empty() {
                    println!("No subcollections found.");
                } else {
                    print_collections(collections, 0);
                }
            })
        }
        CollectionCommand::Items(args) => {
            let items = library.get_collection_items(&args.key)?;
            CommandOutput::new(ctx, items, None, |items| print_items(items))
        }
        CollectionCommand::Search(args) => {
            let collections =
                library.search_collections(&args.query, resolved_output_limit(args.limit))?;
            CommandOutput::new(ctx, collections, None, |collections| {
                print_collections(collections, 0)
            })
        }
        CollectionCommand::ItemCount(args) => {
            let count = library.get_collection_item_count(&args.key)?;
            let payload = serde_json::json!({ "collection_key": args.key, "item_count": count });
            CommandOutput::new(ctx, payload, None, move |_| println!("{count}"))
        }
        CollectionCommand::Tags(args) => {
            let tags = library.get_collection_tags(&args.key)?;
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
        _ => unreachable!("write commands are dispatched in `handle`"),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use zot_core::{AppConfig, Collection, Item, LibraryScope, TagSummary, ZotError, ZotResult};
    use zot_local::PdfiumBackend;
    use zot_remote::HttpRuntime;

    use super::*;
    use crate::cli::CollectionKeyArgs;

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

    struct FakeCollections {
        collections: Vec<Collection>,
        items: Vec<Item>,
        tags: Vec<TagSummary>,
    }

    fn sample_library() -> FakeCollections {
        FakeCollections {
            collections: vec![Collection {
                key: "COLL1".to_string(),
                name: "Machine Learning".to_string(),
                parent_key: None,
                children: Vec::new(),
            }],
            items: vec![Item {
                key: "ATTN001".to_string(),
                item_type: "journalArticle".to_string(),
                title: "Attention Is All You Need".to_string(),
                creators: vec![],
                abstract_note: None,
                date: None,
                url: None,
                doi: None,
                tags: vec![],
                collections: vec![],
                date_added: None,
                date_modified: None,
                extra: Default::default(),
            }],
            tags: vec![TagSummary {
                name: "transformers".to_string(),
                count: 4,
            }],
        }
    }

    impl CollectionNav for FakeCollections {
        fn get_collections(&self) -> ZotResult<Vec<Collection>> {
            Ok(self.collections.clone())
        }
        fn get_collection(&self, collection_key: &str) -> ZotResult<Option<Collection>> {
            Ok(self
                .collections
                .iter()
                .find(|collection| collection.key == collection_key)
                .cloned())
        }
        fn get_subcollections(&self, _collection_key: &str) -> ZotResult<Vec<Collection>> {
            Ok(Vec::new())
        }
        fn search_collections(&self, _query: &str, _limit: usize) -> ZotResult<Vec<Collection>> {
            Ok(self.collections.clone())
        }
    }

    impl CollectionContent for FakeCollections {
        fn get_collection_items(&self, _collection_key: &str) -> ZotResult<Vec<Item>> {
            Ok(self.items.clone())
        }
        fn get_collection_item_count(&self, _collection_key: &str) -> ZotResult<usize> {
            Ok(self.items.len())
        }
        fn get_collection_tags(&self, _collection_key: &str) -> ZotResult<Vec<TagSummary>> {
            Ok(self.tags.clone())
        }
    }

    fn key_args(key: &str) -> CollectionKeyArgs {
        CollectionKeyArgs {
            key: key.to_string(),
        }
    }

    #[test]
    fn list_envelope_carries_collections() {
        let out = handle_read(&ctx(true), &sample_library(), CollectionCommand::List)
            .expect("list must succeed");
        let json = out.as_json().expect("json payload");
        assert!(json.contains("\"ok\": true"));
        assert!(json.contains("\"key\": \"COLL1\""));
        assert!(json.contains("\"name\": \"Machine Learning\""));
    }

    #[test]
    fn items_envelope_carries_collection_items() {
        let out = handle_read(
            &ctx(true),
            &sample_library(),
            CollectionCommand::Items(crate::cli::CollectionItemsArgs {
                key: "COLL1".to_string(),
            }),
        )
        .expect("items must succeed");
        let json = out.as_json().expect("json payload");
        assert!(json.contains("\"key\": \"ATTN001\""));
        assert!(json.contains("Attention Is All You Need"));
    }

    #[test]
    fn tags_envelope_carries_tag_counts() {
        let out = handle_read(
            &ctx(true),
            &sample_library(),
            CollectionCommand::Tags(key_args("COLL1")),
        )
        .expect("tags must succeed");
        let json = out.as_json().expect("json payload");
        assert!(json.contains("\"name\": \"transformers\""));
        assert!(json.contains("\"count\": 4"));
    }

    #[test]
    fn item_count_payload_keeps_its_shape() {
        let out = handle_read(
            &ctx(true),
            &sample_library(),
            CollectionCommand::ItemCount(key_args("COLL1")),
        )
        .expect("item count must succeed");
        let json = out.as_json().expect("json payload");
        assert!(json.contains("\"collection_key\": \"COLL1\""));
        assert!(json.contains("\"item_count\": 1"));
    }

    #[test]
    fn get_missing_collection_maps_to_stable_error() {
        let err = handle_read(
            &ctx(true),
            &sample_library(),
            CollectionCommand::Get(key_args("NOPE")),
        )
        .expect_err("missing collection must fail");
        let err = err.downcast_ref::<ZotError>().expect("zot error");
        match err {
            ZotError::InvalidInput { code, message, .. } => {
                assert_eq!(code, "collection-not-found");
                assert_eq!(message, "Collection 'NOPE' not found");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn human_mode_produces_no_json() {
        let out = handle_read(&ctx(false), &sample_library(), CollectionCommand::List)
            .expect("list must succeed");
        assert_eq!(out.as_json(), None);
    }
}
