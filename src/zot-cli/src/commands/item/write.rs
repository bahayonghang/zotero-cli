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
use crate::output::CommandOutput;
use crate::util::{require_valid_doi, run_pdf};

pub(crate) async fn handle_create(ctx: &AppContext, args: ItemCreateArgs) -> Result<CommandOutput> {
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
    let payload = serde_json::json!({ "key": key });
    CommandOutput::new(ctx, payload, None, move |_| println!("Created item: {key}"))
}

pub(crate) async fn handle_add_doi(ctx: &AppContext, args: AddByDoiArgs) -> Result<CommandOutput> {
    let key = add_item_by_doi(
        ctx,
        &args.doi,
        &args.collections,
        &args.tags,
        args.attach_mode,
    )
    .await?;
    let payload = serde_json::json!({ "key": key });
    CommandOutput::new(ctx, payload, None, move |_| println!("Created item: {key}"))
}

pub(crate) async fn handle_add_url(ctx: &AppContext, args: AddByUrlArgs) -> Result<CommandOutput> {
    let key = add_item_by_url(
        ctx,
        &args.url,
        &args.collections,
        &args.tags,
        args.attach_mode,
    )
    .await?;
    let payload = serde_json::json!({ "key": key });
    CommandOutput::new(ctx, payload, None, move |_| println!("Created item: {key}"))
}

pub(crate) async fn handle_add_file(
    ctx: &AppContext,
    args: AddFromFileArgs,
) -> Result<CommandOutput> {
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
    let payload = serde_json::json!({ "key": key });
    CommandOutput::new(ctx, payload, None, move |_| println!("Created item: {key}"))
}

pub(crate) async fn handle_merge(ctx: &AppContext, args: ItemMergeArgs) -> Result<CommandOutput> {
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

    CommandOutput::new(ctx, operation, None, |operation| {
        println!(
            "{}",
            serde_json::to_string_pretty(operation).expect("serialize merge operation")
        );
    })
}

pub(crate) async fn handle_update(ctx: &AppContext, args: ItemUpdateArgs) -> Result<CommandOutput> {
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
    let key = args.key;
    let payload = serde_json::json!({ "updated": key, "fields": fields });
    CommandOutput::new(ctx, payload, None, move |_| println!("Updated {key}"))
}

pub(crate) async fn handle_trash(ctx: &AppContext, args: ItemKeyArgs) -> Result<CommandOutput> {
    ctx.remote()?.delete_item(&args.key).await?;
    let key = args.key;
    let payload = serde_json::json!({ "trashed": key });
    CommandOutput::new(ctx, payload, None, move |_| println!("Moved to trash: {key}"))
}

pub(crate) async fn handle_restore(ctx: &AppContext, args: ItemKeyArgs) -> Result<CommandOutput> {
    ctx.remote()?.restore_item(&args.key).await?;
    let key = args.key;
    let payload = serde_json::json!({ "restored": key });
    CommandOutput::new(ctx, payload, None, move |_| println!("Restored: {key}"))
}

pub(crate) async fn handle_attach(ctx: &AppContext, args: ItemAttachArgs) -> Result<CommandOutput> {
    let key = ctx
        .remote()?
        .upload_attachment(&args.key, &args.file)
        .await?;
    let payload = serde_json::json!({ "attachment_key": key });
    CommandOutput::new(ctx, payload, None, move |_| {
        println!("Attachment uploaded: {key}")
    })
}

