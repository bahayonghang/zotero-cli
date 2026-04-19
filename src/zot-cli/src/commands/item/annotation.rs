use anyhow::Result;
use zot_local::{PdfBackend, PdfiumBackend};

use crate::cli::{AnnotationCreateAreaArgs, ItemAnnotationCommand};
use crate::context::AppContext;
use crate::format::print_enveloped;

pub(crate) async fn handle(ctx: &AppContext, command: ItemAnnotationCommand) -> Result<()> {
    match command {
        ItemAnnotationCommand::List(args) => {
            let annotations = ctx
                .local_library()?
                .get_annotations(args.item_key.as_deref(), args.limit)?;
            if ctx.json {
                print_enveloped(&annotations, None)?;
            } else if annotations.is_empty() {
                println!("No annotations found.");
            } else {
                for annotation in annotations {
                    println!(
                        "{} [{}] {}",
                        annotation.key, annotation.annotation_type, annotation.text
                    );
                }
            }
        }
        ItemAnnotationCommand::Search(args) => {
            let annotations = ctx
                .local_library()?
                .search_annotations(&args.query, args.limit)?;
            if ctx.json {
                print_enveloped(&annotations, None)?;
            } else if annotations.is_empty() {
                println!("No annotations found.");
            } else {
                for annotation in annotations {
                    println!(
                        "{} [{}] {}",
                        annotation.key, annotation.annotation_type, annotation.text
                    );
                }
            }
        }
        ItemAnnotationCommand::Create(args) => {
            let payload = create_highlight_annotation(
                ctx,
                &args.attachment_key,
                args.page,
                &args.text,
                args.comment.as_deref(),
                &args.color,
            )
            .await?;
            if ctx.json {
                print_enveloped(payload, None)?;
            } else {
                println!("{}", serde_json::to_string_pretty(&payload)?);
            }
        }
        ItemAnnotationCommand::CreateArea(args) => {
            let payload = create_area_annotation(ctx, &args).await?;
            if ctx.json {
                print_enveloped(payload, None)?;
            } else {
                println!("{}", serde_json::to_string_pretty(&payload)?);
            }
        }
    }
    Ok(())
}

async fn create_highlight_annotation(
    ctx: &AppContext,
    attachment_key: &str,
    page: usize,
    text: &str,
    comment: Option<&str>,
    color: &str,
) -> Result<serde_json::Value> {
    let library = ctx.local_library()?;
    let attachment = library
        .get_attachment_by_key(attachment_key)?
        .ok_or_else(|| zot_core::ZotError::InvalidInput {
            code: "attachment-not-found".to_string(),
            message: format!("Attachment '{}' not found", attachment_key),
            hint: None,
        })?;
    if attachment.content_type != "application/pdf" {
        return Err(zot_core::ZotError::InvalidInput {
            code: "attachment-not-pdf".to_string(),
            message: format!("Attachment '{}' is not a PDF attachment", attachment_key),
            hint: None,
        }
        .into());
    }
    let pdf_path = library.pdf_path(&attachment);
    let backend = PdfiumBackend;
    let position = backend
        .find_text_position(&pdf_path, page, text)?
        .ok_or_else(|| zot_core::ZotError::Pdf {
            code: "annotation-text-not-found".to_string(),
            message: "Could not find the requested text on the target page".to_string(),
            hint: Some("Try a shorter exact phrase copied from the PDF".to_string()),
        })?;
    let payload = serde_json::json!({
        "itemType": "annotation",
        "parentItem": attachment_key,
        "annotationType": "highlight",
        "annotationText": text,
        "annotationComment": comment.unwrap_or(""),
        "annotationColor": color,
        "annotationSortIndex": position.sort_index,
        "annotationPosition": build_annotation_position_json(position.page_index, &position.rects),
        "annotationPageLabel": position.page_label,
    });
    let key = ctx.remote()?.create_item_from_value(payload).await?;
    Ok(serde_json::json!({
        "annotation_key": key,
        "page": position.page_label,
        "text": text,
        "color": color,
    }))
}

async fn create_area_annotation(
    ctx: &AppContext,
    args: &AnnotationCreateAreaArgs,
) -> Result<serde_json::Value> {
    let library = ctx.local_library()?;
    let attachment = library
        .get_attachment_by_key(&args.attachment_key)?
        .ok_or_else(|| zot_core::ZotError::InvalidInput {
            code: "attachment-not-found".to_string(),
            message: format!("Attachment '{}' not found", args.attachment_key),
            hint: None,
        })?;
    if attachment.content_type != "application/pdf" {
        return Err(zot_core::ZotError::InvalidInput {
            code: "attachment-not-pdf".to_string(),
            message: format!(
                "Attachment '{}' is not a PDF attachment",
                args.attachment_key
            ),
            hint: None,
        }
        .into());
    }
    let pdf_path = library.pdf_path(&attachment);
    let backend = PdfiumBackend;
    let position = backend.build_area_position(
        &pdf_path,
        args.page,
        args.x,
        args.y,
        args.width,
        args.height,
    )?;
    let payload = serde_json::json!({
        "itemType": "annotation",
        "parentItem": args.attachment_key,
        "annotationType": "image",
        "annotationComment": args.comment.as_deref().unwrap_or(""),
        "annotationColor": args.color,
        "annotationSortIndex": position.sort_index,
        "annotationPosition": build_annotation_position_json(position.page_index, &position.rects),
        "annotationPageLabel": position.page_label,
    });
    let key = ctx.remote()?.create_item_from_value(payload).await?;
    Ok(serde_json::json!({
        "annotation_key": key,
        "page": position.page_label,
        "rects": position.rects,
        "color": args.color,
    }))
}

fn build_annotation_position_json(page_index: usize, rects: &[[f32; 4]]) -> String {
    serde_json::json!({
        "pageIndex": page_index,
        "rects": rects,
    })
    .to_string()
}
