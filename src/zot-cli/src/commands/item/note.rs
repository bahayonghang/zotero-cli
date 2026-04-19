use anyhow::Result;

use crate::cli::ItemNoteCommand;
use crate::context::AppContext;
use crate::format::print_enveloped;

pub(crate) async fn handle(ctx: &AppContext, command: ItemNoteCommand) -> Result<()> {
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
