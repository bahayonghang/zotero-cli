use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::{Component, Path, PathBuf};

use anyhow::Result;
use zot_local::{PdfBackend, PdfCache};

use crate::cli::{
    ItemChildrenArgs, ItemCiteArgs, ItemDeletedArgs, ItemDownloadArgs, ItemExportArgs, ItemKeyArgs,
    ItemOpenArgs, ItemPdfArgs, ItemRelatedArgs, ItemVersionsArgs, resolved_output_limit,
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
    let items = library.get_related_items(&args.key, resolved_output_limit(args.limit))?;
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
    copy_attachment(&source, &destination, args.force)?;
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
    let items = ctx
        .local_library()?
        .get_trash_items(resolved_output_limit(args.limit))?;
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
        Some(path) if path.is_dir() => path.join(safe_attachment_basename(filename)?),
        Some(path) => path,
        None => std::env::current_dir()
            .map_err(|source| zot_core::ZotError::Io {
                path: PathBuf::from("."),
                source,
            })?
            .join(safe_attachment_basename(filename)?),
    };
    Ok(destination)
}

fn safe_attachment_basename(filename: &str) -> zot_core::ZotResult<PathBuf> {
    let path = Path::new(filename);
    let mut components = path.components();
    let component = components.next();
    let safe = !filename.is_empty()
        && !filename.contains(['/', '\\', ':', '\0'])
        && !filename.chars().any(char::is_control)
        && !path.is_absolute()
        && components.next().is_none()
        && matches!(component, Some(Component::Normal(value)) if !value.is_empty());
    if !safe {
        return Err(zot_core::ZotError::InvalidInput {
            code: "attachment-filename".to_string(),
            message: "Attachment metadata filename is not a safe basename".to_string(),
            hint: Some("Pass an explicit --output file path to choose the destination".to_string()),
        });
    }
    Ok(path.to_path_buf())
}

fn copy_attachment(source: &Path, destination: &Path, force: bool) -> zot_core::ZotResult<()> {
    if force {
        validate_force_destination(source, destination)?;
    }
    let mut input = File::open(source).map_err(|error| zot_core::ZotError::Io {
        path: source.to_path_buf(),
        source: error,
    })?;
    let mut options = OpenOptions::new();
    options.write(true);
    if force {
        options.create(true).truncate(true);
    } else {
        options.create_new(true);
    }
    let mut output = options.open(destination).map_err(|source| {
        if !force && source.kind() == io::ErrorKind::AlreadyExists {
            zot_core::ZotError::InvalidInput {
                code: "attachment-exists".to_string(),
                message: format!("Destination already exists: {}", destination.display()),
                hint: Some("Pass --force to overwrite the existing file".to_string()),
            }
        } else {
            zot_core::ZotError::Io {
                path: destination.to_path_buf(),
                source,
            }
        }
    })?;
    io::copy(&mut input, &mut output).map_err(|source| zot_core::ZotError::Io {
        path: destination.to_path_buf(),
        source,
    })?;
    Ok(())
}

fn validate_force_destination(source: &Path, destination: &Path) -> zot_core::ZotResult<()> {
    if fs::symlink_metadata(destination).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
        return Err(zot_core::ZotError::InvalidInput {
            code: "attachment-destination-symlink".to_string(),
            message: format!(
                "Refusing to overwrite a symbolic-link destination: {}",
                destination.display()
            ),
            hint: Some("Choose a regular output file path".to_string()),
        });
    }
    if destination.exists() {
        let source_path = fs::canonicalize(source).map_err(|error| zot_core::ZotError::Io {
            path: source.to_path_buf(),
            source: error,
        })?;
        let destination_path =
            fs::canonicalize(destination).map_err(|error| zot_core::ZotError::Io {
                path: destination.to_path_buf(),
                source: error,
            })?;
        if source_path == destination_path {
            return Err(zot_core::ZotError::InvalidInput {
                code: "attachment-source-destination".to_string(),
                message: "Source attachment and destination refer to the same file".to_string(),
                hint: Some("Choose a different --output path".to_string()),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{copy_attachment, resolve_download_path, safe_attachment_basename};

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

    #[test]
    fn rejects_untrusted_attachment_filenames_cross_platform() {
        for filename in [
            "",
            ".",
            "..",
            "../escape.pdf",
            "/absolute.pdf",
            "folder/paper.pdf",
            "folder\\paper.pdf",
            "C:\\paper.pdf",
            "paper.pdf:stream",
            "paper\n.pdf",
        ] {
            let err = safe_attachment_basename(filename).expect_err("unsafe basename");
            assert_eq!(err.payload().code, "attachment-filename", "{filename:?}");
        }
        assert_eq!(
            safe_attachment_basename("paper.pdf").expect("safe basename"),
            std::path::PathBuf::from("paper.pdf")
        );
    }

    #[test]
    fn explicit_output_file_ignores_untrusted_metadata_filename() {
        let path = resolve_download_path(Some("reviewed.pdf".into()), "../escape.pdf")
            .expect("explicit path");
        assert_eq!(path, std::path::PathBuf::from("reviewed.pdf"));
    }

    #[test]
    fn download_is_no_clobber_unless_force_is_explicit() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let source = tempdir.path().join("source.pdf");
        let destination = tempdir.path().join("destination.pdf");
        fs::write(&source, b"new").expect("source");
        fs::write(&destination, b"original").expect("destination");

        let err = copy_attachment(&source, &destination, false).expect_err("no clobber");
        assert_eq!(err.payload().code, "attachment-exists");
        assert_eq!(fs::read(&destination).expect("unchanged"), b"original");

        copy_attachment(&source, &destination, true).expect("forced overwrite");
        assert_eq!(fs::read(&destination).expect("overwritten"), b"new");
    }

    #[test]
    fn force_never_truncates_the_source_attachment() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let source = tempdir.path().join("source.pdf");
        fs::write(&source, b"original").expect("source");

        let err = copy_attachment(&source, &source, true).expect_err("same file");
        assert_eq!(err.payload().code, "attachment-source-destination");
        assert_eq!(fs::read(&source).expect("source intact"), b"original");
    }

    #[cfg(unix)]
    #[test]
    fn force_rejects_symbolic_link_destination() {
        use std::os::unix::fs::symlink;

        let tempdir = tempfile::tempdir().expect("tempdir");
        let source = tempdir.path().join("source.pdf");
        let target = tempdir.path().join("target.pdf");
        let destination = tempdir.path().join("destination.pdf");
        fs::write(&source, b"new").expect("source");
        fs::write(&target, b"original").expect("target");
        symlink(&target, &destination).expect("symlink");

        let err = copy_attachment(&source, &destination, true).expect_err("symlink");
        assert_eq!(err.payload().code, "attachment-destination-symlink");
        assert_eq!(fs::read(&target).expect("target intact"), b"original");
    }
}
