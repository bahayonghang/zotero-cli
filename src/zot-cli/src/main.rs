mod format;

use std::collections::{BTreeMap, HashSet};
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;

use anyhow::Result;
use chrono::Utc;
use clap::{Args, Parser, Subcommand, ValueEnum};
use format::{
    print_collections, print_enveloped, print_error, print_item, print_items, print_json,
    print_query_chunks, print_stats, print_workspace,
};
use zot_core::{
    AppConfig, EnvelopeMeta, LibraryScope, PdfOutlineEntry, RetractionCheckResult, SciteItemReport,
    SemanticHit, SemanticIndexStatus, redact_secret,
};
use zot_local::{
    CitationStyle, DuplicateMatchMethod, HybridMode, LocalLibrary, PdfBackend, PdfCache,
    PdfiumBackend, RagIndex, SearchOptions, SortDirection, SortField, WorkspaceStore,
    build_metadata_chunk, chunk_text, compute_term_frequencies, format_citation, tokenize,
};
use zot_remote::oa::CreatorName;
use zot_remote::{
    BetterBibTexClient, EmbeddingClient, OaClient, PublicationStatus, SciteClient,
    SemanticScholarClient, ZoteroRemote, extract_preprint_info, normalize_arxiv_id, normalize_doi,
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
    Citekey(LibraryCiteKeyArgs),
    Tags,
    Libraries,
    Feeds,
    FeedItems(LibraryFeedItemsArgs),
    SemanticSearch(LibrarySemanticSearchArgs),
    SemanticIndex(LibrarySemanticIndexArgs),
    SemanticStatus,
    Duplicates(LibraryDuplicatesArgs),
    DuplicatesMerge(LibraryDuplicatesMergeArgs),
}

#[derive(Subcommand)]
enum ItemCommand {
    Get(ItemKeyArgs),
    Related(ItemRelatedArgs),
    Open(ItemOpenArgs),
    Pdf(ItemPdfArgs),
    Fulltext(ItemPdfArgs),
    Children(ItemChildrenArgs),
    Outline(ItemKeyArgs),
    Export(ItemExportArgs),
    Cite(ItemCiteArgs),
    Create(ItemCreateArgs),
    AddDoi(AddByDoiArgs),
    AddUrl(AddByUrlArgs),
    AddFile(AddFromFileArgs),
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
    Annotation {
        #[command(subcommand)]
        command: ItemAnnotationCommand,
    },
    Scite {
        #[command(subcommand)]
        command: ItemSciteCommand,
    },
}

#[derive(Subcommand)]
enum ItemNoteCommand {
    List(ItemKeyArgs),
    Search(NoteSearchArgs),
    Add(ItemNoteAddArgs),
    Update(ItemNoteUpdateArgs),
    Delete(ItemKeyArgs),
}

#[derive(Subcommand)]
enum ItemTagCommand {
    List(ItemKeyArgs),
    Add(ItemTagUpdateArgs),
    Remove(ItemTagUpdateArgs),
    Batch(ItemTagBatchArgs),
}

#[derive(Subcommand)]
enum ItemAnnotationCommand {
    List(AnnotationListArgs),
    Search(AnnotationSearchArgs),
    Create(AnnotationCreateArgs),
    CreateArea(AnnotationCreateAreaArgs),
}

#[derive(Subcommand)]
enum ItemSciteCommand {
    Report(SciteReportArgs),
    Search(SciteSearchArgs),
    Retractions(SciteRetractionsArgs),
}

