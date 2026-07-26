use std::collections::BTreeMap;

use anyhow::Result;
use zot_remote::{PublicationStatus, SemanticScholarClient, extract_preprint_info};

use crate::cli::SyncCommand;
use crate::context::AppContext;
use crate::output::CommandOutput;
use crate::util::update_status_to_json;

pub(crate) async fn handle(ctx: &AppContext, command: SyncCommand) -> Result<CommandOutput> {
    match command {
        SyncCommand::UpdateStatus(args) => {
            let library = ctx.local_library()?;
            let items = if let Some(key) = args.key.as_deref() {
                library.get_item(key)?.into_iter().collect::<Vec<_>>()
            } else {
                library.get_arxiv_preprints(args.collection.as_deref(), args.limit)?
            };
            let client = SemanticScholarClient::new(ctx.http(), ctx.config.semantic_scholar_key())?;
            let mut matches = Vec::new();
            for item in items {
                if let Some(info) = extract_preprint_info(
                    item.url.as_deref(),
                    item.doi.as_deref(),
                    item.extra.get("extra").map(String::as_str),
                ) {
                    if let Some(status) = client.check_publication(&info).await? {
                        matches.push((item.key.clone(), status));
                    }
                }
            }
            if args.apply {
                let remote = ctx.remote()?;
                for (key, status) in &matches {
                    let fields = publication_status_fields(status);
                    if !fields.is_empty() {
                        remote.update_item_fields(key, &fields).await?;
                    }
                }
            }
            let payload = matches
                .into_iter()
                .map(|(key, status)| update_status_to_json(key, status))
                .collect::<Vec<_>>();
            CommandOutput::new(ctx, payload, None, |entries| {
                for entry in entries {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(entry).expect("serialize status entry")
                    );
                }
            })
        }
    }
}

/// Map a resolved publication status into the Zotero fields to apply. Pure: the
/// `--apply` gate stays in the shell. Returns an empty map when the preprint is
/// not yet published or carries no usable fields, so the caller skips the write.
fn publication_status_fields(status: &PublicationStatus) -> BTreeMap<String, String> {
    let mut fields = BTreeMap::new();
    if !status.is_published {
        return fields;
    }
    if let Some(doi) = status.doi.as_deref() {
        fields.insert("DOI".to_string(), doi.to_string());
    }
    if let Some(venue) = status.venue.as_deref().or(status.journal_name.as_deref()) {
        fields.insert("publicationTitle".to_string(), venue.to_string());
    }
    if let Some(date) = status.publication_date.as_deref() {
        fields.insert("date".to_string(), date.to_string());
    }
    fields
}

#[cfg(test)]
mod tests {
    use super::publication_status_fields;
    use zot_remote::PublicationStatus;

    fn status(is_published: bool) -> PublicationStatus {
        PublicationStatus {
            preprint_id: "2301.00774".to_string(),
            source: "arxiv".to_string(),
            title: "Sample".to_string(),
            is_published,
            venue: None,
            journal_name: None,
            doi: None,
            publication_date: None,
        }
    }

    #[test]
    fn not_published_yields_empty_fields() {
        let mut status = status(false);
        status.doi = Some("10.1000/x".to_string());
        status.venue = Some("Venue".to_string());
        assert!(publication_status_fields(&status).is_empty());
    }

    #[test]
    fn published_with_all_fields_maps_each() {
        let mut status = status(true);
        status.doi = Some("10.1000/x".to_string());
        status.venue = Some("Venue".to_string());
        status.publication_date = Some("2024-01-01".to_string());
        let fields = publication_status_fields(&status);
        assert_eq!(fields.get("DOI").map(String::as_str), Some("10.1000/x"));
        assert_eq!(
            fields.get("publicationTitle").map(String::as_str),
            Some("Venue")
        );
        assert_eq!(fields.get("date").map(String::as_str), Some("2024-01-01"));
    }

    #[test]
    fn published_falls_back_to_journal_name_for_publication_title() {
        let mut status = status(true);
        status.journal_name = Some("Journal of Testing".to_string());
        let fields = publication_status_fields(&status);
        assert_eq!(
            fields.get("publicationTitle").map(String::as_str),
            Some("Journal of Testing")
        );
        assert!(!fields.contains_key("DOI"));
        assert!(!fields.contains_key("date"));
    }

    #[test]
    fn published_without_usable_fields_is_empty() {
        assert!(publication_status_fields(&status(true)).is_empty());
    }
}
