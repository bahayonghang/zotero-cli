use anyhow::Result;

use crate::cli::ItemNoteCommand;
use crate::context::AppContext;
use crate::output::CommandOutput;

pub(crate) async fn handle(ctx: &AppContext, command: ItemNoteCommand) -> Result<CommandOutput> {
    match command {
        ItemNoteCommand::List(args) => {
            let notes = ctx.local_library()?.get_notes(&args.key)?;
            CommandOutput::new(ctx, notes, None, |notes| {
                for note in notes {
                    println!("{}: {}", note.key, note.content);
                }
            })
        }
        ItemNoteCommand::Search(args) => {
            let notes = ctx.local_library()?.search_notes(&args.query, args.limit)?;
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
    }
}
