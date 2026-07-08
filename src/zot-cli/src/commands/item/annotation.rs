use anyhow::Result;
use zot_local::PdfBackend;

use crate::cli::{AnnotationCreateAreaArgs, ItemAnnotationCommand};
use crate::context::AppContext;
use crate::output::CommandOutput;
use crate::util::{require_pdf_attachment, run_pdf};

pub(crate) async fn handle(
    ctx: &AppContext,
    command: ItemAnnotationCommand,
) -> Result<CommandOutput> {
    match command {
        ItemAnnotationCommand::List(args) => {
            let annotations = ctx
                .local_library()?
                .get_annotations(args.item_key.as_deref(), args.limit)?;
            CommandOutput::new(ctx, annotations, None, |annotations| {
                if annotations.is_empty() {
                    println!("No annotations found.");
                } else {
                    for annotation in annotations {
                        println!(
                            "{} [{}] {}",
                            annotation.key, annotation.annotation_type, annotation.text
                        );
                    }
                }
            })
        }
        ItemAnnotationCommand::Search(args) => {
            let annotations = ctx
                .local_library()?
                .search_annotations(&args.query, args.limit)?;
            CommandOutput::new(ctx, annotations, None, |annotations| {
                if annotations.is_empty() {
                    println!("No annotations found.");
                } else {
                    for annotation in annotations {
                        println!(
                            "{} [{}] {}",
                            annotation.key, annotation.annotation_type, annotation.text
                        );
                    }
                }
            })
        }
        ItemAnnotationCommand::Create(args) => {
            let payload = create_highlight_annotation(
                ctx,
                &args.attachment_key,
                args.page,
                &args.text,
                args.comment.as_deref(),
                &args.color,
                args.occurrence,
            )
            .await?;
            CommandOutput::new(ctx, payload, None, |payload| {
                println!(
                    "{}",
                    serde_json::to_string_pretty(payload).expect("serialize annotation")
                );
            })
        }
        ItemAnnotationCommand::CreateArea(args) => {
            let payload = create_area_annotation(ctx, &args).await?;
            CommandOutput::new(ctx, payload, None, |payload| {
                println!(
                    "{}",
                    serde_json::to_string_pretty(payload).expect("serialize annotation")
                );
            })
        }
    }
}

async fn create_highlight_annotation(
    ctx: &AppContext,
    attachment_key: &str,
    page: usize,
    text: &str,
    comment: Option<&str>,
    color: &str,
    occurrence: usize,
) -> Result<serde_json::Value> {
    let library = ctx.local_library()?;
    let attachment = require_pdf_attachment(&library, attachment_key)?;
    let pdf_path = library.pdf_path(&attachment);
    let backend = ctx.pdf_backend();
    let position = {
        let text_owned = text.to_string();
        run_pdf(move || backend.find_text_position(&pdf_path, page, &text_owned, occurrence))
            .await?
    }
    .ok_or_else(|| zot_core::ZotError::Pdf {
        code: "annotation-text-not-found".to_string(),
        message: format!(
            "Could not find occurrence {occurrence} of the requested text on the target page"
        ),
        hint: Some(
            "Try a shorter exact phrase copied from the PDF, or lower --occurrence".to_string(),
        ),
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
    let mut summary = serde_json::json!({
        "annotation_key": key,
        "page": position.page_label,
        "text": text,
        "color": color,
        "occurrence": occurrence,
    });
    if let Some(total) = position.total_matches {
        let remaining = total.saturating_sub(occurrence);
        if remaining > 0 {
            summary["more_occurrences"] = serde_json::Value::from(remaining);
        }
        summary["total_matches"] = serde_json::Value::from(total);
    }
    Ok(summary)
}

async fn create_area_annotation(
    ctx: &AppContext,
    args: &AnnotationCreateAreaArgs,
) -> Result<serde_json::Value> {
    let library = ctx.local_library()?;
    let attachment = require_pdf_attachment(&library, &args.attachment_key)?;
    let pdf_path = library.pdf_path(&attachment);
    let backend = ctx.pdf_backend();
    let page = args.page;
    let x = args.x;
    let y = args.y;
    let width = args.width;
    let height = args.height;
    let position =
        run_pdf(move || backend.build_area_position(&pdf_path, page, x, y, width, height)).await?;
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
