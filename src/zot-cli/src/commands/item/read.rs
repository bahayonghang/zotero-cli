use std::path::PathBuf;

use anyhow::Result;
use zot_local::{PdfBackend, PdfCache, PdfiumBackend};

use crate::cli::{
    ItemChildrenArgs, ItemCiteArgs, ItemDeletedArgs, ItemDownloadArgs, ItemExportArgs, ItemKeyArgs,
    ItemOpenArgs, ItemPdfArgs, ItemRelatedArgs, ItemVersionsArgs,
};
use crate::context::AppContext;
use crate::format::{print_enveloped, print_item, print_items};
use crate::util::{open_target, parse_page_range, print_outline_entries};

pub(crate) async fn handle_get(ctx: &AppContext, args: ItemKeyArgs) -> Result<()> {
    let library = ctx.local_library()?;
    let item = library
        .get_item(&args.key)?
        .ok_or_else(|| zot_core::ZotError::InvalidInput {
            code: "item-not-found".to_string(),
            message: format!("Item '{}' not found", args.key),
            hint: None,
        })?;
    let notes = library.get_notes(&args.key)?;
    let attachments = library.get_attachments(&args.key)?;
    if ctx.json {
        let payload = serde_json::json!({
            "item": item,
            "notes": notes,
            "attachments": attachments,
        });
        print_enveloped(payload, None)?;
    } else {
        print_item(&item, &notes, &attachments);
    }
    Ok(())
}

pub(crate) async fn handle_related(ctx: &AppContext, args: ItemRelatedArgs) -> Result<()> {
    let library = ctx.local_library()?;
    let items = library.get_related_items(&args.key, args.limit)?;
    if ctx.json {
        print_enveloped(&items, None)?;
    } else {
        print_items(&items);
    }
    Ok(())
}

pub(crate) async fn handle_open(ctx: &AppContext, args: ItemOpenArgs) -> Result<()> {
    let library = ctx.local_library()?;
    let item = library
        .get_item(&args.key)?
        .ok_or_else(|| zot_core::ZotError::InvalidInput {
            code: "item-not-found".to_string(),
            message: format!("Item '{}' not found", args.key),
            hint: None,
        })?;
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
        let attachment = library.get_pdf_attachment(&args.key)?.ok_or_else(|| {
            zot_core::ZotError::InvalidInput {
                code: "item-no-pdf".to_string(),
                message: format!("Item '{}' has no PDF attachment", args.key),
                hint: None,
            }
        })?;
        library.pdf_path(&attachment).display().to_string()
    };
    open_target(&target)?;
    if ctx.json {
        print_enveloped(serde_json::json!({ "opened": target }), None)?;
    } else {
        println!("Opened {target}");
    }
    Ok(())
}

pub(crate) async fn handle_pdf(ctx: &AppContext, args: ItemPdfArgs) -> Result<()> {
    let library = ctx.local_library()?;
    let attachment =
        library
            .get_pdf_attachment(&args.key)?
            .ok_or_else(|| zot_core::ZotError::InvalidInput {
                code: "item-no-pdf".to_string(),
                message: format!("Item '{}' has no PDF attachment", args.key),
                hint: None,
            })?;
    let pdf_path = library.pdf_path(&attachment);
    let backend = PdfiumBackend;
    let cache = PdfCache::new(None)?;
    if args.annotations {
        let annotations = backend.extract_annotations(&pdf_path)?;
        if ctx.json {
            print_enveloped(&annotations, None)?;
        } else {
            for annotation in annotations {
                println!(
                    "[p.{}] {} {}",
                    annotation.page, annotation.annotation_type, annotation.content
                );
            }
        }
        return Ok(());
    }
    let page_range = parse_page_range(args.pages.as_deref())?;
    let text = if page_range.is_none() {
        if let Some(cached) = cache.get(&pdf_path)? {
            cached
        } else {
            let extracted = backend.extract_text(&pdf_path, None)?;
            cache.put(&pdf_path, &extracted)?;
            extracted
        }
    } else {
        backend.extract_text(&pdf_path, page_range)?
    };
    if ctx.json {
        print_enveloped(serde_json::json!({ "text": text }), None)?;
    } else {
        println!("{text}");
    }
    Ok(())
}

