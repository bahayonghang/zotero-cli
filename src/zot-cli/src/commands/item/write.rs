use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;

use anyhow::Result;
use zot_local::{PdfBackend, PdfiumBackend};
use zot_remote::oa::CreatorName;
use zot_remote::{HttpRuntime, OaClient, ZoteroRemote, normalize_arxiv_id, normalize_doi};

use super::merge::merge_item_set;
use crate::cli::{
    AddByDoiArgs, AddByUrlArgs, AddFromFileArgs, AttachModeArg, ItemAttachArgs, ItemCreateArgs,
    ItemKeyArgs, ItemMergeArgs, ItemUpdateArgs,
};
use crate::context::AppContext;
use crate::format::print_enveloped;

pub(crate) async fn handle_create(ctx: &AppContext, args: ItemCreateArgs) -> Result<()> {
    let key = if let Some(pdf) = args.pdf.as_deref() {
        add_item_from_file(
            ctx,
            pdf,
            None,
            "document",
            args.doi.as_deref(),
            &args.collections,
            &args.tags,
        )
        .await?
    } else if let Some(doi) = args.doi.as_deref() {
        add_item_by_doi(ctx, doi, &args.collections, &args.tags, args.attach_mode).await?
    } else if let Some(url) = args.url.as_deref() {
        add_item_by_url(ctx, url, &args.collections, &args.tags, args.attach_mode).await?
    } else {
        return Err(zot_core::ZotError::InvalidInput {
            code: "item-create".to_string(),
            message: "Provide --doi, --url, or --pdf".to_string(),
            hint: None,
        }
        .into());
    };
    if ctx.json {
        print_enveloped(serde_json::json!({ "key": key }), None)?;
    } else {
        println!("Created item: {key}");
    }
    Ok(())
}

pub(crate) async fn handle_add_doi(ctx: &AppContext, args: AddByDoiArgs) -> Result<()> {
    let key = add_item_by_doi(
        ctx,
        &args.doi,
        &args.collections,
        &args.tags,
        args.attach_mode,
    )
    .await?;
    if ctx.json {
        print_enveloped(serde_json::json!({ "key": key }), None)?;
    } else {
        println!("Created item: {key}");
    }
    Ok(())
}

pub(crate) async fn handle_add_url(ctx: &AppContext, args: AddByUrlArgs) -> Result<()> {
    let key = add_item_by_url(
        ctx,
        &args.url,
        &args.collections,
        &args.tags,
        args.attach_mode,
    )
    .await?;
    if ctx.json {
        print_enveloped(serde_json::json!({ "key": key }), None)?;
    } else {
        println!("Created item: {key}");
    }
    Ok(())
}

pub(crate) async fn handle_add_file(ctx: &AppContext, args: AddFromFileArgs) -> Result<()> {
    let key = add_item_from_file(
        ctx,
        &args.file,
        args.title.as_deref(),
        &args.item_type,
        args.doi.as_deref(),
        &args.collections,
        &args.tags,
    )
    .await?;
    if ctx.json {
        print_enveloped(serde_json::json!({ "key": key }), None)?;
    } else {
        println!("Created item: {key}");
    }
    Ok(())
}

pub(crate) async fn handle_merge(ctx: &AppContext, args: ItemMergeArgs) -> Result<()> {
    let keeper_key = match args.keep.as_deref() {
        Some(key) if key == args.key1 => args.key1.as_str(),
        Some(key) if key == args.key2 => args.key2.as_str(),
        Some(key) => {
            return Err(zot_core::ZotError::InvalidInput {
                code: "item-merge".to_string(),
                message: format!(
                    "--keep must match one of the provided keys ('{}' or '{}'), got '{}'",
                    args.key1, args.key2, key
                ),
                hint: None,
            }
            .into());
        }
        None => args.key1.as_str(),
    };
    let source_keys = [args.key1.clone(), args.key2.clone()]
        .into_iter()
        .filter(|key| key != keeper_key)
        .collect::<Vec<_>>();
    let operation = merge_item_set(&ctx.remote()?, keeper_key, &source_keys, args.confirm).await?;

    if ctx.json {
        print_enveloped(operation, None)?;
    } else {
        println!("{}", serde_json::to_string_pretty(&operation)?);
    }
    Ok(())
}

pub(crate) async fn handle_update(ctx: &AppContext, args: ItemUpdateArgs) -> Result<()> {
    let mut fields = BTreeMap::new();
    if let Some(title) = args.title {
        fields.insert("title".to_string(), title);
    }
    if let Some(date) = args.date {
        fields.insert("date".to_string(), date);
    }
    for field in args.fields {
        if let Some((key, value)) = field.split_once('=') {
            fields.insert(key.to_string(), value.to_string());
        }
    }
    ctx.remote()?.update_item_fields(&args.key, &fields).await?;
    if ctx.json {
        print_enveloped(
            serde_json::json!({ "updated": args.key, "fields": fields }),
            None,
        )?;
    } else {
        println!("Updated {}", args.key);
    }
    Ok(())
}

