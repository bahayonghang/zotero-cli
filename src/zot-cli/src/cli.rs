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
    pub(crate) verbose: bool,
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
    Graph(GraphArgs),
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub(crate) enum ItemImportFormatArg {
    Bibtex,
    Ris,
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

impl Cli {
    pub(crate) fn validate_output_protocol(&self) -> Result<(), zot_core::ZotError> {
        if !self.json {
            return Ok(());
        }

        let unsupported = match &self.command {
            Commands::Graph(args) if matches!(args.command, Some(GraphCommand::Serve(_))) => {
                Some("`graph serve` uses a long-running human output protocol")
            }
            Commands::Completions { .. } => {
                Some("`completions` writes a raw shell completion script")
            }
            _ => None,
        };

        match unsupported {
            Some(message) => Err(zot_core::ZotError::Unsupported {
                code: "json-protocol-unsupported".to_string(),
                message: message.to_string(),
                hint: Some("Omit `--json` for this command".to_string()),
            }),
            None => Ok(()),
        }
    }

    pub(crate) fn resolve_effective_options(
        &mut self,
        configured_limit: usize,
    ) -> Result<(), zot_core::ZotError> {
        match &mut self.command {
            Commands::Library { command } => match command {
                LibraryCommand::Search(args) => resolve_limit(&mut args.limit, configured_limit),
                LibraryCommand::List(args) => resolve_limit(&mut args.limit, configured_limit),
                LibraryCommand::Recent(args) => resolve_limit(&mut args.limit, configured_limit),
                LibraryCommand::FeedItems(args) => resolve_limit(&mut args.limit, configured_limit),
                LibraryCommand::SemanticSearch(args) => {
                    resolve_limit(&mut args.limit, configured_limit)
                }
                LibraryCommand::Duplicates(args) => {
                    resolve_limit(&mut args.limit, configured_limit)
                }
                _ => Ok(()),
            },
            Commands::Item { command } => match command {
                ItemCommand::Related(args) => resolve_limit(&mut args.limit, configured_limit),
                ItemCommand::Deleted(args) => resolve_limit(&mut args.limit, configured_limit),
                ItemCommand::Note {
                    command: ItemNoteCommand::Search(args),
                } => resolve_limit(&mut args.limit, configured_limit),
                ItemCommand::Annotation {
                    command: ItemAnnotationCommand::List(args),
                } => resolve_limit(&mut args.limit, configured_limit),
                ItemCommand::Annotation {
                    command: ItemAnnotationCommand::Search(args),
                } => resolve_limit(&mut args.limit, configured_limit),
                ItemCommand::Scite {
                    command: ItemSciteCommand::Search(args),
                } => resolve_limit(&mut args.limit, configured_limit),
                ItemCommand::Scite {
                    command: ItemSciteCommand::Retractions(args),
                } => resolve_limit(&mut args.limit, configured_limit),
                _ => Ok(()),
            },
            Commands::Collection {
                command: CollectionCommand::Search(args),
            } => resolve_limit(&mut args.limit, configured_limit),
            Commands::Workspace {
                command: WorkspaceCommand::Show(args),
            } => resolve_limit(&mut args.limit, configured_limit),
            Commands::Workspace {
                command: WorkspaceCommand::Query(args),
            } => resolve_limit(&mut args.limit, configured_limit),
            _ => Ok(()),
        }
    }
}

pub(crate) fn resolved_output_limit(limit: Option<usize>) -> usize {
    limit.unwrap_or(50)
}

fn resolve_limit(
    limit: &mut Option<usize>,
    configured_limit: usize,
) -> Result<(), zot_core::ZotError> {
    if limit.is_some_and(|value| value == 0) || configured_limit == 0 {
        return Err(zot_core::ZotError::InvalidInput {
            code: "config-value".to_string(),
            message: "Output limit must be greater than zero".to_string(),
            hint: Some("Pass --limit with a positive integer or update output-limit".to_string()),
        });
    }
    if limit.is_none() {
        *limit = Some(configured_limit);
    }
    Ok(())
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

    use super::{Cli, Commands, ItemCommand, ItemTagCommand, LibraryCommand};

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
            ["zot", "library", "dedupe"].as_slice(),
            [
                "zot",
                "library",
                "dedupe",
                "--method",
                "doi",
                "--collection",
                "COLTR02",
                "--limit",
                "5",
                "--confirm",
                "--include-low-confidence",
            ]
            .as_slice(),
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
            [
                "zot",
                "item",
                "tag",
                "batch",
                "--query",
                "transformer",
                "--add-tag",
                "reviewed",
                "--limit",
                "75",
                "--max-affected",
                "75",
                "--confirm",
            ]
            .as_slice(),
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
            ["zot", "item", "import", "--file", "refs.bib"].as_slice(),
            [
                "zot",
                "item",
                "import",
                "--text",
                "TY  - JOUR\nER  - \n",
                "--format",
                "ris",
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
            ["zot", "graph"].as_slice(),
            ["zot", "graph", "--collection", "COLTR02"].as_slice(),
            ["zot", "--json", "graph", "--collection", "COLTR02"].as_slice(),
            ["zot", "--verbose", "doctor"].as_slice(),
            ["zot", "graph", "serve"].as_slice(),
            ["zot", "graph", "serve", "--no-open", "--port", "7901"].as_slice(),
            [
                "zot",
                "graph",
                "serve",
                "--collection",
                "COLTR02",
                "--no-open",
            ]
            .as_slice(),
            ["zot", "completions", "powershell"].as_slice(),
        ] {
            if let Err(err) = Cli::try_parse_from(argv) {
                panic!("cli parse failed for {:?}: {err}", argv);
            }
        }
    }

    #[test]
    fn resolves_configured_limit_only_for_read_output_commands() {
        let mut search =
            Cli::try_parse_from(["zot", "library", "search", "attention"]).expect("parse search");
        search
            .resolve_effective_options(17)
            .expect("resolve search limit");
        match search.command {
            Commands::Library {
                command: LibraryCommand::Search(args),
            } => assert_eq!(args.limit, Some(17)),
            _ => panic!("unexpected search command"),
        }

        let mut explicit =
            Cli::try_parse_from(["zot", "library", "search", "attention", "--limit", "3"])
                .expect("parse explicit search");
        explicit
            .resolve_effective_options(17)
            .expect("resolve explicit search limit");
        match explicit.command {
            Commands::Library {
                command: LibraryCommand::Search(args),
            } => assert_eq!(args.limit, Some(3)),
            _ => panic!("unexpected explicit search command"),
        }

        let mut batch =
            Cli::try_parse_from(["zot", "item", "tag", "batch", "--add-tag", "reviewed"])
                .expect("parse tag batch");
        batch
            .resolve_effective_options(17)
            .expect("resolve tag batch options");
        match batch.command {
            Commands::Item {
                command:
                    ItemCommand::Tag {
                        command: ItemTagCommand::Batch(args),
                    },
            } => assert_eq!(args.limit, 50),
            _ => panic!("unexpected tag batch command"),
        }
    }

    #[test]
    fn rejects_zero_explicit_output_limit() {
        let mut cli =
            Cli::try_parse_from(["zot", "collection", "search", "attention", "--limit", "0"])
                .expect("parse zero limit");

        let error = cli
            .resolve_effective_options(17)
            .expect_err("zero limit must fail");

        assert_eq!(error.payload().code, "config-value");
    }
}
