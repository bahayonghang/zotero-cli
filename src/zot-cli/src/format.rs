use zot_core::{
    Attachment, CliEnvelope, Collection, EnvelopeMeta, Item, KnowledgeGraph, LibraryStats, Note,
    QueryChunk, Workspace, ZotError,
};

use crate::context::AppContext;

pub const ENVELOPE_API_VERSION: u32 = 1;

#[derive(Debug, Clone, Default)]
pub struct EnvelopeMetaSeed {
    pub count: Option<usize>,
    pub total: Option<usize>,
}

pub fn to_pretty_json<T: serde::Serialize>(value: &T) -> anyhow::Result<String> {
    Ok(serde_json::to_string_pretty(value)?)
}

pub fn print_json<T: serde::Serialize>(value: &T) -> anyhow::Result<()> {
    println!("{}", to_pretty_json(value)?);
    Ok(())
}

pub fn print_enveloped<T: serde::Serialize>(
    ctx: &AppContext,
    data: T,
    seed: Option<EnvelopeMetaSeed>,
) -> anyhow::Result<()> {
    let seed = seed.unwrap_or_default();
    let meta = EnvelopeMeta {
        count: seed.count,
        total: seed.total,
        profile: ctx.profile.clone(),
        api_version: Some(ENVELOPE_API_VERSION),
    };
    print_json(&CliEnvelope::ok_with_meta(data, meta))
}

pub fn print_error(err: &ZotError, json: bool) -> anyhow::Result<()> {
    if json {
        print_json(&CliEnvelope::<serde_json::Value>::err(err))?;
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

pub fn print_graph_summary(graph: &KnowledgeGraph) {
    let metrics = &graph.metrics;
    println!("Scope: {}", graph.scope);
    println!(
        "Nodes: {}  Edges: {}  Components: {}  Communities: {}",
        metrics.node_count,
        metrics.edge_count,
        metrics.connected_components,
        metrics.communities.len()
    );
    if !metrics.top_by_weighted_degree.is_empty() {
        println!("\nTop papers (by weighted connections):");
        for ranked in &metrics.top_by_weighted_degree {
            println!(
                "- {} [{}] (score {})",
                ranked.title, ranked.key, ranked.score
            );
        }
    }
    if !metrics.top_authors.is_empty() {
        println!("\nTop authors:");
        for author in &metrics.top_authors {
            println!("- {} ({})", author.name, author.count);
        }
    }
    if !metrics.top_tags.is_empty() {
        println!("\nTop tags:");
        for tag in &metrics.top_tags {
            println!("- {} ({})", tag.name, tag.count);
        }
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
                api_version: Some(1),
            },
        ))
        .expect("serialize envelope");

        assert!(json.contains("\"ok\": true"));
        assert!(json.contains("\"data\""));
        assert!(json.contains("\"count\": 1"));
        assert!(json.contains("\"profile\": \"default\""));
        assert!(json.contains("\"api_version\": 1"));
    }

    #[test]
    fn envelope_meta_carries_profile_from_context_and_api_version() {
        // We don't construct an `AppContext` here (it carries non-trivial
        // dependencies); instead exercise the same `EnvelopeMeta` shape that
        // `print_enveloped` would emit, ensuring the new `api_version` field
        // round-trips through JSON.
        let json = to_pretty_json(&CliEnvelope::ok_with_meta(
            serde_json::json!({"key": "WORK001"}),
            zot_core::EnvelopeMeta {
                count: Some(1),
                total: None,
                profile: Some("work".to_string()),
                api_version: Some(super::ENVELOPE_API_VERSION),
            },
        ))
        .expect("serialize envelope");
        assert!(json.contains("\"profile\": \"work\""));
        assert!(json.contains("\"api_version\": 1"));
    }

    #[test]
    fn serializes_error_envelope_byte_exact() {
        let err = zot_core::ZotError::InvalidInput {
            code: "invalid-input".to_string(),
            message: "bad key".to_string(),
            hint: Some("check the item key".to_string()),
        };
        let json = to_pretty_json(&CliEnvelope::<serde_json::Value>::err(&err))
            .expect("serialize error envelope");
        assert_eq!(
            json,
            "{\n  \"ok\": false,\n  \"error\": {\n    \"code\": \"invalid-input\",\n    \"message\": \"bad key\",\n    \"hint\": \"check the item key\"\n  }\n}"
        );
    }

    #[test]
    fn serializes_error_envelope_without_hint() {
        let err = zot_core::ZotError::Database {
            code: "db".to_string(),
            message: "locked".to_string(),
            hint: None,
        };
        let json = to_pretty_json(&CliEnvelope::<serde_json::Value>::err(&err))
            .expect("serialize error envelope");
        assert_eq!(
            json,
            "{\n  \"ok\": false,\n  \"error\": {\n    \"code\": \"db\",\n    \"message\": \"locked\"\n  }\n}"
        );
    }
}
