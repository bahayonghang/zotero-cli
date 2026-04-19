use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};
use zot_local::{CitationStyle, DuplicateMatchMethod, HybridMode, SortDirection, SortField};

#[derive(Parser)]
#[command(name = "zot", version, about = "Rust Zotero CLI")]
pub(crate) struct Cli {
    #[arg(long, global = true)]
    pub(crate) json: bool,
    #[arg(long, global = true)]
    pub(crate) profile: Option<String>,
    #[arg(long, global = true, default_value = "user")]
    pub(crate) library: String,
    #[command(subcommand)]
    pub(crate) command: Commands,
}

#[derive(Subcommand)]
pub(crate) enum Commands {
    Doctor,
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
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
    pub(crate) since: String,
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

#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum SortFieldArg {
    DateAdded,
    DateModified,
    Title,
    Creator,
}

#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum SortDirectionArg {
    Asc,
    Desc,
}

#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum CitationStyleArg {
    Apa,
    Nature,
    Vancouver,
}

#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum HybridModeArg {
    Bm25,
    Semantic,
    Hybrid,
}

#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum AttachModeArg {
    Auto,
    LinkedUrl,
    None,
}

#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum DuplicateMethodArg {
    Title,
    Doi,
    Both,
}

#[derive(Clone, Debug, Copy, ValueEnum)]
pub(crate) enum ConfigKeyArg {
    DataDir,
    LibraryId,
    ApiKey,
    SemanticScholarApiKey,
    EmbeddingUrl,
    EmbeddingKey,
    EmbeddingModel,
    OutputFormat,
    OutputLimit,
    ExportStyle,
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
            ["zot", "config", "show"].as_slice(),
            [
                "zot",
                "config",
                "init",
                "--target-profile",
                "work",
                "--library-id",
                "42",
            ]
            .as_slice(),
            [
                "zot",
                "config",
                "set",
                "library-id",
                "42",
                "--target-profile",
                "work",
            ]
            .as_slice(),
            ["zot", "config", "profiles", "use", "work"].as_slice(),
            ["zot", "library", "semantic-status"].as_slice(),
            ["zot", "library", "citekey", "Smith2024"].as_slice(),
            ["zot", "library", "duplicates", "--method", "both"].as_slice(),
            ["zot", "library", "saved-search", "list"].as_slice(),
            [
                "zot",
                "library",
                "saved-search",
                "create",
                "--name",
                "Recent",
                "--conditions",
                "[]",
            ]
            .as_slice(),
            ["zot", "item", "children", "ATTN001"].as_slice(),
            ["zot", "item", "download", "ATCH005"].as_slice(),
            ["zot", "item", "versions", "--since", "12"].as_slice(),
            ["zot", "item", "deleted", "--limit", "10"].as_slice(),
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
            ["zot", "collection", "get", "COLTR02"].as_slice(),
            ["zot", "collection", "subcollections", "COLTR02"].as_slice(),
            ["zot", "collection", "item-count", "COLTR02"].as_slice(),
            ["zot", "collection", "tags", "COLTR02"].as_slice(),
        ] {
            if let Err(err) = Cli::try_parse_from(argv) {
                panic!("cli parse failed for {:?}: {err}", argv);
            }
        }
    }
}
