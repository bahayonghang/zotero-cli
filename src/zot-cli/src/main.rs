mod format;

use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;
use std::process::Command;

use anyhow::Result;
use clap::{Args, Parser, Subcommand, ValueEnum};
use format::{
    print_collections, print_enveloped, print_error, print_item, print_items, print_json,
    print_query_chunks, print_stats, print_workspace,
};
use zot_core::{AppConfig, EnvelopeMeta, LibraryScope, redact_secret};
use zot_local::{
    CitationStyle, HybridMode, LocalLibrary, PdfBackend, PdfCache, PdfiumBackend, RagIndex,
    SearchOptions, SortDirection, SortField, WorkspaceStore, build_metadata_chunk, chunk_text,
    compute_term_frequencies, format_citation, tokenize,
};
use zot_remote::{
    EmbeddingClient, PublicationStatus, SemanticScholarClient, ZoteroRemote, extract_preprint_info,
};

#[derive(Parser)]
#[command(name = "zot", version, about = "Rust Zotero CLI")]
struct Cli {
    #[arg(long, global = true)]
    json: bool,
    #[arg(long, global = true)]
    profile: Option<String>,
    #[arg(long, global = true, default_value = "user")]
    library: String,
    #[arg(long, global = true)]
    verbose: bool,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Doctor,
    Library {
        #[command(subcommand)]
        command: LibraryCommand,
    },
    Item {
        #[command(subcommand)]
        command: ItemCommand,
    },
    Collection {
        #[command(subcommand)]
        command: CollectionCommand,
    },
    Workspace {
        #[command(subcommand)]
        command: WorkspaceCommand,
    },
    Sync {
        #[command(subcommand)]
        command: SyncCommand,
    },
    Mcp {
        #[command(subcommand)]
        command: McpCommand,
    },
}

#[derive(Subcommand)]
enum LibraryCommand {
    Search(LibrarySearchArgs),
    List(LibraryListArgs),
    Recent(LibraryRecentArgs),
    Stats,
    Duplicates(LimitArgs),
}

#[derive(Subcommand)]
enum ItemCommand {
    Get(ItemKeyArgs),
    Related(ItemRelatedArgs),
    Open(ItemOpenArgs),
    Pdf(ItemPdfArgs),
    Export(ItemExportArgs),
    Cite(ItemCiteArgs),
    Create(ItemCreateArgs),
    Update(ItemUpdateArgs),
    Trash(ItemKeyArgs),
    Restore(ItemKeyArgs),
    Attach(ItemAttachArgs),
    Note {
        #[command(subcommand)]
        command: ItemNoteCommand,
    },
    Tag {
        #[command(subcommand)]
        command: ItemTagCommand,
    },
}

#[derive(Subcommand)]
enum ItemNoteCommand {
    List(ItemKeyArgs),
    Add(ItemNoteAddArgs),
    Update(ItemNoteUpdateArgs),
}

#[derive(Subcommand)]
enum ItemTagCommand {
    List(ItemKeyArgs),
    Add(ItemTagUpdateArgs),
    Remove(ItemTagUpdateArgs),
}

#[derive(Subcommand)]
enum CollectionCommand {
    List,
    Items(CollectionItemsArgs),
    Create(CollectionCreateArgs),
    Rename(CollectionRenameArgs),
    Delete(CollectionKeyArgs),
    AddItem(CollectionMembershipArgs),
    RemoveItem(CollectionMembershipArgs),
}

#[derive(Subcommand)]
enum WorkspaceCommand {
    New(WorkspaceNewArgs),
    Delete(WorkspaceNameArgs),
    List,
    Show(WorkspaceShowArgs),
    Add(WorkspaceAddArgs),
    Remove(WorkspaceRemoveArgs),
    Import(WorkspaceImportArgs),
    Search(WorkspaceSearchArgs),
    Export(WorkspaceExportArgs),
    Index(WorkspaceNameArgs),
    Query(WorkspaceQueryArgs),
}

#[derive(Subcommand)]
enum SyncCommand {
    UpdateStatus(UpdateStatusArgs),
}

#[derive(Subcommand)]
enum McpCommand {
    Serve,
}

#[derive(Args)]
struct LimitArgs {
    #[arg(long, default_value_t = 50)]
    limit: usize,
}

