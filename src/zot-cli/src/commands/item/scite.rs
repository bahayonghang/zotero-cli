use std::collections::BTreeMap;

use anyhow::Result;
use zot_core::{Item, RetractionCheckResult, SciteItemReport};
use zot_local::SearchOptions;
use zot_remote::SciteClient;

use crate::cli::{ItemSciteCommand, resolved_output_limit};
use crate::context::AppContext;
use crate::output::CommandOutput;
use crate::util::{require_item, require_valid_doi};

pub(crate) async fn handle(ctx: &AppContext, command: ItemSciteCommand) -> Result<CommandOutput> {
    match command {
        ItemSciteCommand::Report(args) => {
            let report = report(ctx, args.item_key.as_deref(), args.doi.as_deref()).await?;
            CommandOutput::new(ctx, report, None, |report| {
                println!(
                    "{}",
                    serde_json::to_string_pretty(report).expect("serialize scite report")
                );
            })
        }
        ItemSciteCommand::Search(args) => {
            let reports = search(ctx, &args.query, resolved_output_limit(args.limit)).await?;
            CommandOutput::new(ctx, reports, None, |reports| {
                println!(
                    "{}",
                    serde_json::to_string_pretty(reports).expect("serialize scite reports")
                );
            })
        }
        ItemSciteCommand::Retractions(args) => {
            let reports = retractions(
                ctx,
                args.collection.as_deref(),
                args.tag.as_deref(),
                resolved_output_limit(args.limit),
            )
            .await?;
            CommandOutput::new(ctx, reports, None, |reports| {
                if reports.is_empty() {
                    println!("No editorial notices found.");
                } else {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(reports).expect("serialize retractions")
                    );
                }
            })
        }
    }
}

async fn report(
    ctx: &AppContext,
    item_key: Option<&str>,
    doi: Option<&str>,
) -> Result<SciteItemReport> {
    let resolved_doi = if let Some(doi) = doi {
        require_valid_doi(doi)?
    } else if let Some(item_key) = item_key {
        let item = require_item(&ctx.local_library()?, item_key)?;
        item.doi.ok_or_else(|| zot_core::ZotError::InvalidInput {
            code: "item-no-doi".to_string(),
            message: format!("Item '{}' has no DOI", item_key),
            hint: None,
        })?
    } else {
        return Err(zot_core::ZotError::InvalidInput {
            code: "scite-target".to_string(),
            message: "Provide --item-key or --doi".to_string(),
            hint: None,
        }
        .into());
    };
    SciteClient::new(ctx.http())
        .get_report(&resolved_doi)
        .await?
        .ok_or_else(|| {
            zot_core::ZotError::Remote {
                code: "scite-not-found".to_string(),
                message: format!("No Scite data found for DOI {}", resolved_doi),
                hint: None,
                status: None,
            }
            .into()
        })
}

async fn search(ctx: &AppContext, query: &str, limit: usize) -> Result<Vec<serde_json::Value>> {
    let library = ctx.local_library()?;
    let items = library
        .search(SearchOptions {
            query: query.to_string(),
            limit,
            ..SearchOptions::default()
        })?
        .items;
    let reports = SciteClient::new(ctx.http())
        .get_reports_batch(&collect_dois(&items))
        .await?;
    Ok(pair_scite_reports(items, &reports))
}

async fn retractions(
    ctx: &AppContext,
    collection: Option<&str>,
    tag: Option<&str>,
    limit: usize,
) -> Result<Vec<RetractionCheckResult>> {
    let library = ctx.local_library()?;
    let mut items = if let Some(collection) = collection {
        library.get_collection_items(collection)?
    } else {
        library.list_items(None, limit, 0)?
    };
    if let Some(tag) = tag {
        items.retain(|item| item.tags.iter().any(|value| value == tag));
    }
    items.truncate(limit);
    let reports = SciteClient::new(ctx.http())
        .get_reports_batch(&collect_dois(&items))
        .await?;
    Ok(collect_retraction_results(items, &reports))
}

