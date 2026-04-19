use std::collections::BTreeMap;

use anyhow::Result;
use zot_remote::{SemanticScholarClient, extract_preprint_info};

use crate::cli::SyncCommand;
use crate::context::AppContext;
use crate::format::print_enveloped;
use crate::util::update_status_to_json;

pub(crate) async fn handle(ctx: &AppContext, command: SyncCommand) -> Result<()> {
    match command {
        SyncCommand::UpdateStatus(args) => {
            let library = ctx.local_library()?;
            let items = if let Some(key) = args.key.as_deref() {
                library.get_item(key)?.into_iter().collect::<Vec<_>>()
            } else {
                library.get_arxiv_preprints(args.collection.as_deref(), args.limit)?
            };
            let client = SemanticScholarClient::new(ctx.config.semantic_scholar_key())?;
            let mut matches = Vec::new();
            for item in items {
                if let Some(info) = extract_preprint_info(
                    item.url.as_deref(),
                    item.doi.as_deref(),
                    item.extra.get("extra").map(String::as_str),
                ) && let Some(status) = client.check_publication(&info).await?
                {
                    matches.push((item.key.clone(), status));
                }
            }
            if args.apply {
                let remote = ctx.remote()?;
                for (key, status) in &matches {
                    if status.is_published {
                        let mut fields = BTreeMap::new();
                        if let Some(doi) = status.doi.as_deref() {
                            fields.insert("DOI".to_string(), doi.to_string());
                        }
                        if let Some(venue) =
                            status.venue.as_deref().or(status.journal_name.as_deref())
                        {
                            fields.insert("publicationTitle".to_string(), venue.to_string());
                        }
                        if let Some(date) = status.publication_date.as_deref() {
                            fields.insert("date".to_string(), date.to_string());
                        }
                        if !fields.is_empty() {
                            remote.update_item_fields(key, &fields).await?;
                        }
                    }
                }
            }
            let payload = matches
                .into_iter()
                .map(|(key, status)| update_status_to_json(key, status))
                .collect::<Vec<_>>();
            if ctx.json {
                print_enveloped(payload, None)?;
            } else {
                for entry in payload {
                    println!("{}", serde_json::to_string_pretty(&entry)?);
                }
            }
        }
    }
    Ok(())
}