#[derive(Args)]
struct LibrarySearchArgs {
    query: String,
    #[arg(long)]
    collection: Option<String>,
    #[arg(long = "type")]
    item_type: Option<String>,
    #[arg(long)]
    sort: Option<SortFieldArg>,
    #[arg(long, default_value = "desc")]
    direction: SortDirectionArg,
    #[arg(long, default_value_t = 50)]
    limit: usize,
    #[arg(long, default_value_t = 0)]
    offset: usize,
}

#[derive(Args)]
struct LibraryListArgs {
    #[arg(long)]
    collection: Option<String>,
    #[arg(long, default_value_t = 50)]
    limit: usize,
    #[arg(long, default_value_t = 0)]
    offset: usize,
}

#[derive(Args)]
struct LibraryRecentArgs {
    since: String,
    #[arg(long, default_value = "date-added")]
    sort: SortFieldArg,
    #[arg(long, default_value_t = 50)]
    limit: usize,
}

#[derive(Args)]
struct ItemKeyArgs {
    key: String,
}

#[derive(Args)]
struct ItemRelatedArgs {
    key: String,
    #[arg(long, default_value_t = 20)]
    limit: usize,
}

#[derive(Args)]
struct ItemOpenArgs {
    key: String,
    #[arg(long)]
    url: bool,
}

#[derive(Args)]
struct ItemPdfArgs {
    key: String,
    #[arg(long)]
    pages: Option<String>,
    #[arg(long)]
    annotations: bool,
}

#[derive(Args)]
struct ItemExportArgs {
    key: String,
    #[arg(long, default_value = "bibtex")]
    format: String,
}

#[derive(Args)]
struct ItemCiteArgs {
    key: String,
    #[arg(long, default_value = "apa")]
    style: CitationStyleArg,
}

#[derive(Args)]
struct ItemCreateArgs {
    #[arg(long)]
    doi: Option<String>,
    #[arg(long)]
    url: Option<String>,
    #[arg(long)]
    pdf: Option<PathBuf>,
}

#[derive(Args)]
struct ItemUpdateArgs {
    key: String,
    #[arg(long)]
    title: Option<String>,
    #[arg(long)]
    date: Option<String>,
    #[arg(long = "field")]
    fields: Vec<String>,
}

#[derive(Args)]
struct ItemAttachArgs {
    key: String,
    #[arg(long)]
    file: PathBuf,
}

#[derive(Args)]
struct ItemNoteAddArgs {
    key: String,
    #[arg(long)]
    content: String,
}

#[derive(Args)]
struct ItemNoteUpdateArgs {
    note_key: String,
    #[arg(long)]
    content: String,
}

#[derive(Args)]
struct ItemTagUpdateArgs {
    key: String,
    #[arg(long = "tag")]
    tags: Vec<String>,
}

#[derive(Args)]
struct CollectionItemsArgs {
    key: String,
}

#[derive(Args)]
struct CollectionCreateArgs {
    name: String,
    #[arg(long)]
    parent: Option<String>,
}

#[derive(Args)]
struct CollectionRenameArgs {
    key: String,
    new_name: String,
}

#[derive(Args)]
struct CollectionKeyArgs {
    key: String,
}

#[derive(Args)]
struct CollectionMembershipArgs {
    collection_key: String,
    item_key: String,
}

#[derive(Args)]
struct WorkspaceNewArgs {
    name: String,
    #[arg(long, default_value = "")]
    description: String,
}

#[derive(Args)]
struct WorkspaceNameArgs {
    name: String,
}

#[derive(Args)]
struct WorkspaceShowArgs {
    name: String,
    #[arg(long, default_value_t = 50)]
    limit: usize,
}

#[derive(Args)]
struct WorkspaceAddArgs {
    name: String,
    keys: Vec<String>,
}

#[derive(Args)]
struct WorkspaceRemoveArgs {
    name: String,
    keys: Vec<String>,
}

#[derive(Args)]
struct WorkspaceImportArgs {
    name: String,
    #[arg(long)]
    collection: Option<String>,
    #[arg(long)]
    tag: Option<String>,
    #[arg(long)]
    search: Option<String>,
}

#[derive(Args)]
struct WorkspaceSearchArgs {
    name: String,
    query: String,
}