#[derive(Subcommand)]
enum CollectionCommand {
    List,
    Items(CollectionItemsArgs),
    Search(CollectionSearchArgs),
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
    tag: Option<String>,
    #[arg(long)]
    creator: Option<String>,
    #[arg(long)]
    year: Option<String>,
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
struct LibraryCiteKeyArgs {
    citekey: String,
}

#[derive(Args)]
struct LibraryFeedItemsArgs {
    library_id: i64,
    #[arg(long, default_value_t = 20)]
    limit: usize,
}

#[derive(Args)]
struct LibrarySemanticSearchArgs {
    query: String,
    #[arg(long, default_value = "hybrid")]
    mode: HybridModeArg,
    #[arg(long)]
    collection: Option<String>,
    #[arg(long, default_value_t = 10)]
    limit: usize,
}

#[derive(Args)]
struct LibrarySemanticIndexArgs {
    #[arg(long)]
    fulltext: bool,
    #[arg(long)]
    force_rebuild: bool,
    #[arg(long)]
    collection: Option<String>,
    #[arg(long, default_value_t = 0)]
    limit: usize,
}

#[derive(Args)]
struct LibraryDuplicatesArgs {
    #[arg(long, default_value = "both")]
    method: DuplicateMethodArg,
    #[arg(long)]
    collection: Option<String>,
    #[arg(long, default_value_t = 50)]
    limit: usize,
}

#[derive(Args)]
struct LibraryDuplicatesMergeArgs {
    #[arg(long)]
    keeper: String,
    #[arg(long = "duplicate")]
    duplicates: Vec<String>,
    #[arg(long)]
    confirm: bool,
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
struct ItemChildrenArgs {
    keys: Vec<String>,
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
    #[arg(long = "collection")]
    collections: Vec<String>,
    #[arg(long = "tag")]
    tags: Vec<String>,
    #[arg(long, default_value = "auto")]
    attach_mode: AttachModeArg,
}

#[derive(Args)]
struct AddByDoiArgs {
    doi: String,
    #[arg(long = "collection")]
    collections: Vec<String>,
    #[arg(long = "tag")]
    tags: Vec<String>,
    #[arg(long, default_value = "auto")]
    attach_mode: AttachModeArg,
}

#[derive(Args)]
struct AddByUrlArgs {
    url: String,
    #[arg(long = "collection")]
    collections: Vec<String>,
    #[arg(long = "tag")]
    tags: Vec<String>,
    #[arg(long, default_value = "auto")]
    attach_mode: AttachModeArg,
}

#[derive(Args)]
struct AddFromFileArgs {
    file: PathBuf,
    #[arg(long)]
    title: Option<String>,
    #[arg(long, default_value = "document")]
    item_type: String,
    #[arg(long)]
    doi: Option<String>,
    #[arg(long = "collection")]
    collections: Vec<String>,
    #[arg(long = "tag")]
    tags: Vec<String>,
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
struct NoteSearchArgs {
    query: String,
    #[arg(long, default_value_t = 20)]
    limit: usize,
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
struct ItemTagBatchArgs {
    #[arg(long, default_value = "")]
    query: String,
    #[arg(long)]
    tag: Option<String>,
    #[arg(long = "add-tag")]
    add_tags: Vec<String>,
    #[arg(long = "remove-tag")]
    remove_tags: Vec<String>,
    #[arg(long, default_value_t = 50)]
    limit: usize,
}

#[derive(Args)]
struct CollectionItemsArgs {
    key: String,
}

#[derive(Args)]
struct CollectionSearchArgs {
    query: String,
    #[arg(long, default_value_t = 20)]
    limit: usize,
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

#[derive(Args)]
struct AnnotationListArgs {
    #[arg(long)]
    item_key: Option<String>,
    #[arg(long, default_value_t = 50)]
    limit: usize,
}

#[derive(Args)]
struct AnnotationSearchArgs {
    query: String,
    #[arg(long, default_value_t = 20)]
    limit: usize,
}

#[derive(Args)]
struct AnnotationCreateArgs {
    attachment_key: String,
    #[arg(long)]
    page: usize,
    #[arg(long)]
    text: String,
    #[arg(long)]
    comment: Option<String>,
    #[arg(long, default_value = "#ffd400")]
    color: String,
}

#[derive(Args)]
struct AnnotationCreateAreaArgs {
    attachment_key: String,
    #[arg(long)]
    page: usize,
    #[arg(long)]
    x: f32,
    #[arg(long)]
    y: f32,
    #[arg(long)]
    width: f32,
    #[arg(long)]
    height: f32,
    #[arg(long)]
    comment: Option<String>,
    #[arg(long, default_value = "#ffd400")]
    color: String,
}

#[derive(Args)]
struct SciteReportArgs {
    #[arg(long)]
    item_key: Option<String>,
    #[arg(long)]
    doi: Option<String>,
}

#[derive(Args)]
struct SciteSearchArgs {
    query: String,
    #[arg(long, default_value_t = 10)]
    limit: usize,
}

#[derive(Args)]
struct SciteRetractionsArgs {
    #[arg(long)]
    collection: Option<String>,
    #[arg(long)]
    tag: Option<String>,
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

#[derive(Clone, Copy, ValueEnum)]
enum AttachModeArg {
    Auto,
    LinkedUrl,
    None,
}

#[derive(Clone, Copy, ValueEnum)]
enum DuplicateMethodArg {
    Title,
    Doi,
    Both,
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

