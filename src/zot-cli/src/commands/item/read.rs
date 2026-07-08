use std::path::PathBuf;

use anyhow::Result;
use zot_local::{PdfBackend, PdfCache};

use crate::cli::{
    ItemChildrenArgs, ItemCiteArgs, ItemDeletedArgs, ItemDownloadArgs, ItemExportArgs, ItemKeyArgs,
    ItemOpenArgs, ItemPdfArgs, ItemRelatedArgs, ItemVersionsArgs,
};
use crate::context::AppContext;
use crate::format::{print_item, print_items};
use crate::output::CommandOutput;
use crate::util::{
    item_not_found, open_target, parse_page_range, print_outline_entries, require_item,
    require_item_pdf, run_pdf,
};

pub(crate) async fn handle_get(ctx: &AppContext, args: ItemKeyArgs) -> Result<CommandOutput> {
    let library = ctx.local_library()?;
    let item = require_item(&library, &args.key)?;
    let notes = library.get_notes(&args.key)?;
    let attachments = library.get_attachments(&args.key)?;
    let payload = serde_json::json!({
        "item": item,
        "notes": notes,
        "attachments": attachments,
    });
    CommandOutput::new(ctx, payload, None, move |_| {
        print_item(&item, &notes, &attachments)
    })
}

pub(crate) async fn handle_related(
    ctx: &AppContext,
    args: ItemRelatedArgs,
) -> Result<CommandOutput> {
    let library = ctx.local_library()?;
    let items = library.get_related_items(&args.key, args.limit)?;
    CommandOutput::new(ctx, items, None, |items| print_items(items))
}

pub(crate) async fn handle_open(ctx: &AppContext, args: ItemOpenArgs) -> Result<CommandOutput> {
    let library = ctx.local_library()?;
    let item = require_item(&library, &args.key)?;
    let target = if args.url {
        item.url
            .clone()
            .or_else(|| {
                item.doi
                    .as_deref()
                    .map(|doi| format!("https://doi.org/{doi}"))
            })
            .ok_or_else(|| zot_core::ZotError::InvalidInput {
                code: "item-no-url".to_string(),
                message: format!("Item '{}' has no URL or DOI", args.key),
                hint: None,
            })?
    } else {
        let attachment = require_item_pdf(&library, &args.key)?;
        library.pdf_path(&attachment).display().to_string()
    };
    open_target(&target)?;
    let payload = serde_json::json!({ "opened": target });
    CommandOutput::new(ctx, payload, None, move |_| println!("Opened {target}"))
}

pub(crate) async fn handle_pdf(ctx: &AppContext, args: ItemPdfArgs) -> Result<CommandOutput> {
    let library = ctx.local_library()?;
    let attachment = require_item_pdf(&library, &args.key)?;
    let pdf_path = library.pdf_path(&attachment);
    let backend = ctx.pdf_backend();
    let cache = PdfCache::new(None)?;
    if args.annotations {
        let annotations = {
            let pdf_path = pdf_path.clone();
            run_pdf(move || backend.extract_annotations(&pdf_path)).await?
        };
        return CommandOutput::new(ctx, annotations, None, |annotations| {
            for annotation in annotations {
                println!(
                    "[p.{}] {} {}",
                    annotation.page, annotation.annotation_type, annotation.content
                );
            }
        });
    }
    let page_range = parse_page_range(args.pages.as_deref())?;
    let text = if page_range.is_none() {
        if let Some(cached) = cache.get(&pdf_path)? {
            cached
        } else {
            let extracted = {
                let pdf_path = pdf_path.clone();
                run_pdf(move || backend.extract_text(&pdf_path, None)).await?
            };
            cache.put(&pdf_path, &extracted)?;
            extracted
        }
    } else {
        let pdf_path = pdf_path.clone();
        run_pdf(move || backend.extract_text(&pdf_path, page_range)).await?
    };
    let payload = serde_json::json!({ "text": text });
    CommandOutput::new(ctx, payload, None, move |_| println!("{text}"))
}

pub(crate) async fn handle_children(
    ctx: &AppContext,
    args: ItemChildrenArgs,
) -> Result<CommandOutput> {
    let children = ctx.local_library()?.get_items_children(&args.keys)?;
    CommandOutput::new(ctx, children, None, |children| {
        for (key, values) in children {
            println!("{key}");
            for value in values {
                println!("  - {} [{}]", value.key(), value.kind_label());
            }
        }
    })
}