#[derive(Args)]
struct WorkspaceExportArgs {
    name: String,
    #[arg(long, default_value = "markdown")]
    format: String,
}

#[derive(Args)]
struct WorkspaceQueryArgs {
    name: String,
    question: String,
    #[arg(long, default_value = "hybrid")]
    mode: HybridModeArg,
    #[arg(long, default_value_t = 10)]
    limit: usize,
}

#[derive(Args)]
struct UpdateStatusArgs {
    key: Option<String>,
    #[arg(long)]
    apply: bool,
    #[arg(long)]
    collection: Option<String>,
    #[arg(long, default_value_t = 50)]
    limit: usize,
}

#[derive(Clone, Copy, ValueEnum)]
enum SortFieldArg {
    DateAdded,
    DateModified,
    Title,
    Creator,
}

#[derive(Clone, Copy, ValueEnum)]
enum SortDirectionArg {
    Asc,
    Desc,
}

#[derive(Clone, Copy, ValueEnum)]
enum CitationStyleArg {
    Apa,
    Nature,
    Vancouver,
}

#[derive(Clone, Copy, ValueEnum)]
enum HybridModeArg {
    Bm25,
    Semantic,
    Hybrid,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let json = cli.json;
    if let Err(err) = run(cli).await {
        if let Some(zot_error) = err.downcast_ref::<zot_core::ZotError>() {
            print_error(zot_error, json)?;
            std::process::exit(1);
        }
        eprintln!("{err}");
        std::process::exit(1);
    }
    Ok(())
}

struct AppContext {
    json: bool,
    _verbose: bool,
    profile: Option<String>,
    scope: LibraryScope,
    config: AppConfig,
}

impl AppContext {
    fn local_library(&self) -> zot_core::ZotResult<LocalLibrary> {
        LocalLibrary::open(zot_core::get_data_dir(&self.config), self.scope.clone())
    }

    fn remote(&self) -> zot_core::ZotResult<ZoteroRemote> {
        let library_id = self
            .scope
            .public_id(Some(&self.config.zotero.library_id))
            .ok_or_else(|| zot_core::ZotError::InvalidInput {
                code: "write-credentials".to_string(),
                message: "Missing configured library_id for remote writes".to_string(),
                hint: Some("Run `zot config init` or set ZOT_LIBRARY_ID".to_string()),
            })?;
        if self.config.zotero.api_key.is_empty() {
            return Err(zot_core::ZotError::InvalidInput {
                code: "write-credentials".to_string(),
                message: "Missing Zotero API key".to_string(),
                hint: Some("Run `zot config init` or set ZOT_API_KEY".to_string()),
            });
        }
        ZoteroRemote::new(library_id, &self.config.zotero.api_key, self.scope.clone())
    }
}

async fn run(cli: Cli) -> Result<()> {
    let scope = zot_core::parse_library_scope(&cli.library)?;
    let config = AppConfig::load(cli.profile.as_deref())?;
    let ctx = AppContext {
        json: cli.json,
        _verbose: cli.verbose,
        profile: cli.profile,
        scope,
        config,
    };

    match cli.command {
        Commands::Doctor => handle_doctor(&ctx).await?,
        Commands::Library { command } => handle_library(&ctx, command).await?,
        Commands::Item { command } => handle_item(&ctx, command).await?,
        Commands::Collection { command } => handle_collection(&ctx, command).await?,
        Commands::Workspace { command } => handle_workspace(&ctx, command).await?,
        Commands::Sync { command } => handle_sync(&ctx, command).await?,
        Commands::Mcp { command } => handle_mcp(&ctx, command).await?,
    }
    Ok(())
}