async fn add_item_by_doi(
    ctx: &AppContext,
    doi: &str,
    collections: &[String],
    tags: &[String],
    attach_mode: AttachModeArg,
) -> Result<String> {
    let doi = require_valid_doi(doi)?;
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
    match plan_add_url(url) {
        AddUrlPlan::Doi(doi) => add_item_by_doi(ctx, &doi, collections, tags, attach_mode).await,
        AddUrlPlan::Arxiv(arxiv_id) => {
            let remote = ctx.remote()?;
            let work = OaClient::new(ctx.http())
                .fetch_arxiv_work(&arxiv_id)
                .await?;
            let key = remote
                .create_item_from_value(build_arxiv_item_payload(&work, collections, tags))
                .await?;
            if let Some(attachment) = arxiv_pdf_attachment(&work, &arxiv_id, attach_mode) {
                maybe_attach_pdf_url(
                    ctx.http(),
                    &remote,
                    &key,
                    &attachment.url,
                    &attachment.filename,
                    attach_mode,
                )
                .await?;
            }
            Ok(key)
        }
        AddUrlPlan::Webpage => ctx
            .remote()?
            .create_item_from_value(build_webpage_item_payload(url, collections, tags))
            .await
            .map_err(Into::into),
    }
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
        Some(require_valid_doi(doi)?)
    } else if file
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("pdf"))
        .unwrap_or(false)
    {
        let file_owned = file.to_path_buf();
        run_pdf(move || backend.extract_doi(&file_owned)).await?
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

#[derive(Debug, PartialEq)]
enum AddUrlPlan {
    Doi(String),
    Arxiv(String),
    Webpage,
}

/// Classify an add-by-url input into its target route. Pure: only normalizes
/// the string, no I/O. The DOI and arXiv routes carry the normalized identifier
/// the shell fetches metadata for; the webpage route builds its payload from the
/// raw url.
fn plan_add_url(url: &str) -> AddUrlPlan {
    if let Some(doi) = normalize_doi(url) {
        AddUrlPlan::Doi(doi)
    } else if let Some(arxiv_id) = normalize_arxiv_id(url) {
        AddUrlPlan::Arxiv(arxiv_id)
    } else {
        AddUrlPlan::Webpage
    }
}

struct PdfAttachment {
    url: String,
    filename: String,
}

/// Decide whether an arXiv item should get a PDF attachment and under what
/// filename. Pure: returns the download target without performing the download.
fn arxiv_pdf_attachment(
    work: &zot_remote::ArxivWork,
    arxiv_id: &str,
    attach_mode: AttachModeArg,
) -> Option<PdfAttachment> {
    if matches!(attach_mode, AttachModeArg::None) {
        return None;
    }
    Some(PdfAttachment {
        url: work.pdf_url.clone(),
        filename: format!("arxiv_{}.pdf", arxiv_id.replace('/', "_")),
    })
}

fn build_webpage_item_payload(
    url: &str,
    collections: &[String],
    tags: &[String],
) -> serde_json::Value {
    serde_json::json!({
        "itemType": "webpage",
        "title": url,
        "url": url,
        "accessDate": "",
        "collections": collections,
        "tags": tags.iter().map(|tag| serde_json::json!({ "tag": tag })).collect::<Vec<_>>(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use zot_remote::{ArxivWork, CrossRefWork};

    fn sample_crossref() -> CrossRefWork {
        CrossRefWork {
            record_type: "journal-article".to_string(),
            title: Some("Sample Title".to_string()),
            creators: vec![CreatorName {
                first_name: "Ashish".to_string(),
                last_name: "Vaswani".to_string(),
                creator_type: "author".to_string(),
            }],
            date: Some("2017-06-12".to_string()),
            doi: "10.1000/test".to_string(),
            url: Some("https://example.com/work".to_string()),
            volume: Some("30".to_string()),
            issue: Some("1".to_string()),
            pages: Some("1-11".to_string()),
            publisher: Some("ACM".to_string()),
            issn: Some("1234-5678".to_string()),
            publication_title: Some("NeurIPS".to_string()),
            abstract_note: Some("Abstract text".to_string()),
            relations: Vec::new(),
            alternative_ids: Vec::new(),
            links: Vec::new(),
        }
    }

    fn sample_arxiv() -> ArxivWork {
        ArxivWork {
            arxiv_id: "2301.00774".to_string(),
            title: "Sample Preprint".to_string(),
            abstract_note: Some("Abstract text".to_string()),
            date: Some("2023-01-02".to_string()),
            creators: vec![CreatorName {
                first_name: "Jane".to_string(),
                last_name: "Doe".to_string(),
                creator_type: "author".to_string(),
            }],
            abs_url: "https://arxiv.org/abs/2301.00774".to_string(),
            pdf_url: "https://arxiv.org/pdf/2301.00774.pdf".to_string(),
        }
    }

    #[test]
    fn crossref_type_maps_known_and_falls_back_for_unknown() {
        assert_eq!(crossref_type_to_zotero("journal-article"), "journalArticle");
        assert_eq!(crossref_type_to_zotero("book-chapter"), "bookSection");
        assert_eq!(crossref_type_to_zotero("posted-content"), "preprint");
        assert_eq!(crossref_type_to_zotero("totally-unknown"), "document");
    }

    #[test]
    fn crossref_payload_maps_fields() {
        let work = sample_crossref();
        let payload =
            build_crossref_item_payload(&work, &["COLL1".to_string()], &["reading".to_string()]);
        assert_eq!(payload["itemType"], "journalArticle");
        assert_eq!(payload["title"], "Sample Title");
        assert_eq!(payload["DOI"], "10.1000/test");
        assert_eq!(payload["publicationTitle"], "NeurIPS");
        assert_eq!(payload["collections"][0], "COLL1");
        assert_eq!(payload["tags"][0]["tag"], "reading");
        assert_eq!(payload["creators"][0]["lastName"], "Vaswani");
    }

    #[test]
    fn crossref_payload_title_falls_back_to_doi() {
        let mut work = sample_crossref();
        work.title = None;
        let payload = build_crossref_item_payload(&work, &[], &[]);
        assert_eq!(payload["title"], "10.1000/test");
    }

    #[test]
    fn arxiv_payload_uses_preprint_type_and_extra() {
        let payload = build_arxiv_item_payload(&sample_arxiv(), &[], &[]);
        assert_eq!(payload["itemType"], "preprint");
        assert_eq!(payload["extra"], "arXiv:2301.00774");
        assert_eq!(payload["url"], "https://arxiv.org/abs/2301.00774");
    }

    #[test]
    fn webpage_payload_uses_url_for_title_and_url() {
        let payload =
            build_webpage_item_payload("https://example.com/page", &[], &["news".to_string()]);
        assert_eq!(payload["itemType"], "webpage");
        assert_eq!(payload["title"], "https://example.com/page");
        assert_eq!(payload["url"], "https://example.com/page");
        assert_eq!(payload["accessDate"], "");
        assert_eq!(payload["tags"][0]["tag"], "news");
    }

    #[test]
    fn plan_add_url_classifies_doi_arxiv_and_webpage() {
        assert_eq!(
            plan_add_url("10.1000/test"),
            AddUrlPlan::Doi("10.1000/test".to_string())
        );
        assert_eq!(
            plan_add_url("https://arxiv.org/abs/2301.00774v2"),
            AddUrlPlan::Arxiv("2301.00774v2".to_string())
        );
        assert_eq!(
            plan_add_url("https://example.com/page"),
            AddUrlPlan::Webpage
        );
    }

    #[test]
    fn arxiv_pdf_attachment_skips_when_mode_is_none() {
        assert!(arxiv_pdf_attachment(&sample_arxiv(), "2301.00774", AttachModeArg::None).is_none());
    }

    #[test]
    fn arxiv_pdf_attachment_builds_target_and_sanitizes_filename() {
        let work = sample_arxiv();
        let versioned = arxiv_pdf_attachment(&work, "2301.00774v2", AttachModeArg::Auto)
            .expect("attachment for versioned id");
        assert_eq!(versioned.url, work.pdf_url);
        assert_eq!(versioned.filename, "arxiv_2301.00774v2.pdf");

        let legacy = arxiv_pdf_attachment(&work, "cond-mat/0102536", AttachModeArg::Auto)
            .expect("attachment for legacy id");
        assert_eq!(legacy.filename, "arxiv_cond-mat_0102536.pdf");
    }
}