pub(crate) async fn handle_download(
    ctx: &AppContext,
    args: ItemDownloadArgs,
) -> Result<CommandOutput> {
    let library = ctx.local_library()?;
    let attachment = library.get_attachment_by_key(&args.key)?.ok_or_else(|| {
        zot_core::ZotError::InvalidInput {
            code: "attachment-not-found".to_string(),
            message: format!("Attachment '{}' not found", args.key),
            hint: Some("Pass an attachment item key such as ATCH005".to_string()),
        }
    })?;
    let source = library.attachment_path(&attachment);
    if !source.exists() {
        return Err(zot_core::ZotError::Io {
            path: source,
            source: std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Attachment file is missing from local Zotero storage",
            ),
        }
        .into());
    }
    let destination = resolve_download_path(args.output, &attachment.filename)?;
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent).map_err(|source| zot_core::ZotError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    std::fs::copy(&source, &destination).map_err(|source| zot_core::ZotError::Io {
        path: destination.clone(),
        source,
    })?;
    let payload = serde_json::json!({
        "attachment_key": args.key,
        "path": zot_core::canonicalize_or_original(&destination),
    });
    CommandOutput::new(ctx, payload, None, move |_| {
        println!("{}", destination.display())
    })
}

pub(crate) async fn handle_deleted(
    ctx: &AppContext,
    args: ItemDeletedArgs,
) -> Result<CommandOutput> {
    let items = ctx.local_library()?.get_trash_items(args.limit)?;
    CommandOutput::new(ctx, items, None, |items| print_items(items))
}

pub(crate) async fn handle_versions(
    ctx: &AppContext,
    args: ItemVersionsArgs,
) -> Result<CommandOutput> {
    let versions = ctx.remote()?.list_item_versions(args.since).await?;
    CommandOutput::new(ctx, versions, None, |versions| {
        if versions.is_empty() {
            println!("No item versions found.");
        } else {
            for (key, version) in versions {
                println!("{key} {version}");
            }
        }
    })
}

pub(crate) async fn handle_outline(ctx: &AppContext, key: &str) -> Result<CommandOutput> {
    let library = ctx.local_library()?;
    let attachment = require_item_pdf(&library, key)?;
    let backend = ctx.pdf_backend();
    let pdf_path = library.pdf_path(&attachment);
    let entries = run_pdf(move || backend.extract_outline(&pdf_path)).await?;
    CommandOutput::new(ctx, entries, None, |entries| {
        if entries.is_empty() {
            println!("This PDF does not contain a table of contents/outline.");
        } else {
            print_outline_entries(entries);
        }
    })
}

pub(crate) async fn handle_export(ctx: &AppContext, args: ItemExportArgs) -> Result<CommandOutput> {
    let library = ctx.local_library()?;
    let export = library
        .export_citation(&args.key, &args.format)?
        .ok_or_else(|| item_not_found(&args.key))?;
    let payload = serde_json::json!({ "format": args.format, "content": export });
    CommandOutput::new(ctx, payload, None, move |_| println!("{export}"))
}

pub(crate) async fn handle_cite(ctx: &AppContext, args: ItemCiteArgs) -> Result<CommandOutput> {
    let library = ctx.local_library()?;
    let item = require_item(&library, &args.key)?;
    let citation = zot_local::format_citation(&item, args.style.into());
    let payload = serde_json::json!({ "citation": citation });
    CommandOutput::new(ctx, payload, None, move |_| println!("{citation}"))
}

fn resolve_download_path(output: Option<PathBuf>, filename: &str) -> zot_core::ZotResult<PathBuf> {
    let destination = match output {
        Some(path) if path.is_dir() => path.join(filename),
        Some(path) => path,
        None => std::env::current_dir()
            .map_err(|source| zot_core::ZotError::Io {
                path: PathBuf::from("."),
                source,
            })?
            .join(filename),
    };
    Ok(destination)
}

#[cfg(test)]
mod tests {
    use super::resolve_download_path;

    #[test]
    fn resolves_download_path_inside_directory() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let path = resolve_download_path(Some(tempdir.path().to_path_buf()), "paper.pdf")
            .expect("download path");
        assert_eq!(
            path.file_name().and_then(|value| value.to_str()),
            Some("paper.pdf")
        );
    }

    #[test]
    fn keeps_explicit_download_filename() {
        let path =
            resolve_download_path(Some("custom.pdf".into()), "paper.pdf").expect("download path");
        assert_eq!(
            path.file_name().and_then(|value| value.to_str()),
            Some("custom.pdf")
        );
    }
}