/// Collect the DOIs present across a set of items, dropping items without one.
fn collect_dois(items: &[Item]) -> Vec<String> {
    items.iter().filter_map(|item| item.doi.clone()).collect()
}

/// Pair each item with its Scite report by DOI, emitting `null` for items that
/// have no matching report. Pure: the batch fetch happens in the shell.
fn pair_scite_reports(
    items: Vec<Item>,
    reports: &BTreeMap<String, SciteItemReport>,
) -> Vec<serde_json::Value> {
    items
        .into_iter()
        .map(|item| {
            serde_json::json!({
                "item": item,
                "scite": item.doi.as_deref().and_then(|doi| reports.get(doi)),
            })
        })
        .collect()
}

/// Keep only the items whose Scite report carries at least one editorial notice,
/// pairing each with its notices. Pure: the batch fetch happens in the shell.
fn collect_retraction_results(
    items: Vec<Item>,
    reports: &BTreeMap<String, SciteItemReport>,
) -> Vec<RetractionCheckResult> {
    items
        .into_iter()
        .filter_map(|item| {
            item.doi
                .as_deref()
                .and_then(|doi| reports.get(doi))
                .filter(|report| !report.notices.is_empty())
                .map(|report| RetractionCheckResult {
                    item,
                    notices: report.notices.clone(),
                })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{collect_dois, collect_retraction_results, pair_scite_reports};
    use std::collections::BTreeMap;
    use zot_core::{EditorialNotice, Item, SciteItemReport};

    fn item(key: &str, doi: Option<&str>) -> Item {
        Item {
            key: key.to_string(),
            item_type: "journalArticle".to_string(),
            title: format!("Title {key}"),
            creators: Vec::new(),
            abstract_note: None,
            date: None,
            url: None,
            doi: doi.map(ToString::to_string),
            tags: Vec::new(),
            collections: Vec::new(),
            date_added: None,
            date_modified: None,
            extra: Default::default(),
        }
    }

    fn report(doi: &str, notices: Vec<EditorialNotice>) -> SciteItemReport {
        SciteItemReport {
            doi: doi.to_string(),
            title: format!("Report {doi}"),
            tally: None,
            notices,
        }
    }

    fn notice() -> EditorialNotice {
        EditorialNotice {
            notice_type: "retraction".to_string(),
            source: None,
        }
    }

    #[test]
    fn collect_dois_drops_items_without_doi() {
        let items = vec![
            item("A", Some("10.1000/a")),
            item("B", None),
            item("C", Some("10.1000/c")),
        ];
        assert_eq!(
            collect_dois(&items),
            vec!["10.1000/a".to_string(), "10.1000/c".to_string()]
        );
    }

    #[test]
    fn pair_scite_reports_attaches_match_or_null() {
        let items = vec![item("A", Some("10.1000/a")), item("B", None)];
        let mut reports = BTreeMap::new();
        reports.insert("10.1000/a".to_string(), report("10.1000/a", Vec::new()));

        let paired = pair_scite_reports(items, &reports);

        assert_eq!(paired.len(), 2);
        assert_eq!(paired[0]["item"]["key"], "A");
        assert_eq!(paired[0]["scite"]["doi"], "10.1000/a");
        assert!(paired[1]["scite"].is_null());
    }

    #[test]
    fn collect_retraction_results_keeps_only_items_with_notices() {
        let items = vec![
            item("A", Some("10.1000/a")),
            item("B", Some("10.1000/b")),
            item("C", Some("10.1000/c")),
            item("D", None),
        ];
        let mut reports = BTreeMap::new();
        reports.insert("10.1000/a".to_string(), report("10.1000/a", vec![notice()]));
        reports.insert("10.1000/b".to_string(), report("10.1000/b", Vec::new()));

        let results = collect_retraction_results(items, &reports);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].item.key, "A");
        assert_eq!(results[0].notices.len(), 1);
    }

    #[test]
    fn collect_retraction_results_with_empty_reports_yields_nothing() {
        let items = vec![item("A", Some("10.1000/a"))];
        assert!(collect_retraction_results(items, &BTreeMap::new()).is_empty());
    }
}