pub(crate) async fn handle_children(ctx: &AppContext, args: ItemChildrenArgs) -> Result<()> {
    let children = ctx.local_library()?.get_items_children(&args.keys)?;
    if ctx.json {
        print_enveloped(&children, None)?;
    } else {
        for (key, values) in children {
            println!("{key}");
            for value in values {
                println!("  - {} [{}]", value.key, value.item_type);
            }
        }
    }
    Ok(())
}

pub(crate) async fn handle_download(ctx: &AppContext, args: ItemDownloadArgs) -> Result<()> {
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
    if ctx.json {
        print_enveloped(
            serde_json::json!({
                "attachment_key": args.key,
                "path": zot_core::canonicalize_or_original(&destination),
            }),
            None,
        )?;
    } else {
        println!("{}", destination.display());
    }
    Ok(())
}

pub(crate) async fn handle_deleted(ctx: &AppContext, args: ItemDeletedArgs) -> Result<()> {
    let items = ctx.local_library()?.get_trash_items(args.limit)?;
    if ctx.json {
        print_enveloped(&items, None)?;
    } else {
        print_items(&items);
    }
    Ok(())
}

pub(crate) async fn handle_versions(ctx: &AppContext, args: ItemVersionsArgs) -> Result<()> {
    let versions = ctx.remote()?.list_item_versions(args.since).await?;
    if ctx.json {
        print_enveloped(&versions, None)?;
    } else if versions.is_empty() {
        println!("No item versions found.");
    } else {
        for (key, version) in versions {
            println!("{key} {version}");
        }
    }
    Ok(())
}

pub(crate) async fn handle_outline(ctx: &AppContext, key: &str) -> Result<()> {
    let library = ctx.local_library()?;
    let attachment =
        library
            .get_pdf_attachment(key)?
            .ok_or_else(|| zot_core::ZotError::InvalidInput {
                code: "item-no-pdf".to_string(),
                message: format!("Item '{}' has no PDF attachment", key),
                hint: None,
            })?;
    let backend = PdfiumBackend;
    let entries = backend.extract_outline(&library.pdf_path(&attachment))?;
    if ctx.json {
        print_enveloped(&entries, None)?;
    } else if entries.is_empty() {
        println!("This PDF does not contain a table of contents/outline.");
    } else {
        print_outline_entries(&entries);
    }
    Ok(())
}

pub(crate) async fn handle_export(ctx: &AppContext, args: ItemExportArgs) -> Result<()> {
    let library = ctx.local_library()?;
    let export = library
        .export_citation(&args.key, &args.format)?
        .ok_or_else(|| zot_core::ZotError::InvalidInput {
            code: "item-not-found".to_string(),
            message: format!("Item '{}' not found", args.key),
            hint: None,
        })?;
    if ctx.json {
        print_enveloped(
            serde_json::json!({ "format": args.format, "content": export }),
            None,
        )?;
    } else {
        println!("{export}");
    }
    Ok(())
}

pub(crate) async fn handle_cite(ctx: &AppContext, args: ItemCiteArgs) -> Result<()> {
    let library = ctx.local_library()?;
    let item = library
        .get_item(&args.key)?
        .ok_or_else(|| zot_core::ZotError::InvalidInput {
            code: "item-not-found".to_string(),
            message: format!("Item '{}' not found", args.key),
            hint: None,
        })?;
    let citation = zot_local::format_citation(&item, args.style.into());
    if ctx.json {
        print_enveloped(serde_json::json!({ "citation": citation }), None)?;
    } else {
        println!("{citation}");
    }
    Ok(())
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