async fn handle_doctor(ctx: &AppContext) -> Result<()> {
    let data_dir = zot_core::get_data_dir(&ctx.config);
    let db_path = data_dir.join("zotero.sqlite");
    let pdf_backend = PdfiumBackend;
    let library = ctx.local_library();
    let schema_version = library
        .as_ref()
        .ok()
        .and_then(|library| library.check_schema_compatibility().ok())
        .flatten();
    let payload = serde_json::json!({
        "config_file": AppConfig::config_file(),
        "data_dir": data_dir,
        "db_exists": db_path.exists(),
        "write_credentials": {
            "configured": ctx.config.write_credentials_configured(),
            "library_id": if ctx.config.zotero.library_id.is_empty() { "(missing)".to_string() } else { ctx.config.zotero.library_id.clone() },
            "api_key": if ctx.config.zotero.api_key.is_empty() { "(missing)".to_string() } else { redact_secret(&ctx.config.zotero.api_key) },
        },
        "embedding": {
            "configured": ctx.config.embedding.is_configured(),
            "url": ctx.config.embedding.url,
            "model": ctx.config.embedding.model,
        },
        "semantic_scholar": {
            "configured": ctx.config.semantic_scholar_key().is_some(),
        },
        "pdf_backend": {
            "available": pdf_backend.availability_hint().is_ok(),
        },
        "schema_version": schema_version,
    });
    if ctx.json {
        print_enveloped(payload, None)?;
    } else {
        println!("Config: {}", AppConfig::config_file().display());
        println!("Data dir: {}", data_dir.display());
        println!("Database exists: {}", db_path.exists());
        println!(
            "Write credentials: {}",
            if ctx.config.write_credentials_configured() {
                "configured"
            } else {
                "missing"
            }
        );
        println!(
            "PDF backend: {}",
            if pdf_backend.availability_hint().is_ok() {
                "available"
            } else {
                "unavailable"
            }
        );
        if let Some(version) = schema_version {
            println!("Schema version: {version}");
        }
    }
    Ok(())
}