    fn library_index_path(&self) -> PathBuf {
        let scope = match &self.scope {
            LibraryScope::User => "user".to_string(),
            LibraryScope::Group { group_id } => format!("group-{group_id}"),
        };
        AppConfig::config_dir()
            .join("indexes")
            .join(format!("{scope}.idx.sqlite"))
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
    let libraries = library
        .as_ref()
        .ok()
        .and_then(|library| library.get_libraries().ok())
        .unwrap_or_default();
    let feeds = library
        .as_ref()
        .ok()
        .and_then(|library| library.get_feeds().ok())
        .unwrap_or_default();
    let bbt = BetterBibTexClient::new();
    let semantic_status = library_semantic_status(ctx).await.ok();
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
        "better_bibtex": {
            "available": bbt.probe().await,
        },
        "libraries": {
            "count": libraries.len(),
            "feeds_available": !feeds.is_empty(),
        },
        "semantic_index": semantic_status,
        "annotation_support": {
            "pdf_outline": pdf_backend.availability_hint().is_ok(),
            "annotation_creation": ctx.config.write_credentials_configured() && pdf_backend.availability_hint().is_ok(),
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
        println!(
            "Better BibTeX: {}",
            if bbt.probe().await {
                "available"
            } else {
                "unavailable"
            }
        );
        println!("Libraries discovered: {}", libraries.len());
        println!("Feeds discovered: {}", feeds.len());
        if let Some(status) = semantic_status {
            println!(
                "Semantic index: {} (items={}, chunks={})",
                if status.exists { "present" } else { "missing" },
                status.indexed_items,
                status.indexed_chunks
            );
        }
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
            let hits = library_semantic_search(ctx, &library, args).await?;
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
            let payload = library_semantic_index(ctx, &library, args).await?;
            if ctx.json {
                print_enveloped(payload, None)?;
            } else {
                println!("Library semantic index updated.");
            }
        }
        LibraryCommand::SemanticStatus => {
            let status = library_semantic_status(ctx).await?;
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
        ItemCommand::Fulltext(args) => handle_item_pdf(ctx, args).await?,
        ItemCommand::Children(args) => {
            let children = ctx.local_library()?.get_items_children(&args.keys)?;
            if ctx.json {
                print_enveloped(&children, None)?;
            } else {
                for (key, values) in children {
                    println!("{key}");
                    for value in values {
                        println!("  - {} [{}]", value.key, value.item_type);
                    }
                }
            }
        }
        ItemCommand::Outline(args) => handle_item_outline(ctx, &args.key).await?,
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
        ItemCommand::AddDoi(args) => {
            let key = add_item_by_doi(
                ctx,
                &args.doi,
                &args.collections,
                &args.tags,
                args.attach_mode,
            )
            .await?;
            if ctx.json {
                print_enveloped(serde_json::json!({ "key": key }), None)?;
            } else {
                println!("Created item: {key}");
            }
        }
        ItemCommand::AddUrl(args) => {
            let key = add_item_by_url(
                ctx,
                &args.url,
                &args.collections,
                &args.tags,
                args.attach_mode,
            )
            .await?;
            if ctx.json {
                print_enveloped(serde_json::json!({ "key": key }), None)?;
            } else {
                println!("Created item: {key}");
            }
        }
        ItemCommand::AddFile(args) => {
            let key = add_item_from_file(
                ctx,
                &args.file,
                args.title.as_deref(),
                &args.item_type,
                args.doi.as_deref(),
                &args.collections,
                &args.tags,
            )
            .await?;
            if ctx.json {
                print_enveloped(serde_json::json!({ "key": key }), None)?;
            } else {
                println!("Created item: {key}");
            }
        }
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
        ItemCommand::Annotation { command } => handle_item_annotation(ctx, command).await?,
        ItemCommand::Scite { command } => handle_item_scite(ctx, command).await?,
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
        ItemNoteCommand::Search(args) => {
            let notes = ctx.local_library()?.search_notes(&args.query, args.limit)?;
            if ctx.json {
                print_enveloped(&notes, None)?;
            } else {
                for note in notes {
                    println!(
                        "{} [{}] {}",
                        note.key,
                        note.parent_title.unwrap_or_else(|| "Unknown".to_string()),
                        note.content
                    );
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
        ItemNoteCommand::Delete(args) => {
            ctx.remote()?.delete_note(&args.key).await?;
            if ctx.json {
                print_enveloped(serde_json::json!({ "trashed": args.key }), None)?;
            } else {
                println!("Note moved to trash: {}", args.key);
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

async fn handle_item_annotation(ctx: &AppContext, command: ItemAnnotationCommand) -> Result<()> {
    match command {
        ItemAnnotationCommand::List(args) => {
            let annotations = ctx
                .local_library()?
                .get_annotations(args.item_key.as_deref(), args.limit)?;
            if ctx.json {
                print_enveloped(&annotations, None)?;
            } else if annotations.is_empty() {
                println!("No annotations found.");
            } else {
                for annotation in annotations {
                    println!(
                        "{} [{}] {}",
                        annotation.key, annotation.annotation_type, annotation.text
                    );
                }
            }
        }
        ItemAnnotationCommand::Search(args) => {
            let annotations = ctx
                .local_library()?
                .search_annotations(&args.query, args.limit)?;
            if ctx.json {
                print_enveloped(&annotations, None)?;
            } else if annotations.is_empty() {
                println!("No annotations found.");
            } else {
                for annotation in annotations {
                    println!(
                        "{} [{}] {}",
                        annotation.key, annotation.annotation_type, annotation.text
                    );
                }
            }
        }
        ItemAnnotationCommand::Create(args) => {
            let payload = create_highlight_annotation(
                ctx,
                &args.attachment_key,
                args.page,
                &args.text,
                args.comment.as_deref(),
                &args.color,
            )
            .await?;
            if ctx.json {
                print_enveloped(payload, None)?;
            } else {
                println!("{}", serde_json::to_string_pretty(&payload)?);
            }
        }
        ItemAnnotationCommand::CreateArea(args) => {
            let payload = create_area_annotation(ctx, &args).await?;
            if ctx.json {
                print_enveloped(payload, None)?;
            } else {
                println!("{}", serde_json::to_string_pretty(&payload)?);
            }
        }
    }
    Ok(())
}

async fn handle_item_scite(ctx: &AppContext, command: ItemSciteCommand) -> Result<()> {
    match command {
        ItemSciteCommand::Report(args) => {
            let report = scite_report(ctx, args.item_key.as_deref(), args.doi.as_deref()).await?;
            if ctx.json {
                print_enveloped(&report, None)?;
            } else {
                println!("{}", serde_json::to_string_pretty(&report)?);
            }
        }
        ItemSciteCommand::Search(args) => {
            let reports = scite_search(ctx, &args.query, args.limit).await?;
            if ctx.json {
                print_enveloped(&reports, None)?;
            } else {
                println!("{}", serde_json::to_string_pretty(&reports)?);
            }
        }
        ItemSciteCommand::Retractions(args) => {
            let reports = scite_retractions(
                ctx,
                args.collection.as_deref(),
                args.tag.as_deref(),
                args.limit,
            )
            .await?;
            if ctx.json {
                print_enveloped(&reports, None)?;
            } else if reports.is_empty() {
                println!("No editorial notices found.");
            } else {
                println!("{}", serde_json::to_string_pretty(&reports)?);
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
        CollectionCommand::Search(args) => {
            let collections = ctx
                .local_library()?
                .search_collections(&args.query, args.limit)?;
            if ctx.json {
                print_enveloped(&collections, None)?;
            } else {
                print_collections(&collections, 0);
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
    let key = if let Some(pdf) = args.pdf.as_deref() {
        add_item_from_file(
            ctx,
            pdf,
            None,
            "document",
            args.doi.as_deref(),
            &args.collections,
            &args.tags,
        )
        .await?
    } else if let Some(doi) = args.doi.as_deref() {
        add_item_by_doi(ctx, doi, &args.collections, &args.tags, args.attach_mode).await?
    } else if let Some(url) = args.url.as_deref() {
        add_item_by_url(ctx, url, &args.collections, &args.tags, args.attach_mode).await?
    } else {
        return Err(zot_core::ZotError::InvalidInput {
            code: "item-create".to_string(),
            message: "Provide --doi, --url, or --pdf".to_string(),
            hint: None,
        }
        .into());
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

async fn handle_item_outline(ctx: &AppContext, key: &str) -> Result<()> {
    let library = ctx.local_library()?;
    let attachment =
        library
            .get_pdf_attachment(key)?
            .ok_or_else(|| zot_core::ZotError::InvalidInput {
                code: "item-no-pdf".to_string(),
                message: format!("Item '{}' has no PDF attachment", key),
                hint: None,
            })?;
    let backend = PdfiumBackend;
    let entries = backend.extract_outline(&library.pdf_path(&attachment))?;
    if ctx.json {
        print_enveloped(&entries, None)?;
    } else if entries.is_empty() {
        println!("This PDF does not contain a table of contents/outline.");
    } else {
        print_outline_entries(&entries);
    }
    Ok(())
}

async fn library_semantic_status(ctx: &AppContext) -> Result<SemanticIndexStatus> {
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

async fn library_semantic_index(
    ctx: &AppContext,
    library: &LocalLibrary,
    args: LibrarySemanticIndexArgs,
) -> Result<serde_json::Value> {
    let path = ctx.library_index_path();
    let index = RagIndex::open(&path)?;
    if args.force_rebuild || path.exists() {
        index.clear()?;
    }
    let backend = PdfiumBackend;
    let cache = PdfCache::new(Some(
        AppConfig::config_dir()
            .join("cache")
            .join("library_md_cache.sqlite"),
    ))?;
    let embedding_client = EmbeddingClient::new(ctx.config.embedding.clone());
    let mut items = if let Some(collection) = args.collection.as_deref() {
        library.get_collection_items(collection)?
    } else {
        let limit = if args.limit == 0 { 10_000 } else { args.limit };
        library.list_items(None, limit, 0)?
    };
    if args.limit > 0 {
        items.truncate(args.limit);
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
        for (chunk_id, embedding) in chunk_ids.into_iter().zip(embeddings.into_iter()) {
            index.set_embedding(chunk_id, &embedding)?;
        }
    }
    index.set_meta("indexed_at", &Utc::now().to_rfc3339())?;
    let status = library_semantic_status(ctx).await?;
    Ok(serde_json::json!({
        "indexed": true,
        "items": items.len(),
        "fulltext": args.fulltext,
        "status": status,
    }))
}

async fn library_semantic_search(
    ctx: &AppContext,
    library: &LocalLibrary,
    args: LibrarySemanticSearchArgs,
) -> Result<Vec<SemanticHit>> {
    let index = RagIndex::open(ctx.library_index_path())?;
    let mut mode: HybridMode = args.mode.into();
    let embedding = if matches!(mode, HybridMode::Semantic | HybridMode::Hybrid) {
        let client = EmbeddingClient::new(ctx.config.embedding.clone());
        if client.configured() {
            Some(
                client
                    .embed(std::slice::from_ref(&args.query))
                    .await?
                    .into_iter()
                    .next()
                    .unwrap_or_default(),
            )
        } else {
            mode = HybridMode::Bm25;
            None
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
        args.limit.saturating_mul(5).max(args.limit),
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

async fn batch_update_tags(ctx: &AppContext, args: ItemTagBatchArgs) -> Result<serde_json::Value> {
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

async fn add_item_by_doi(
    ctx: &AppContext,
    doi: &str,
    collections: &[String],
    tags: &[String],
    attach_mode: AttachModeArg,
) -> Result<String> {
    let doi = normalize_doi(doi).ok_or_else(|| zot_core::ZotError::InvalidInput {
        code: "invalid-doi".to_string(),
        message: format!("'{}' does not appear to be a valid DOI", doi),
        hint: None,
    })?;
    let oa = OaClient::new();
    let work = oa.fetch_crossref_work(&doi).await?;
    let remote = ctx.remote()?;
    let key = remote
        .create_item_from_value(build_crossref_item_payload(&work, collections, tags))
        .await?;
    if !matches!(attach_mode, AttachModeArg::None) {
        maybe_attach_open_access_pdf(ctx, &remote, &key, &doi, Some(&work), attach_mode).await?;
    }
    Ok(key)
}

async fn add_item_by_url(
    ctx: &AppContext,
    url: &str,
    collections: &[String],
    tags: &[String],
    attach_mode: AttachModeArg,
) -> Result<String> {
    if let Some(doi) = normalize_doi(url) {
        return add_item_by_doi(ctx, &doi, collections, tags, attach_mode).await;
    }
    let remote = ctx.remote()?;
    if let Some(arxiv_id) = normalize_arxiv_id(url) {
        let work = OaClient::new().fetch_arxiv_work(&arxiv_id).await?;
        let key = remote
            .create_item_from_value(build_arxiv_item_payload(&work, collections, tags))
            .await?;
        if !matches!(attach_mode, AttachModeArg::None) {
            maybe_attach_pdf_url(
                &remote,
                &key,
                &work.pdf_url,
                &format!("arxiv_{}.pdf", arxiv_id.replace('/', "_")),
                attach_mode,
            )
            .await?;
        }
        return Ok(key);
    }
    remote
        .create_item_from_value(serde_json::json!({
            "itemType": "webpage",
            "title": url,
            "url": url,
            "accessDate": "",
            "collections": collections,
            "tags": tags.iter().map(|tag| serde_json::json!({ "tag": tag })).collect::<Vec<_>>(),
        }))
        .await
        .map_err(Into::into)
}

async fn add_item_from_file(
    ctx: &AppContext,
    file: &std::path::Path,
    title: Option<&str>,
    item_type: &str,
    doi_override: Option<&str>,
    collections: &[String],
    tags: &[String],
) -> Result<String> {
    let backend = PdfiumBackend;
    let resolved_doi = if let Some(doi) = doi_override {
        Some(
            normalize_doi(doi).ok_or_else(|| zot_core::ZotError::InvalidInput {
                code: "invalid-doi".to_string(),
                message: format!("'{}' does not appear to be a valid DOI", doi),
                hint: None,
            })?,
        )
    } else if file
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("pdf"))
        .unwrap_or(false)
    {
        backend.extract_doi(file)?
    } else {
        None
    };
    let remote = ctx.remote()?;
    let key = if let Some(doi) = resolved_doi.as_deref() {
        let key = add_item_by_doi(ctx, doi, collections, tags, AttachModeArg::None).await?;
        remote.upload_attachment(&key, file).await?;
        key
    } else {
        let payload = serde_json::json!({
            "itemType": item_type,
            "title": title.unwrap_or_else(|| file.file_name().and_then(|name| name.to_str()).unwrap_or("document")),
            "collections": collections,
            "tags": tags.iter().map(|tag| serde_json::json!({ "tag": tag })).collect::<Vec<_>>(),
        });
        let key = remote.create_item_from_value(payload).await?;
        remote.upload_attachment(&key, file).await?;
        key
    };
    Ok(key)
}

async fn maybe_attach_open_access_pdf(
    _ctx: &AppContext,
    remote: &ZoteroRemote,
    item_key: &str,
    doi: &str,
    crossref: Option<&zot_remote::CrossRefWork>,
    attach_mode: AttachModeArg,
) -> Result<()> {
    if matches!(attach_mode, AttachModeArg::None) {
        return Ok(());
    }
    if let Some(resolved) = OaClient::new()
        .resolve_open_access_pdf(doi, crossref)
        .await?
    {
        maybe_attach_pdf_url(
            remote,
            item_key,
            &resolved.url,
            &format!("{}.pdf", doi.replace('/', "_")),
            attach_mode,
        )
        .await?;
    }
    Ok(())
}

async fn maybe_attach_pdf_url(
    remote: &ZoteroRemote,
    item_key: &str,
    url: &str,
    filename: &str,
    attach_mode: AttachModeArg,
) -> Result<()> {
    match attach_mode {
        AttachModeArg::None => {}
        AttachModeArg::LinkedUrl => {
            remote
                .add_linked_attachment(item_key, url, "PDF (linked URL)")
                .await?;
        }
        AttachModeArg::Auto => {
            let response = reqwest::Client::new()
                .get(url)
                .send()
                .await
                .map_err(|err| zot_core::ZotError::Remote {
                    code: "pdf-download".to_string(),
                    message: err.to_string(),
                    hint: None,
                    status: err.status().map(|status| status.as_u16()),
                })?;
            if !response.status().is_success() {
                return Ok(());
            }
            let bytes = response
                .bytes()
                .await
                .map_err(|err| zot_core::ZotError::Remote {
                    code: "pdf-download-bytes".to_string(),
                    message: err.to_string(),
                    hint: None,
                    status: err.status().map(|status| status.as_u16()),
                })?;
            let path = std::env::temp_dir().join(format!("{}-{}", uuid::Uuid::new_v4(), filename));
            let mut file =
                std::fs::File::create(&path).map_err(|source| zot_core::ZotError::Io {
                    path: path.clone(),
                    source,
                })?;
            file.write_all(&bytes)
                .map_err(|source| zot_core::ZotError::Io {
                    path: path.clone(),
                    source,
                })?;
            let upload_result = remote.upload_attachment(item_key, &path).await;
            let _ = std::fs::remove_file(&path);
            upload_result?;
        }
    }
    Ok(())
}

fn build_crossref_item_payload(
    work: &zot_remote::CrossRefWork,
    collections: &[String],
    tags: &[String],
) -> serde_json::Value {
    serde_json::json!({
        "itemType": crossref_type_to_zotero(&work.record_type),
        "title": work.title.clone().unwrap_or_else(|| work.doi.clone()),
        "creators": work.creators.iter().map(creator_to_json).collect::<Vec<_>>(),
        "date": work.date,
        "DOI": work.doi,
        "url": work.url,
        "volume": work.volume,
        "issue": work.issue,
        "pages": work.pages,
        "publisher": work.publisher,
        "ISSN": work.issn,
        "publicationTitle": work.publication_title,
        "abstractNote": work.abstract_note,
        "collections": collections,
        "tags": tags.iter().map(|tag| serde_json::json!({ "tag": tag })).collect::<Vec<_>>(),
    })
}

fn build_arxiv_item_payload(
    work: &zot_remote::ArxivWork,
    collections: &[String],
    tags: &[String],
) -> serde_json::Value {
    serde_json::json!({
        "itemType": "preprint",
        "title": work.title,
        "creators": work.creators.iter().map(creator_to_json).collect::<Vec<_>>(),
        "abstractNote": work.abstract_note,
        "date": work.date,
        "url": work.abs_url,
        "extra": format!("arXiv:{}", work.arxiv_id),
        "collections": collections,
        "tags": tags.iter().map(|tag| serde_json::json!({ "tag": tag })).collect::<Vec<_>>(),
    })
}

fn creator_to_json(creator: &CreatorName) -> serde_json::Value {
    serde_json::json!({
        "creatorType": creator.creator_type,
        "firstName": creator.first_name,
        "lastName": creator.last_name,
    })
}

fn crossref_type_to_zotero(value: &str) -> &'static str {
    match value {
        "journal-article" => "journalArticle",
        "book" => "book",
        "book-chapter" => "bookSection",
        "proceedings-article" => "conferencePaper",
        "report" => "report",
        "dissertation" => "thesis",
        "posted-content" => "preprint",
        _ => "document",
    }
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

async fn create_highlight_annotation(
    ctx: &AppContext,
    attachment_key: &str,
    page: usize,
    text: &str,
    comment: Option<&str>,
    color: &str,
) -> Result<serde_json::Value> {
    let library = ctx.local_library()?;
    let attachment = library
        .get_attachment_by_key(attachment_key)?
        .ok_or_else(|| zot_core::ZotError::InvalidInput {
            code: "attachment-not-found".to_string(),
            message: format!("Attachment '{}' not found", attachment_key),
            hint: None,
        })?;
    if attachment.content_type != "application/pdf" {
        return Err(zot_core::ZotError::InvalidInput {
            code: "attachment-not-pdf".to_string(),
            message: format!("Attachment '{}' is not a PDF attachment", attachment_key),
            hint: None,
        }
        .into());
    }
    let pdf_path = library.pdf_path(&attachment);
    let backend = PdfiumBackend;
    let position = backend
        .find_text_position(&pdf_path, page, text)?
        .ok_or_else(|| zot_core::ZotError::Pdf {
            code: "annotation-text-not-found".to_string(),
            message: "Could not find the requested text on the target page".to_string(),
            hint: Some("Try a shorter exact phrase copied from the PDF".to_string()),
        })?;
    let payload = serde_json::json!({
        "itemType": "annotation",
        "parentItem": attachment_key,
        "annotationType": "highlight",
        "annotationText": text,
        "annotationComment": comment.unwrap_or(""),
        "annotationColor": color,
        "annotationSortIndex": position.sort_index,
        "annotationPosition": build_annotation_position_json(position.page_index, &position.rects),
        "annotationPageLabel": position.page_label,
    });
    let key = ctx.remote()?.create_item_from_value(payload).await?;
    Ok(serde_json::json!({
        "annotation_key": key,
        "page": position.page_label,
        "text": text,
        "color": color,
    }))
}

async fn create_area_annotation(
    ctx: &AppContext,
    args: &AnnotationCreateAreaArgs,
) -> Result<serde_json::Value> {
    let library = ctx.local_library()?;
    let attachment = library
        .get_attachment_by_key(&args.attachment_key)?
        .ok_or_else(|| zot_core::ZotError::InvalidInput {
            code: "attachment-not-found".to_string(),
            message: format!("Attachment '{}' not found", args.attachment_key),
            hint: None,
        })?;
    if attachment.content_type != "application/pdf" {
        return Err(zot_core::ZotError::InvalidInput {
            code: "attachment-not-pdf".to_string(),
            message: format!(
                "Attachment '{}' is not a PDF attachment",
                args.attachment_key
            ),
            hint: None,
        }
        .into());
    }
    let pdf_path = library.pdf_path(&attachment);
    let backend = PdfiumBackend;
    let position = backend.build_area_position(
        &pdf_path,
        args.page,
        args.x,
        args.y,
        args.width,
        args.height,
    )?;
    let payload = serde_json::json!({
        "itemType": "annotation",
        "parentItem": args.attachment_key,
        "annotationType": "image",
        "annotationComment": args.comment.as_deref().unwrap_or(""),
        "annotationColor": args.color,
        "annotationSortIndex": position.sort_index,
        "annotationPosition": build_annotation_position_json(position.page_index, &position.rects),
        "annotationPageLabel": position.page_label,
    });
    let key = ctx.remote()?.create_item_from_value(payload).await?;
    Ok(serde_json::json!({
        "annotation_key": key,
        "page": position.page_label,
        "rects": position.rects,
        "color": args.color,
    }))
}

fn build_annotation_position_json(page_index: usize, rects: &[[f32; 4]]) -> String {
    serde_json::json!({
        "pageIndex": page_index,
        "rects": rects,
    })
    .to_string()
}

async fn scite_report(
    ctx: &AppContext,
    item_key: Option<&str>,
    doi: Option<&str>,
) -> Result<SciteItemReport> {
    let resolved_doi = if let Some(doi) = doi {
        normalize_doi(doi).ok_or_else(|| zot_core::ZotError::InvalidInput {
            code: "invalid-doi".to_string(),
            message: format!("'{}' does not appear to be a valid DOI", doi),
            hint: None,
        })?
    } else if let Some(item_key) = item_key {
        let item = ctx.local_library()?.get_item(item_key)?.ok_or_else(|| {
            zot_core::ZotError::InvalidInput {
                code: "item-not-found".to_string(),
                message: format!("Item '{}' not found", item_key),
                hint: None,
            }
        })?;
        item.doi.ok_or_else(|| zot_core::ZotError::InvalidInput {
            code: "item-no-doi".to_string(),
            message: format!("Item '{}' has no DOI", item_key),
            hint: None,
        })?
    } else {
        return Err(zot_core::ZotError::InvalidInput {
            code: "scite-target".to_string(),
            message: "Provide --item-key or --doi".to_string(),
            hint: None,
        }
        .into());
    };
    SciteClient::new()
        .get_report(&resolved_doi)
        .await?
        .ok_or_else(|| {
            zot_core::ZotError::Remote {
                code: "scite-not-found".to_string(),
                message: format!("No Scite data found for DOI {}", resolved_doi),
                hint: None,
                status: None,
            }
            .into()
        })
}

async fn scite_search(
    ctx: &AppContext,
    query: &str,
    limit: usize,
) -> Result<Vec<serde_json::Value>> {
    let library = ctx.local_library()?;
    let items = library
        .search(SearchOptions {
            query: query.to_string(),
            limit,
            ..SearchOptions::default()
        })?
        .items;
    let dois = items
        .iter()
        .filter_map(|item| item.doi.clone())
        .collect::<Vec<_>>();
    let reports = SciteClient::new().get_reports_batch(&dois).await?;
    Ok(items
        .into_iter()
        .map(|item| {
            serde_json::json!({
                "item": item,
                "scite": item.doi.as_deref().and_then(|doi| reports.get(doi)),
            })
        })
        .collect())
}

async fn scite_retractions(
    ctx: &AppContext,
    collection: Option<&str>,
    tag: Option<&str>,
    limit: usize,
) -> Result<Vec<RetractionCheckResult>> {
    let library = ctx.local_library()?;
    let mut items = if let Some(collection) = collection {
        library.get_collection_items(collection)?
    } else {
        library.list_items(None, limit, 0)?
    };
    if let Some(tag) = tag {
        items.retain(|item| item.tags.iter().any(|value| value == tag));
    }
    items.truncate(limit);
    let dois = items
        .iter()
        .filter_map(|item| item.doi.clone())
        .collect::<Vec<_>>();
    let reports = SciteClient::new().get_reports_batch(&dois).await?;
    Ok(items
        .into_iter()
        .filter_map(|item| {
            item.doi
                .as_deref()
                .and_then(|doi| reports.get(doi))
                .filter(|report| !report.notices.is_empty())
                .map(|report| RetractionCheckResult {
                    item,
                    notices: report.notices.clone(),
                })
        })
        .collect())
}

fn print_outline_entries(entries: &[PdfOutlineEntry]) {
    for entry in entries {
        let indent = "  ".repeat(entry.level.saturating_sub(1));
        if let Some(page) = entry.page {
            println!("{indent}- {} (p. {page})", entry.title);
        } else {
            println!("{indent}- {}", entry.title);
        }
    }
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

impl From<DuplicateMethodArg> for DuplicateMatchMethod {
    fn from(value: DuplicateMethodArg) -> Self {
        match value {
            DuplicateMethodArg::Title => DuplicateMatchMethod::Title,
            DuplicateMethodArg::Doi => DuplicateMatchMethod::Doi,
            DuplicateMethodArg::Both => DuplicateMatchMethod::Both,
        }
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::Cli;

    #[test]
    fn parses_new_library_and_item_command_surfaces() {
        for argv in [
            ["zot", "library", "semantic-status"].as_slice(),
            ["zot", "library", "citekey", "Smith2024"].as_slice(),
            ["zot", "library", "duplicates", "--method", "both"].as_slice(),
            ["zot", "item", "children", "ATTN001"].as_slice(),
            ["zot", "item", "annotation", "search", "core"].as_slice(),
            ["zot", "item", "scite", "search", "attention"].as_slice(),
            [
                "zot",
                "item",
                "scite",
                "retractions",
                "--tag",
                "reading-list",
            ]
            .as_slice(),
            ["zot", "collection", "search", "Transform"].as_slice(),
        ] {
            if let Err(err) = Cli::try_parse_from(argv) {
                panic!("cli parse failed for {:?}: {err}", argv);
            }
        }
    }
}
