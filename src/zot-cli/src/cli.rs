use clap::{Parser, Subcommand, ValueEnum};
use zot_local::{CitationStyle, DuplicateMatchMethod, HybridMode, SortDirection, SortField};

pub(crate) mod args;

// Re-export so callers can keep using `crate::cli::LibraryCommand` etc.
pub(crate) use args::*;

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
    Completions {
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
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
            ["zot", "library", "recent", "--count", "10"].as_slice(),
            ["zot", "library", "recent", "2026-04-01", "--limit", "20"].as_slice(),
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
            ["zot", "item", "merge", "KEEP001", "DUPE001"].as_slice(),
            [
                "zot",
                "item",
                "merge",
                "KEEP001",
                "DUPE001",
                "--keep",
                "DUPE001",
                "--confirm",
            ]
            .as_slice(),
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
            ["zot", "completions", "powershell"].as_slice(),
        ] {
            if let Err(err) = Cli::try_parse_from(argv) {
                panic!("cli parse failed for {:?}: {err}", argv);
            }
        }
    }
}