async fn handle_library(ctx: &AppContext, command: LibraryCommand) -> Result<()> {
    let library = ctx.local_library()?;
    match command {
        LibraryCommand::Search(args) => {
            let result = library.search(SearchOptions {
                query: args.query,
                collection: args.collection,
                item_type: args.item_type,
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
        LibraryCommand::Duplicates(args) => {
            let groups = library.find_duplicates(args.limit)?;
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
    }
    Ok(())
}

async fn handle_item(ctx: &AppContext, command: ItemCommand) -> Result<()> {
    match command {
        ItemCommand::Get(args) => {
            let library = ctx.local_library()?;
            let item =
                library
                    .get_item(&args.key)?
                    .ok_or_else(|| zot_core::ZotError::InvalidInput {
                        code: "item-not-found".to_string(),
                        message: format!("Item '{}' not found", args.key),
                        hint: None,
                    })?;
            let notes = library.get_notes(&args.key)?;
            let attachments = library.get_attachments(&args.key)?;
            if ctx.json {
                let payload = serde_json::json!({
                    "item": item,
                    "notes": notes,
                    "attachments": attachments,
                });
                print_enveloped(payload, None)?;
            } else {
                print_item(&item, &notes, &attachments);
            }
        }
        ItemCommand::Related(args) => {
            let library = ctx.local_library()?;
            let items = library.get_related_items(&args.key, args.limit)?;
            if ctx.json {
                print_enveloped(&items, None)?;
            } else {
                print_items(&items);
            }
        }
        ItemCommand::Open(args) => handle_item_open(ctx, args).await?,
        ItemCommand::Pdf(args) => handle_item_pdf(ctx, args).await?,
        ItemCommand::Export(args) => {
            let library = ctx.local_library()?;
            let export = library
                .export_citation(&args.key, &args.format)?
                .ok_or_else(|| zot_core::ZotError::InvalidInput {
                    code: "item-not-found".to_string(),
                    message: format!("Item '{}' not found", args.key),
                    hint: None,
                })?;
            if ctx.json {
                print_enveloped(
                    serde_json::json!({ "format": args.format, "content": export }),
                    None,
                )?;
            } else {
                println!("{export}");
            }
        }
        ItemCommand::Cite(args) => {
            let library = ctx.local_library()?;
            let item =
                library
                    .get_item(&args.key)?
                    .ok_or_else(|| zot_core::ZotError::InvalidInput {
                        code: "item-not-found".to_string(),
                        message: format!("Item '{}' not found", args.key),
                        hint: None,
                    })?;
            let citation = format_citation(&item, args.style.into());
            if ctx.json {
                print_enveloped(serde_json::json!({ "citation": citation }), None)?;
            } else {
                println!("{citation}");
            }
        }
        ItemCommand::Create(args) => handle_item_create(ctx, args).await?,
        ItemCommand::Update(args) => {
            let mut fields = BTreeMap::new();
            if let Some(title) = args.title {
                fields.insert("title".to_string(), title);
            }
            if let Some(date) = args.date {
                fields.insert("date".to_string(), date);
            }
            for field in args.fields {
                if let Some((key, value)) = field.split_once('=') {
                    fields.insert(key.to_string(), value.to_string());
                }
            }
            ctx.remote()?.update_item_fields(&args.key, &fields).await?;
            if ctx.json {
                print_enveloped(
                    serde_json::json!({ "updated": args.key, "fields": fields }),
                    None,
                )?;
            } else {
                println!("Updated {}", args.key);
            }
        }
        ItemCommand::Trash(args) => {
            ctx.remote()?.delete_item(&args.key).await?;
            if ctx.json {
                print_enveloped(serde_json::json!({ "trashed": args.key }), None)?;
            } else {
                println!("Moved to trash: {}", args.key);
            }
        }
        ItemCommand::Restore(args) => {
            ctx.remote()?.restore_item(&args.key).await?;
            if ctx.json {
                print_enveloped(serde_json::json!({ "restored": args.key }), None)?;
            } else {
                println!("Restored: {}", args.key);
            }
        }
        ItemCommand::Attach(args) => {
            let key = ctx
                .remote()?
                .upload_attachment(&args.key, &args.file)
                .await?;
            if ctx.json {
                print_enveloped(serde_json::json!({ "attachment_key": key }), None)?;
            } else {
                println!("Attachment uploaded: {key}");
            }
        }
        ItemCommand::Note { command } => handle_item_note(ctx, command).await?,
        ItemCommand::Tag { command } => handle_item_tag(ctx, command).await?,
    }
    Ok(())
}

async fn handle_item_note(ctx: &AppContext, command: ItemNoteCommand) -> Result<()> {
    match command {
        ItemNoteCommand::List(args) => {
            let notes = ctx.local_library()?.get_notes(&args.key)?;
            if ctx.json {
                print_enveloped(&notes, None)?;
            } else {
                for note in notes {
                    println!("{}: {}", note.key, note.content);
                }
            }
        }
        ItemNoteCommand::Add(args) => {
            let key = ctx.remote()?.add_note(&args.key, &args.content).await?;
            if ctx.json {
                print_enveloped(serde_json::json!({ "note_key": key }), None)?;
            } else {
                println!("Note added: {key}");
            }
        }
        ItemNoteCommand::Update(args) => {
            ctx.remote()?
                .update_note(&args.note_key, &args.content)
                .await?;
            if ctx.json {
                print_enveloped(serde_json::json!({ "updated": args.note_key }), None)?;
            } else {
                println!("Note updated: {}", args.note_key);
            }
        }
    }
    Ok(())
}

async fn handle_item_tag(ctx: &AppContext, command: ItemTagCommand) -> Result<()> {
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
    }
    Ok(())
}

async fn handle_collection(ctx: &AppContext, command: CollectionCommand) -> Result<()> {
    match command {
        CollectionCommand::List => {
            let collections = ctx.local_library()?.get_collections()?;
            if ctx.json {
                print_enveloped(&collections, None)?;
            } else {
                print_collections(&collections, 0);
            }
        }
        CollectionCommand::Items(args) => {
            let items = ctx.local_library()?.get_collection_items(&args.key)?;
            if ctx.json {
                print_enveloped(&items, None)?;
            } else {
                print_items(&items);
            }
        }
        CollectionCommand::Create(args) => {
            let key = ctx
                .remote()?
                .create_collection(&args.name, args.parent.as_deref())
                .await?;
            if ctx.json {
                print_enveloped(serde_json::json!({ "collection_key": key }), None)?;
            } else {
                println!("Collection created: {key}");
            }
        }
        CollectionCommand::Rename(args) => {
            ctx.remote()?
                .rename_collection(&args.key, &args.new_name)
                .await?;
            if ctx.json {
                print_enveloped(
                    serde_json::json!({ "renamed": args.key, "name": args.new_name }),
                    None,
                )?;
            } else {
                println!("Collection renamed.");
            }
        }
        CollectionCommand::Delete(args) => {
            ctx.remote()?.delete_collection(&args.key).await?;
            if ctx.json {
                print_enveloped(serde_json::json!({ "deleted": args.key }), None)?;
            } else {
                println!("Collection deleted.");
            }
        }
        CollectionCommand::AddItem(args) => {
            ctx.remote()?
                .add_item_to_collection(&args.item_key, &args.collection_key)
                .await?;
            if ctx.json {
                print_enveloped(
                    serde_json::json!({ "item_key": args.item_key, "collection_key": args.collection_key }),
                    None,
                )?;
            } else {
                println!("Item added to collection.");
            }
        }
        CollectionCommand::RemoveItem(args) => {
            ctx.remote()?
                .remove_item_from_collection(&args.item_key, &args.collection_key)
                .await?;
            if ctx.json {
                print_enveloped(
                    serde_json::json!({ "item_key": args.item_key, "collection_key": args.collection_key }),
                    None,
                )?;
            } else {
                println!("Item removed from collection.");
            }
        }
    }
    Ok(())
}

async fn handle_workspace(ctx: &AppContext, command: WorkspaceCommand) -> Result<()> {
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
        WorkspaceCommand::Import(args) => handle_workspace_import(ctx, &store, args).await?,
        WorkspaceCommand::Search(args) => handle_workspace_search(ctx, &store, args).await?,
        WorkspaceCommand::Export(args) => handle_workspace_export(ctx, &store, args).await?,
        WorkspaceCommand::Index(args) => handle_workspace_index(ctx, &store, &args.name).await?,
        WorkspaceCommand::Query(args) => handle_workspace_query(ctx, &store, args).await?,
    }
    Ok(())
}

async fn handle_sync(ctx: &AppContext, command: SyncCommand) -> Result<()> {
    match command {
        SyncCommand::UpdateStatus(args) => {
            let library = ctx.local_library()?;
            let items = if let Some(key) = args.key.as_deref() {
                library.get_item(key)?.into_iter().collect::<Vec<_>>()
            } else {
                library.get_arxiv_preprints(args.collection.as_deref(), args.limit)?
            };
            let client = SemanticScholarClient::new(ctx.config.semantic_scholar_key())?;
            let mut matches = Vec::new();
            for item in items {
                if let Some(info) = extract_preprint_info(
                    item.url.as_deref(),
                    item.doi.as_deref(),
                    item.extra.get("extra").map(String::as_str),
                ) && let Some(status) = client.check_publication(&info).await?
                {
                    matches.push((item.key.clone(), status));
                }
            }
            if args.apply {
                let remote = ctx.remote()?;
                for (key, status) in &matches {
                    if status.is_published {
                        let mut fields = BTreeMap::new();
                        if let Some(doi) = status.doi.as_deref() {
                            fields.insert("DOI".to_string(), doi.to_string());
                        }
                        if let Some(venue) =
                            status.venue.as_deref().or(status.journal_name.as_deref())
                        {
                            fields.insert("publicationTitle".to_string(), venue.to_string());
                        }
                        if let Some(date) = status.publication_date.as_deref() {
                            fields.insert("date".to_string(), date.to_string());
                        }
                        if !fields.is_empty() {
                            remote.update_item_fields(key, &fields).await?;
                        }
                    }
                }
            }
            let payload = matches
                .into_iter()
                .map(|(key, status)| update_status_to_json(key, status))
                .collect::<Vec<_>>();
            if ctx.json {
                print_enveloped(payload, None)?;
            } else {
                for entry in payload {
                    println!("{}", serde_json::to_string_pretty(&entry)?);
                }
            }
        }
    }
    Ok(())
}

async fn handle_mcp(ctx: &AppContext, command: McpCommand) -> Result<()> {
    match command {
        McpCommand::Serve => {
            let _ = ctx;
            Err(zot_core::ZotError::Unsupported {
                code: "mcp-not-implemented".to_string(),
                message: "MCP server is not implemented yet in this Rust port".to_string(),
                hint: Some(
                    "Use the CLI commands directly until rmcp integration lands".to_string(),
                ),
            }
            .into())
        }
    }
}

async fn handle_item_open(ctx: &AppContext, args: ItemOpenArgs) -> Result<()> {
    let library = ctx.local_library()?;
    let item = library
        .get_item(&args.key)?
        .ok_or_else(|| zot_core::ZotError::InvalidInput {
            code: "item-not-found".to_string(),
            message: format!("Item '{}' not found", args.key),
            hint: None,
        })?;
    let target = if args.url {
        item.url
            .clone()
            .or_else(|| {
                item.doi
                    .as_deref()
                    .map(|doi| format!("https://doi.org/{doi}"))
            })
            .ok_or_else(|| zot_core::ZotError::InvalidInput {
                code: "item-no-url".to_string(),
                message: format!("Item '{}' has no URL or DOI", args.key),
                hint: None,
            })?
    } else {
        let attachment = library.get_pdf_attachment(&args.key)?.ok_or_else(|| {
            zot_core::ZotError::InvalidInput {
                code: "item-no-pdf".to_string(),
                message: format!("Item '{}' has no PDF attachment", args.key),
                hint: None,
            }
        })?;
        library.pdf_path(&attachment).display().to_string()
    };
    open_target(&target)?;
    if ctx.json {
        print_enveloped(serde_json::json!({ "opened": target }), None)?;
    } else {
        println!("Opened {target}");
    }
    Ok(())
}

async fn handle_item_pdf(ctx: &AppContext, args: ItemPdfArgs) -> Result<()> {
    let library = ctx.local_library()?;
    let attachment =
        library
            .get_pdf_attachment(&args.key)?
            .ok_or_else(|| zot_core::ZotError::InvalidInput {
                code: "item-no-pdf".to_string(),
                message: format!("Item '{}' has no PDF attachment", args.key),
                hint: None,
            })?;
    let pdf_path = library.pdf_path(&attachment);
    let backend = PdfiumBackend;
    let cache = PdfCache::new(None)?;
    if args.annotations {
        let annotations = backend.extract_annotations(&pdf_path)?;
        if ctx.json {
            print_enveloped(&annotations, None)?;
        } else {
            for annotation in annotations {
                println!(
                    "[p.{}] {} {}",
                    annotation.page, annotation.annotation_type, annotation.content
                );
            }
        }
        return Ok(());
    }
    let page_range = parse_page_range(args.pages.as_deref())?;
    let text = if page_range.is_none() {
        if let Some(cached) = cache.get(&pdf_path)? {
            cached
        } else {
            let extracted = backend.extract_text(&pdf_path, None)?;
            cache.put(&pdf_path, &extracted)?;
            extracted
        }
    } else {
        backend.extract_text(&pdf_path, page_range)?
    };
    if ctx.json {
        print_enveloped(serde_json::json!({ "text": text }), None)?;
    } else {
        println!("{text}");
    }
    Ok(())
}

async fn handle_item_create(ctx: &AppContext, args: ItemCreateArgs) -> Result<()> {
    let remote = ctx.remote()?;
    let backend = PdfiumBackend;
    let key = if let Some(pdf) = args.pdf.as_deref() {
        let doi = if let Some(doi) = args.doi.as_deref() {
            Some(doi.to_string())
        } else {
            backend.extract_doi(pdf)?
        };
        let doi = doi.ok_or_else(|| zot_core::ZotError::Pdf {
            code: "doi-not-found".to_string(),
            message: "No DOI found in PDF".to_string(),
            hint: Some("Pass --doi to override DOI detection".to_string()),
        })?;
        let key = remote.create_item(Some(&doi), None).await?;
        let _attachment_key = remote.upload_attachment(&key, pdf).await?;
        key
    } else {
        remote
            .create_item(args.doi.as_deref(), args.url.as_deref())
            .await?
    };
    if ctx.json {
        print_enveloped(serde_json::json!({ "key": key }), None)?;
    } else {
        println!("Created item: {key}");
    }
    Ok(())
}

async fn handle_workspace_import(
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

async fn handle_workspace_search(
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

async fn handle_workspace_export(
    ctx: &AppContext,
    store: &WorkspaceStore,
    args: WorkspaceExportArgs,
) -> Result<()> {
    let workspace = store.load(&args.name)?;
    let library = ctx.local_library()?;
    let items = workspace
        .items
        .iter()
        .filter_map(|entry| library.get_item(&entry.key).ok().flatten())
        .collect::<Vec<_>>();
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

async fn handle_workspace_index(
    ctx: &AppContext,
    store: &WorkspaceStore,
    name: &str,
) -> Result<()> {
    let workspace = store.load(name)?;
    let library = ctx.local_library()?;
    let index = RagIndex::open(store.root().join(format!("{name}.idx.sqlite")))?;
    index.clear()?;
    let backend = PdfiumBackend;
    let cache = PdfCache::new(Some(store.root().join(".md_cache.sqlite")))?;
    let embedding_client = EmbeddingClient::new(ctx.config.embedding.clone());
    let mut all_texts = Vec::new();
    let mut chunk_ids = Vec::new();
    for entry in workspace.items {
        if let Some(item) = library.get_item(&entry.key)? {
            let metadata_chunk = build_metadata_chunk(&item);
            let chunk_id = index.insert_chunk(&item.key, "metadata", &metadata_chunk)?;
            index.insert_terms(
                chunk_id,
                &compute_term_frequencies(&zot_local::workspace::tokenize(&metadata_chunk)),
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
        for (chunk_id, embedding) in chunk_ids.into_iter().zip(embeddings.into_iter()) {
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

async fn handle_workspace_query(
    ctx: &AppContext,
    store: &WorkspaceStore,
    args: WorkspaceQueryArgs,
) -> Result<()> {
    let index = RagIndex::open(store.root().join(format!("{}.idx.sqlite", args.name)))?;
    let mode: HybridMode = args.mode.into();
    let embedding = if matches!(mode, HybridMode::Semantic | HybridMode::Hybrid) {
        let client = EmbeddingClient::new(ctx.config.embedding.clone());
        if client.configured() {
            Some(
                client
                    .embed(std::slice::from_ref(&args.question))
                    .await?
                    .into_iter()
                    .next()
                    .unwrap_or_default(),
            )
        } else {
            None
        }
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

fn open_target(target: &str) -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        Command::new("cmd")
            .args(["/C", "start", "", target])
            .spawn()?;
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open").arg(target).spawn()?;
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Command::new("xdg-open").arg(target).spawn()?;
    }
    Ok(())
}

fn parse_page_range(range: Option<&str>) -> Result<Option<(usize, usize)>> {
    let Some(range) = range else {
        return Ok(None);
    };
    let parts = range.split('-').collect::<Vec<_>>();
    let start = parts
        .first()
        .ok_or_else(|| zot_core::ZotError::InvalidInput {
            code: "page-range".to_string(),
            message: "Invalid page range".to_string(),
            hint: None,
        })?
        .parse::<usize>()?;
    let end = if let Some(value) = parts.get(1) {
        value.parse::<usize>()?
    } else {
        start
    };
    Ok(Some((start, end)))
}

fn update_status_to_json(key: String, status: PublicationStatus) -> serde_json::Value {
    serde_json::json!({
        "key": key,
        "preprint_id": status.preprint_id,
        "source": status.source,
        "title": status.title,
        "published": status.is_published,
        "venue": status.venue,
        "journal": status.journal_name,
        "doi": status.doi,
        "date": status.publication_date,
    })
}

impl From<SortFieldArg> for SortField {
    fn from(value: SortFieldArg) -> Self {
        match value {
            SortFieldArg::DateAdded => SortField::DateAdded,
            SortFieldArg::DateModified => SortField::DateModified,
            SortFieldArg::Title => SortField::Title,
            SortFieldArg::Creator => SortField::Creator,
        }
    }
}

impl From<SortDirectionArg> for SortDirection {
    fn from(value: SortDirectionArg) -> Self {
        match value {
            SortDirectionArg::Asc => SortDirection::Asc,
            SortDirectionArg::Desc => SortDirection::Desc,
        }
    }
}

impl From<CitationStyleArg> for CitationStyle {
    fn from(value: CitationStyleArg) -> Self {
        match value {
            CitationStyleArg::Apa => CitationStyle::Apa,
            CitationStyleArg::Nature => CitationStyle::Nature,
            CitationStyleArg::Vancouver => CitationStyle::Vancouver,
        }
    }
}

impl From<HybridModeArg> for HybridMode {
    fn from(value: HybridModeArg) -> Self {
        match value {
            HybridModeArg::Bm25 => HybridMode::Bm25,
            HybridModeArg::Semantic => HybridMode::Semantic,
            HybridModeArg::Hybrid => HybridMode::Hybrid,
        }
    }
}