pub(crate) async fn handle_trash(ctx: &AppContext, args: ItemKeyArgs) -> Result<()> {
    ctx.remote()?.delete_item(&args.key).await?;
    if ctx.json {
        print_enveloped(serde_json::json!({ "trashed": args.key }), None)?;
    } else {
        println!("Moved to trash: {}", args.key);
    }
    Ok(())
}

pub(crate) async fn handle_restore(ctx: &AppContext, args: ItemKeyArgs) -> Result<()> {
    ctx.remote()?.restore_item(&args.key).await?;
    if ctx.json {
        print_enveloped(serde_json::json!({ "restored": args.key }), None)?;
    } else {
        println!("Restored: {}", args.key);
    }
    Ok(())
}

pub(crate) async fn handle_attach(ctx: &AppContext, args: ItemAttachArgs) -> Result<()> {
    let key = ctx
        .remote()?
        .upload_attachment(&args.key, &args.file)
        .await?;
    if ctx.json {
        print_enveloped(serde_json::json!({ "attachment_key": key }), None)?;
    } else {
        println!("Attachment uploaded: {key}");
    }
    Ok(())
}

async fn add_item_by_doi(
    ctx: &AppContext,
    doi: &str,
    collections: &[String],
    tags: &[String],
    attach_mode: AttachModeArg,
) -> Result<String> {
    let doi = normalize_doi(doi).ok_or_else(|| zot_core::ZotError::InvalidInput {
        code: "invalid-doi".to_string(),
        message: format!("'{}' does not appear to be a valid DOI", doi),
        hint: None,
    })?;
    let oa = OaClient::new(ctx.http());
    let work = oa.fetch_crossref_work(&doi).await?;
    let remote = ctx.remote()?;
    let key = remote
        .create_item_from_value(build_crossref_item_payload(&work, collections, tags))
        .await?;
    if !matches!(attach_mode, AttachModeArg::None) {
        maybe_attach_open_access_pdf(ctx.http(), &remote, &key, &doi, Some(&work), attach_mode)
            .await?;
    }
    Ok(key)
}

async fn add_item_by_url(
    ctx: &AppContext,
    url: &str,
    collections: &[String],
    tags: &[String],
    attach_mode: AttachModeArg,
) -> Result<String> {
    if let Some(doi) = normalize_doi(url) {
        return add_item_by_doi(ctx, &doi, collections, tags, attach_mode).await;
    }
    let remote = ctx.remote()?;
    if let Some(arxiv_id) = normalize_arxiv_id(url) {
        let work = OaClient::new(ctx.http())
            .fetch_arxiv_work(&arxiv_id)
            .await?;
        let key = remote
            .create_item_from_value(build_arxiv_item_payload(&work, collections, tags))
            .await?;
        if !matches!(attach_mode, AttachModeArg::None) {
            maybe_attach_pdf_url(
                ctx.http(),
                &remote,
                &key,
                &work.pdf_url,
                &format!("arxiv_{}.pdf", arxiv_id.replace('/', "_")),
                attach_mode,
            )
            .await?;
        }
        return Ok(key);
    }
    remote
        .create_item_from_value(serde_json::json!({
            "itemType": "webpage",
            "title": url,
            "url": url,
            "accessDate": "",
            "collections": collections,
            "tags": tags.iter().map(|tag| serde_json::json!({ "tag": tag })).collect::<Vec<_>>(),
        }))
        .await
        .map_err(Into::into)
}

async fn add_item_from_file(
    ctx: &AppContext,
    file: &Path,
    title: Option<&str>,
    item_type: &str,
    doi_override: Option<&str>,
    collections: &[String],
    tags: &[String],
) -> Result<String> {
    let backend = PdfiumBackend;
    let resolved_doi = if let Some(doi) = doi_override {
        Some(
            normalize_doi(doi).ok_or_else(|| zot_core::ZotError::InvalidInput {
                code: "invalid-doi".to_string(),
                message: format!("'{}' does not appear to be a valid DOI", doi),
                hint: None,
            })?,
        )
    } else if file
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("pdf"))
        .unwrap_or(false)
    {
        backend.extract_doi(file)?
    } else {
        None
    };
    let remote = ctx.remote()?;
    let key = if let Some(doi) = resolved_doi.as_deref() {
        let key = add_item_by_doi(ctx, doi, collections, tags, AttachModeArg::None).await?;
        remote.upload_attachment(&key, file).await?;
        key
    } else {
        let payload = serde_json::json!({
            "itemType": item_type,
            "title": title.unwrap_or_else(|| file.file_name().and_then(|name| name.to_str()).unwrap_or("document")),
            "collections": collections,
            "tags": tags.iter().map(|tag| serde_json::json!({ "tag": tag })).collect::<Vec<_>>(),
        });
        let key = remote.create_item_from_value(payload).await?;
        remote.upload_attachment(&key, file).await?;
        key
    };
    Ok(key)
}

