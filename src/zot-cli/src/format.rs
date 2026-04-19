use zot_core::envelope::EnvelopeError;
use zot_core::{
    Attachment, CliEnvelope, Collection, EnvelopeMeta, Item, LibraryStats, Note, QueryChunk,
    Workspace, ZotError,
};

pub fn to_pretty_json<T: serde::Serialize>(value: &T) -> anyhow::Result<String> {
    Ok(serde_json::to_string_pretty(value)?)
}

pub fn print_json<T: serde::Serialize>(value: &T) -> anyhow::Result<()> {
    println!("{}", to_pretty_json(value)?);
    Ok(())
}

pub fn print_enveloped<T: serde::Serialize>(
    data: T,
    meta: Option<EnvelopeMeta>,
) -> anyhow::Result<()> {
    let envelope = if let Some(meta) = meta {
        CliEnvelope::ok_with_meta(data, meta)
    } else {
        CliEnvelope::ok(data)
    };
    print_json(&envelope)
}

pub fn print_error(err: &ZotError, json: bool) -> anyhow::Result<()> {
    if json {
        let payload = CliEnvelope::<serde_json::Value>::Err {
            ok: false,
            error: EnvelopeError {
                code: err.payload().code,
                message: err.payload().message,
                hint: err.payload().hint,
            },
        };
        print_json(&payload)?;
    } else {
        eprintln!("Error: {}", err);
        if let Some(hint) = err.payload().hint {
            eprintln!("Hint: {hint}");
        }
    }
    Ok(())
}

pub fn print_items(items: &[Item]) {
    if items.is_empty() {
        println!("No items found.");
        return;
    }
    for item in items {
        let authors = item
            .creators
            .iter()
            .map(|creator| creator.full_name())
            .collect::<Vec<_>>()
            .join(", ");
        println!("{} [{}] {}", item.key, item.item_type, item.title);
        if !authors.is_empty() {
            println!("  Authors: {authors}");
        }
        if let Some(date) = item.date.as_deref() {
            println!("  Date: {date}");
        }
        if !item.tags.is_empty() {
            println!("  Tags: {}", item.tags.join(", "));
        }
    }
}

pub fn print_item(item: &Item, notes: &[Note], attachments: &[Attachment]) {
    println!("{} [{}]", item.title, item.key);
    println!("Type: {}", item.item_type);
    if !item.creators.is_empty() {
        println!(
            "Authors: {}",
            item.creators
                .iter()
                .map(|creator| creator.full_name())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    if let Some(abstract_note) = item.abstract_note.as_deref() {
        println!("\nAbstract:\n{abstract_note}");
    }
    if !notes.is_empty() {
        println!("\nNotes:");
        for note in notes {
            println!("- {}: {}", note.key, note.content);
        }
    }
    if !attachments.is_empty() {
        println!("\nAttachments:");
        for attachment in attachments {
            println!("- {} ({})", attachment.filename, attachment.key);
        }
    }
}

pub fn print_collections(collections: &[Collection], indent: usize) {
    for collection in collections {
        println!(
            "{}{} ({})",
            " ".repeat(indent),
            collection.name,
            collection.key
        );
        print_collections(&collection.children, indent + 2);
    }
}

pub fn print_stats(stats: &LibraryStats) {
    println!("Total items: {}", stats.total_items);
    println!("PDF attachments: {}", stats.pdf_attachments);
    println!("Notes: {}", stats.notes);
    println!("\nBy type:");
    for (kind, count) in &stats.by_type {
        println!("- {kind}: {count}");
    }
}

pub fn print_workspace(workspace: &Workspace) {
    println!("Workspace: {}", workspace.name);
    if !workspace.description.is_empty() {
        println!("Description: {}", workspace.description);
    }
    println!("Items: {}", workspace.items.len());
    for item in &workspace.items {
        println!("- {} ({})", item.title, item.key);
    }
}

pub fn print_query_chunks(chunks: &[QueryChunk]) {
    for chunk in chunks {
        println!(
            "{} [{}] score={:.3}",
            chunk.item_key, chunk.source, chunk.score
        );
        println!("{}", chunk.content);
        println!();
    }
}

#[cfg(test)]
mod tests {
    use zot_core::CliEnvelope;

    use super::to_pretty_json;

    #[test]
    fn serializes_success_envelope_with_meta() {
        let json = to_pretty_json(&CliEnvelope::ok_with_meta(
            serde_json::json!({ "hello": "world" }),
            zot_core::EnvelopeMeta {
                count: Some(1),
                total: Some(1),
                profile: Some("default".to_string()),
            },
        ))
        .expect("serialize envelope");

        assert!(json.contains("\"ok\": true"));
        assert!(json.contains("\"data\""));
        assert!(json.contains("\"count\": 1"));
        assert!(json.contains("\"profile\": \"default\""));
    }
}
