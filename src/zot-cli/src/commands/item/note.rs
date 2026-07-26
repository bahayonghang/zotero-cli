use anyhow::Result;
use zot_local::NoteReader;

use crate::cli::{ItemNoteCommand, resolved_output_limit};
use crate::context::AppContext;
use crate::output::CommandOutput;

pub(crate) async fn handle(ctx: &AppContext, command: ItemNoteCommand) -> Result<CommandOutput> {
    match command {
        ItemNoteCommand::Add(args) => {
            let key = ctx.remote()?.add_note(&args.key, &args.content).await?;
            let payload = serde_json::json!({ "note_key": key });
            CommandOutput::new(ctx, payload, None, move |_| println!("Note added: {key}"))
        }
        ItemNoteCommand::Update(args) => {
            ctx.remote()?
                .update_note(&args.note_key, &args.content)
                .await?;
            let note_key = args.note_key;
            let payload = serde_json::json!({ "updated": note_key });
            CommandOutput::new(ctx, payload, None, move |_| {
                println!("Note updated: {note_key}")
            })
        }
        ItemNoteCommand::Delete(args) => {
            ctx.remote()?.delete_note(&args.key).await?;
            let key = args.key;
            let payload = serde_json::json!({ "trashed": key });
            CommandOutput::new(ctx, payload, None, move |_| {
                println!("Note moved to trash: {key}")
            })
        }
        read => handle_read(ctx, &ctx.local_library()?, read),
    }
}

/// Read-side arms, generic over [`NoteReader`] so tests can drive the full
/// output path with a fake library. Write arms stay in [`handle`] because
/// they go through `ctx.remote()`.
fn handle_read<L: NoteReader>(
    ctx: &AppContext,
    library: &L,
    command: ItemNoteCommand,
) -> Result<CommandOutput> {
    match command {
        ItemNoteCommand::List(args) => {
            let notes = library.get_notes(&args.key)?;
            CommandOutput::new(ctx, notes, None, |notes| {
                for note in notes {
                    println!("{}: {}", note.key, note.content);
                }
            })
        }
        ItemNoteCommand::Search(args) => {
            let notes = library.search_notes(&args.query, resolved_output_limit(args.limit))?;
            CommandOutput::new(ctx, notes, None, |notes| {
                for note in notes {
                    println!(
                        "{} [{}] {}",
                        note.key,
                        note.parent_title.as_deref().unwrap_or("Unknown"),
                        note.content
                    );
                }
            })
        }
        _ => unreachable!("write commands are dispatched in `handle`"),
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::sync::Arc;

    use zot_core::{AppConfig, LibraryScope, Note, NoteSearchResult, ZotError, ZotResult};
    use zot_local::PdfiumBackend;
    use zot_remote::HttpRuntime;

    use super::*;
    use crate::cli::{ItemKeyArgs, NoteSearchArgs};

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

    struct FakeNotes {
        notes: Vec<Note>,
        results: Vec<NoteSearchResult>,
        seen_limit: Cell<Option<usize>>,
        fail: bool,
    }

    impl FakeNotes {
        fn sample() -> Self {
            Self {
                notes: vec![Note {
                    key: "NOTE001".to_string(),
                    parent_key: "ATTN001".to_string(),
                    content: "Reading notes on attention".to_string(),
                    tags: vec![],
                }],
                results: vec![NoteSearchResult {
                    key: "NOTE001".to_string(),
                    parent_key: Some("ATTN001".to_string()),
                    parent_title: Some("Attention Is All You Need".to_string()),
                    title: None,
                    content: "Reading notes on attention".to_string(),
                    tags: vec![],
                }],
                seen_limit: Cell::new(None),
                fail: false,
            }
        }

        fn failing() -> Self {
            Self {
                fail: true,
                ..Self::sample()
            }
        }
    }

    impl NoteReader for FakeNotes {
        fn get_notes(&self, _key: &str) -> ZotResult<Vec<Note>> {
            if self.fail {
                return Err(ZotError::InvalidInput {
                    code: "fake-library-error".to_string(),
                    message: "injected failure".to_string(),
                    hint: None,
                });
            }
            Ok(self.notes.clone())
        }
        fn search_notes(&self, _query: &str, limit: usize) -> ZotResult<Vec<NoteSearchResult>> {
            self.seen_limit.set(Some(limit));
            Ok(self.results.clone())
        }
    }

    #[test]
    fn list_envelope_carries_note_key_and_content() {
        let out = handle_read(
            &ctx(true),
            &FakeNotes::sample(),
            ItemNoteCommand::List(ItemKeyArgs {
                key: "ATTN001".to_string(),
            }),
        )
        .expect("list must succeed");
        let json = out.as_json().expect("json payload");
        assert!(json.contains("\"key\": \"NOTE001\""));
        assert!(json.contains("Reading notes on attention"));
    }

    #[test]
    fn search_passes_limit_through_to_the_library() {
        let fake = FakeNotes::sample();
        let out = handle_read(
            &ctx(true),
            &fake,
            ItemNoteCommand::Search(NoteSearchArgs {
                query: "attention".to_string(),
                limit: Some(7),
            }),
        )
        .expect("search must succeed");
        assert_eq!(fake.seen_limit.get(), Some(7));
        let json = out.as_json().expect("json payload");
        assert!(json.contains("\"parent_title\": \"Attention Is All You Need\""));
    }

    #[test]
    fn library_errors_pass_through_unchanged() {
        let err = handle_read(
            &ctx(true),
            &FakeNotes::failing(),
            ItemNoteCommand::List(ItemKeyArgs {
                key: "ATTN001".to_string(),
            }),
        )
        .expect_err("injected failure must propagate");
        let err = err.downcast_ref::<ZotError>().expect("zot error");
        match err {
            ZotError::InvalidInput { code, .. } => assert_eq!(code, "fake-library-error"),
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