async fn maybe_attach_open_access_pdf(
    runtime: &HttpRuntime,
    remote: &ZoteroRemote,
    item_key: &str,
    doi: &str,
    crossref: Option<&zot_remote::CrossRefWork>,
    attach_mode: AttachModeArg,
) -> Result<()> {
    if matches!(attach_mode, AttachModeArg::None) {
        return Ok(());
    }
    if let Some(resolved) = OaClient::new(runtime)
        .resolve_open_access_pdf(doi, crossref)
        .await?
    {
        maybe_attach_pdf_url(
            runtime,
            remote,
            item_key,
            &resolved.url,
            &format!("{}.pdf", doi.replace('/', "_")),
            attach_mode,
        )
        .await?;
    }
    Ok(())
}

async fn maybe_attach_pdf_url(
    runtime: &HttpRuntime,
    remote: &ZoteroRemote,
    item_key: &str,
    url: &str,
    filename: &str,
    attach_mode: AttachModeArg,
) -> Result<()> {
    match attach_mode {
        AttachModeArg::None => {}
        AttachModeArg::LinkedUrl => {
            remote
                .add_linked_attachment(item_key, url, "PDF (linked URL)")
                .await?;
        }
        AttachModeArg::Auto => {
            let response = runtime.client().get(url).send().await.map_err(|err| {
                zot_core::ZotError::Remote {
                    code: "pdf-download".to_string(),
                    message: err.to_string(),
                    hint: None,
                    status: err.status().map(|status| status.as_u16()),
                }
            })?;
            if !response.status().is_success() {
                return Ok(());
            }
            let bytes = response
                .bytes()
                .await
                .map_err(|err| zot_core::ZotError::Remote {
                    code: "pdf-download-bytes".to_string(),
                    message: err.to_string(),
                    hint: None,
                    status: err.status().map(|status| status.as_u16()),
                })?;
            let path = std::env::temp_dir().join(format!("{}-{}", uuid::Uuid::new_v4(), filename));
            let mut file =
                std::fs::File::create(&path).map_err(|source| zot_core::ZotError::Io {
                    path: path.clone(),
                    source,
                })?;
            file.write_all(&bytes)
                .map_err(|source| zot_core::ZotError::Io {
                    path: path.clone(),
                    source,
                })?;
            let upload_result = remote.upload_attachment(item_key, &path).await;
            let _ = std::fs::remove_file(&path);
            upload_result?;
        }
    }
    Ok(())
}

fn build_crossref_item_payload(
    work: &zot_remote::CrossRefWork,
    collections: &[String],
    tags: &[String],
) -> serde_json::Value {
    serde_json::json!({
        "itemType": crossref_type_to_zotero(&work.record_type),
        "title": work.title.clone().unwrap_or_else(|| work.doi.clone()),
        "creators": work.creators.iter().map(creator_to_json).collect::<Vec<_>>(),
        "date": work.date,
        "DOI": work.doi,
        "url": work.url,
        "volume": work.volume,
        "issue": work.issue,
        "pages": work.pages,
        "publisher": work.publisher,
        "ISSN": work.issn,
        "publicationTitle": work.publication_title,
        "abstractNote": work.abstract_note,
        "collections": collections,
        "tags": tags.iter().map(|tag| serde_json::json!({ "tag": tag })).collect::<Vec<_>>(),
    })
}

fn build_arxiv_item_payload(
    work: &zot_remote::ArxivWork,
    collections: &[String],
    tags: &[String],
) -> serde_json::Value {
    serde_json::json!({
        "itemType": "preprint",
        "title": work.title,
        "creators": work.creators.iter().map(creator_to_json).collect::<Vec<_>>(),
        "abstractNote": work.abstract_note,
        "date": work.date,
        "url": work.abs_url,
        "extra": format!("arXiv:{}", work.arxiv_id),
        "collections": collections,
        "tags": tags.iter().map(|tag| serde_json::json!({ "tag": tag })).collect::<Vec<_>>(),
    })
}

fn creator_to_json(creator: &CreatorName) -> serde_json::Value {
    serde_json::json!({
        "creatorType": creator.creator_type,
        "firstName": creator.first_name,
        "lastName": creator.last_name,
    })
}

fn crossref_type_to_zotero(value: &str) -> &'static str {
    match value {
        "journal-article" => "journalArticle",
        "book" => "book",
        "book-chapter" => "bookSection",
        "proceedings-article" => "conferencePaper",
        "report" => "report",
        "dissertation" => "thesis",
        "posted-content" => "preprint",
        _ => "document",
    }
}
