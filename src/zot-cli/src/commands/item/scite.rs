use anyhow::Result;
use zot_core::{RetractionCheckResult, SciteItemReport};
use zot_local::SearchOptions;
use zot_remote::{SciteClient, normalize_doi};

use crate::cli::ItemSciteCommand;
use crate::context::AppContext;
use crate::format::print_enveloped;

pub(crate) async fn handle(ctx: &AppContext, command: ItemSciteCommand) -> Result<()> {
    match command {
        ItemSciteCommand::Report(args) => {
            let report = report(ctx, args.item_key.as_deref(), args.doi.as_deref()).await?;
            if ctx.json {
                print_enveloped(&report, None)?;
            } else {
                println!("{}", serde_json::to_string_pretty(&report)?);
            }
        }
        ItemSciteCommand::Search(args) => {
            let reports = search(ctx, &args.query, args.limit).await?;
            if ctx.json {
                print_enveloped(&reports, None)?;
            } else {
                println!("{}", serde_json::to_string_pretty(&reports)?);
            }
        }
        ItemSciteCommand::Retractions(args) => {
            let reports = retractions(
                ctx,
                args.collection.as_deref(),
                args.tag.as_deref(),
                args.limit,
            )
            .await?;
            if ctx.json {
                print_enveloped(&reports, None)?;
            } else if reports.is_empty() {
                println!("No editorial notices found.");
            } else {
                println!("{}", serde_json::to_string_pretty(&reports)?);
            }
        }
    }
    Ok(())
}

async fn report(
    ctx: &AppContext,
    item_key: Option<&str>,
    doi: Option<&str>,
) -> Result<SciteItemReport> {
    let resolved_doi = if let Some(doi) = doi {
        normalize_doi(doi).ok_or_else(|| zot_core::ZotError::InvalidInput {
            code: "invalid-doi".to_string(),
            message: format!("'{}' does not appear to be a valid DOI", doi),
            hint: None,
        })?
    } else if let Some(item_key) = item_key {
        let item = ctx.local_library()?.get_item(item_key)?.ok_or_else(|| {
            zot_core::ZotError::InvalidInput {
                code: "item-not-found".to_string(),
                message: format!("Item '{}' not found", item_key),
                hint: None,
            }
        })?;
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
    let dois = items
        .iter()
        .filter_map(|item| item.doi.clone())
        .collect::<Vec<_>>();
    let reports = SciteClient::new(ctx.http()).get_reports_batch(&dois).await?;
    Ok(items
        .into_iter()
        .map(|item| {
            serde_json::json!({
                "item": item,
                "scite": item.doi.as_deref().and_then(|doi| reports.get(doi)),
            })
        })
        .collect())
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
    let dois = items
        .iter()
        .filter_map(|item| item.doi.clone())
        .collect::<Vec<_>>();
    let reports = SciteClient::new(ctx.http()).get_reports_batch(&dois).await?;
    Ok(items
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
        .collect())
}
