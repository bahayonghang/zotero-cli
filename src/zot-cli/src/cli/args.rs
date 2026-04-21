//! All clap `Args` structs and sub-`Subcommand` enums.
//!
//! Separated from `cli.rs` (which keeps the top-level `Cli` / `Commands`
//! entrypoint and shared value-enums) to stop a single 900-line file from
//! drifting further. Callers continue to reach these types via
//! `crate::cli::LibraryCommand` et al. thanks to the `pub use args::*;` at
//! the top of `cli.rs`.

use std::path::PathBuf;

use clap::{Args, Subcommand};

use super::{
    AttachModeArg, CitationStyleArg, ConfigKeyArg, DuplicateMethodArg, HybridModeArg,
    SortDirectionArg, SortFieldArg,
};

#[derive(Subcommand)]
pub(crate) enum LibraryCommand {
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
    SavedSearch {
        #[command(subcommand)]
        command: LibrarySavedSearchCommand,
    },
}

#[derive(Subcommand)]
pub(crate) enum ItemCommand {
    Get(ItemKeyArgs),
    Related(ItemRelatedArgs),
    Open(ItemOpenArgs),
    Pdf(ItemPdfArgs),
    Fulltext(ItemPdfArgs),
    Children(ItemChildrenArgs),
    Download(ItemDownloadArgs),
    Deleted(ItemDeletedArgs),
    Versions(ItemVersionsArgs),
    Outline(ItemKeyArgs),
    Export(ItemExportArgs),
    Cite(ItemCiteArgs),
    Create(ItemCreateArgs),
    AddDoi(AddByDoiArgs),
    AddUrl(AddByUrlArgs),
    AddFile(AddFromFileArgs),
    Merge(ItemMergeArgs),
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
pub(crate) enum ItemNoteCommand {
    List(ItemKeyArgs),
    Search(NoteSearchArgs),
    Add(ItemNoteAddArgs),
    Update(ItemNoteUpdateArgs),
    Delete(ItemKeyArgs),
}

#[derive(Subcommand)]
pub(crate) enum ItemTagCommand {
    List(ItemKeyArgs),
    Add(ItemTagUpdateArgs),
    Remove(ItemTagUpdateArgs),
    Batch(ItemTagBatchArgs),
}

#[derive(Subcommand)]
pub(crate) enum ItemAnnotationCommand {
    List(AnnotationListArgs),
    Search(AnnotationSearchArgs),
    Create(AnnotationCreateArgs),
    CreateArea(AnnotationCreateAreaArgs),
}

#[derive(Subcommand)]
pub(crate) enum ItemSciteCommand {
    Report(SciteReportArgs),
    Search(SciteSearchArgs),
    Retractions(SciteRetractionsArgs),
}

#[derive(Subcommand)]
pub(crate) enum CollectionCommand {
    List,
    Get(CollectionKeyArgs),
    Subcollections(CollectionKeyArgs),
    Items(CollectionItemsArgs),
    Search(CollectionSearchArgs),
    ItemCount(CollectionKeyArgs),
    Tags(CollectionKeyArgs),
    Create(CollectionCreateArgs),
    Rename(CollectionRenameArgs),
    Delete(CollectionKeyArgs),
    AddItem(CollectionMembershipArgs),
    RemoveItem(CollectionMembershipArgs),
}

#[derive(Subcommand)]
pub(crate) enum WorkspaceCommand {
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
pub(crate) enum SyncCommand {
    UpdateStatus(UpdateStatusArgs),
}

#[derive(Subcommand)]
pub(crate) enum McpCommand {
    Serve,
}

#[derive(Subcommand)]
pub(crate) enum ConfigCommand {
    Init(ConfigInitArgs),
    Show,
    Set(ConfigSetArgs),
    Profiles {
        #[command(subcommand)]
        command: ConfigProfilesCommand,
    },
}

#[derive(Subcommand)]
pub(crate) enum ConfigProfilesCommand {
    List,
    Use(ConfigProfilesUseArgs),
}

#[derive(Subcommand)]
pub(crate) enum LibrarySavedSearchCommand {
    List,
    Create(LibrarySavedSearchCreateArgs),
    Delete(LibrarySavedSearchDeleteArgs),
}

#[derive(Args)]
pub(crate) struct ConfigInitArgs {
    #[arg(long = "target-profile")]
    pub(crate) target_profile: Option<String>,
    #[arg(long)]
    pub(crate) make_default: bool,
    #[arg(long)]
    pub(crate) data_dir: Option<String>,
    #[arg(long)]
    pub(crate) library_id: Option<String>,
    #[arg(long)]
    pub(crate) api_key: Option<String>,
    #[arg(long)]
    pub(crate) semantic_scholar_api_key: Option<String>,
    #[arg(long)]
    pub(crate) embedding_url: Option<String>,
    #[arg(long)]
    pub(crate) embedding_key: Option<String>,
    #[arg(long)]
    pub(crate) embedding_model: Option<String>,
}

#[derive(Args)]
pub(crate) struct ConfigSetArgs {
    pub(crate) key: ConfigKeyArg,
    pub(crate) value: String,
    #[arg(long = "target-profile")]
    pub(crate) target_profile: Option<String>,
}

#[derive(Args)]
pub(crate) struct ConfigProfilesUseArgs {
    pub(crate) name: String,
}

#[derive(Args)]
pub(crate) struct LibrarySearchArgs {
    pub(crate) query: String,
    #[arg(long)]
    pub(crate) collection: Option<String>,
    #[arg(long = "type")]
    pub(crate) item_type: Option<String>,
    #[arg(long)]
    pub(crate) tag: Option<String>,
    #[arg(long)]
    pub(crate) creator: Option<String>,
    #[arg(long)]
    pub(crate) year: Option<String>,
    #[arg(long)]
    pub(crate) sort: Option<SortFieldArg>,
    #[arg(long, default_value = "desc")]
    pub(crate) direction: SortDirectionArg,
    #[arg(long, default_value_t = 50)]
    pub(crate) limit: usize,
    #[arg(long, default_value_t = 0)]
    pub(crate) offset: usize,
}

#[derive(Args)]
pub(crate) struct LibraryListArgs {
    #[arg(long)]
    pub(crate) collection: Option<String>,
    #[arg(long, default_value_t = 50)]
    pub(crate) limit: usize,
    #[arg(long, default_value_t = 0)]
    pub(crate) offset: usize,
}

#[derive(Args)]
pub(crate) struct LibraryRecentArgs {
    pub(crate) since: Option<String>,
    #[arg(long, conflicts_with = "since")]
    pub(crate) count: Option<usize>,
    #[arg(long, default_value = "date-added")]
    pub(crate) sort: SortFieldArg,
    #[arg(long, default_value_t = 50)]
    pub(crate) limit: usize,
}

#[derive(Args)]
pub(crate) struct LibraryCiteKeyArgs {
    pub(crate) citekey: String,
}

#[derive(Args)]
pub(crate) struct LibraryFeedItemsArgs {
    pub(crate) library_id: i64,
    #[arg(long, default_value_t = 20)]
    pub(crate) limit: usize,
}

#[derive(Args)]
pub(crate) struct LibrarySemanticSearchArgs {
    pub(crate) query: String,
    #[arg(long, default_value = "hybrid")]
    pub(crate) mode: HybridModeArg,
    #[arg(long)]
    pub(crate) collection: Option<String>,
    #[arg(long, default_value_t = 10)]
    pub(crate) limit: usize,
}

#[derive(Args)]
pub(crate) struct LibrarySemanticIndexArgs {
    #[arg(long)]
    pub(crate) fulltext: bool,
    #[arg(long)]
    pub(crate) force_rebuild: bool,
    #[arg(long)]
    pub(crate) collection: Option<String>,
    #[arg(long, default_value_t = 0)]
    pub(crate) limit: usize,
}

#[derive(Args)]
pub(crate) struct LibraryDuplicatesArgs {
    #[arg(long, default_value = "both")]
    pub(crate) method: DuplicateMethodArg,
    #[arg(long)]
    pub(crate) collection: Option<String>,
    #[arg(long, default_value_t = 50)]
    pub(crate) limit: usize,
}

#[derive(Args)]
pub(crate) struct LibraryDuplicatesMergeArgs {
    #[arg(long)]
    pub(crate) keeper: String,
    #[arg(long = "duplicate")]
    pub(crate) duplicates: Vec<String>,
    #[arg(long)]
    pub(crate) confirm: bool,
}

#[derive(Args)]
pub(crate) struct LibrarySavedSearchCreateArgs {
    #[arg(long)]
    pub(crate) name: String,
    #[arg(long)]
    pub(crate) conditions: String,
}

#[derive(Args)]
pub(crate) struct LibrarySavedSearchDeleteArgs {
    pub(crate) keys: Vec<String>,
}

#[derive(Args)]
pub(crate) struct ItemKeyArgs {
    pub(crate) key: String,
}

#[derive(Args)]
pub(crate) struct ItemRelatedArgs {
    pub(crate) key: String,
    #[arg(long, default_value_t = 20)]
    pub(crate) limit: usize,
}

#[derive(Args)]
pub(crate) struct ItemOpenArgs {
    pub(crate) key: String,
    #[arg(long)]
    pub(crate) url: bool,
}

#[derive(Args)]
pub(crate) struct ItemPdfArgs {
    pub(crate) key: String,
    #[arg(long)]
    pub(crate) pages: Option<String>,
    #[arg(long)]
    pub(crate) annotations: bool,
}

#[derive(Args)]
pub(crate) struct ItemChildrenArgs {
    pub(crate) keys: Vec<String>,
}

#[derive(Args)]
pub(crate) struct ItemDownloadArgs {
    pub(crate) key: String,
    #[arg(long)]
    pub(crate) output: Option<PathBuf>,
}

#[derive(Args)]
pub(crate) struct ItemDeletedArgs {
    #[arg(long, default_value_t = 50)]
    pub(crate) limit: usize,
}

#[derive(Args)]
pub(crate) struct ItemVersionsArgs {
    #[arg(long)]
    pub(crate) since: Option<i64>,
}

#[derive(Args)]
pub(crate) struct ItemExportArgs {
    pub(crate) key: String,
    #[arg(long, default_value = "bibtex")]
    pub(crate) format: String,
}

#[derive(Args)]
pub(crate) struct ItemCiteArgs {
    pub(crate) key: String,
    #[arg(long, default_value = "apa")]
    pub(crate) style: CitationStyleArg,
}

#[derive(Args)]
pub(crate) struct ItemCreateArgs {
    #[arg(long)]
    pub(crate) doi: Option<String>,
    #[arg(long)]
    pub(crate) url: Option<String>,
    #[arg(long)]
    pub(crate) pdf: Option<PathBuf>,
    #[arg(long = "collection")]
    pub(crate) collections: Vec<String>,
    #[arg(long = "tag")]
    pub(crate) tags: Vec<String>,
    #[arg(long, default_value = "auto")]
    pub(crate) attach_mode: AttachModeArg,
}

#[derive(Args)]
pub(crate) struct AddByDoiArgs {
    pub(crate) doi: String,
    #[arg(long = "collection")]
    pub(crate) collections: Vec<String>,
    #[arg(long = "tag")]
    pub(crate) tags: Vec<String>,
    #[arg(long, default_value = "auto")]
    pub(crate) attach_mode: AttachModeArg,
}

#[derive(Args)]
pub(crate) struct AddByUrlArgs {
    pub(crate) url: String,
    #[arg(long = "collection")]
    pub(crate) collections: Vec<String>,
    #[arg(long = "tag")]
    pub(crate) tags: Vec<String>,
    #[arg(long, default_value = "auto")]
    pub(crate) attach_mode: AttachModeArg,
}

#[derive(Args)]
pub(crate) struct AddFromFileArgs {
    pub(crate) file: PathBuf,
    #[arg(long)]
    pub(crate) title: Option<String>,
    #[arg(long, default_value = "document")]
    pub(crate) item_type: String,
    #[arg(long)]
    pub(crate) doi: Option<String>,
    #[arg(long = "collection")]
    pub(crate) collections: Vec<String>,
    #[arg(long = "tag")]
    pub(crate) tags: Vec<String>,
}

#[derive(Args)]
pub(crate) struct ItemUpdateArgs {
    pub(crate) key: String,
    #[arg(long)]
    pub(crate) title: Option<String>,
    #[arg(long)]
    pub(crate) date: Option<String>,
    #[arg(long = "field")]
    pub(crate) fields: Vec<String>,
}

#[derive(Args)]
pub(crate) struct ItemAttachArgs {
    pub(crate) key: String,
    #[arg(long)]
    pub(crate) file: PathBuf,
}

#[derive(Args)]
pub(crate) struct ItemMergeArgs {
    pub(crate) key1: String,
    pub(crate) key2: String,
    #[arg(long)]
    pub(crate) keep: Option<String>,
    #[arg(long)]
    pub(crate) confirm: bool,
}

#[derive(Args)]
pub(crate) struct ItemNoteAddArgs {
    pub(crate) key: String,
    #[arg(long)]
    pub(crate) content: String,
}

#[derive(Args)]
pub(crate) struct NoteSearchArgs {
    pub(crate) query: String,
    #[arg(long, default_value_t = 20)]
    pub(crate) limit: usize,
}

#[derive(Args)]
pub(crate) struct ItemNoteUpdateArgs {
    pub(crate) note_key: String,
    #[arg(long)]
    pub(crate) content: String,
}

#[derive(Args)]
pub(crate) struct ItemTagUpdateArgs {
    pub(crate) key: String,
    #[arg(long = "tag")]
    pub(crate) tags: Vec<String>,
}

#[derive(Args)]
pub(crate) struct ItemTagBatchArgs {
    #[arg(long, default_value = "")]
    pub(crate) query: String,
    #[arg(long)]
    pub(crate) tag: Option<String>,
    #[arg(long = "add-tag")]
    pub(crate) add_tags: Vec<String>,
    #[arg(long = "remove-tag")]
    pub(crate) remove_tags: Vec<String>,
    #[arg(long, default_value_t = 50)]
    pub(crate) limit: usize,
}

#[derive(Args)]
pub(crate) struct CollectionItemsArgs {
    pub(crate) key: String,
}

#[derive(Args)]
pub(crate) struct CollectionSearchArgs {
    pub(crate) query: String,
    #[arg(long, default_value_t = 20)]
    pub(crate) limit: usize,
}

#[derive(Args)]
pub(crate) struct CollectionCreateArgs {
    pub(crate) name: String,
    #[arg(long)]
    pub(crate) parent: Option<String>,
}

#[derive(Args)]
pub(crate) struct CollectionRenameArgs {
    pub(crate) key: String,
    pub(crate) new_name: String,
}

#[derive(Args)]
pub(crate) struct CollectionKeyArgs {
    pub(crate) key: String,
}

#[derive(Args)]
pub(crate) struct CollectionMembershipArgs {
    pub(crate) collection_key: String,
    pub(crate) item_key: String,
}

#[derive(Args)]
pub(crate) struct WorkspaceNewArgs {
    pub(crate) name: String,
    #[arg(long, default_value = "")]
    pub(crate) description: String,
}

#[derive(Args)]
pub(crate) struct WorkspaceNameArgs {
    pub(crate) name: String,
}

#[derive(Args)]
pub(crate) struct WorkspaceShowArgs {
    pub(crate) name: String,
    #[arg(long, default_value_t = 50)]
    pub(crate) limit: usize,
}

#[derive(Args)]
pub(crate) struct WorkspaceAddArgs {
    pub(crate) name: String,
    pub(crate) keys: Vec<String>,
}

#[derive(Args)]
pub(crate) struct WorkspaceRemoveArgs {
    pub(crate) name: String,
    pub(crate) keys: Vec<String>,
}

#[derive(Args)]
pub(crate) struct WorkspaceImportArgs {
    pub(crate) name: String,
    #[arg(long)]
    pub(crate) collection: Option<String>,
    #[arg(long)]
    pub(crate) tag: Option<String>,
    #[arg(long)]
    pub(crate) search: Option<String>,
}

#[derive(Args)]
pub(crate) struct WorkspaceSearchArgs {
    pub(crate) name: String,
    pub(crate) query: String,
}

#[derive(Args)]
pub(crate) struct WorkspaceExportArgs {
    pub(crate) name: String,
    #[arg(long, default_value = "markdown")]
    pub(crate) format: String,
}

#[derive(Args)]
pub(crate) struct WorkspaceQueryArgs {
    pub(crate) name: String,
    pub(crate) question: String,
    #[arg(long, default_value = "hybrid")]
    pub(crate) mode: HybridModeArg,
    #[arg(long, default_value_t = 10)]
    pub(crate) limit: usize,
}

#[derive(Args)]
pub(crate) struct UpdateStatusArgs {
    pub(crate) key: Option<String>,
    #[arg(long)]
    pub(crate) apply: bool,
    #[arg(long)]
    pub(crate) collection: Option<String>,
    #[arg(long, default_value_t = 50)]
    pub(crate) limit: usize,
}

#[derive(Args)]
pub(crate) struct AnnotationListArgs {
    #[arg(long)]
    pub(crate) item_key: Option<String>,
    #[arg(long, default_value_t = 50)]
    pub(crate) limit: usize,
}

#[derive(Args)]
pub(crate) struct AnnotationSearchArgs {
    pub(crate) query: String,
    #[arg(long, default_value_t = 20)]
    pub(crate) limit: usize,
}

#[derive(Args)]
pub(crate) struct AnnotationCreateArgs {
    pub(crate) attachment_key: String,
    #[arg(long)]
    pub(crate) page: usize,
    #[arg(long)]
    pub(crate) text: String,
    #[arg(long)]
    pub(crate) comment: Option<String>,
    #[arg(long, default_value = "#ffd400")]
    pub(crate) color: String,
}

#[derive(Args)]
pub(crate) struct AnnotationCreateAreaArgs {
    pub(crate) attachment_key: String,
    #[arg(long)]
    pub(crate) page: usize,
    #[arg(long)]
    pub(crate) x: f32,
    #[arg(long)]
    pub(crate) y: f32,
    #[arg(long)]
    pub(crate) width: f32,
    #[arg(long)]
    pub(crate) height: f32,
    #[arg(long)]
    pub(crate) comment: Option<String>,
    #[arg(long, default_value = "#ffd400")]
    pub(crate) color: String,
}

#[derive(Args)]
pub(crate) struct SciteReportArgs {
    #[arg(long)]
    pub(crate) item_key: Option<String>,
    #[arg(long)]
    pub(crate) doi: Option<String>,
}

#[derive(Args)]
pub(crate) struct SciteSearchArgs {
    pub(crate) query: String,
    #[arg(long, default_value_t = 10)]
    pub(crate) limit: usize,
}

#[derive(Args)]
pub(crate) struct SciteRetractionsArgs {
    #[arg(long)]
    pub(crate) collection: Option<String>,
    #[arg(long)]
    pub(crate) tag: Option<String>,
    #[arg(long, default_value_t = 50)]
    pub(crate) limit: usize,
}
