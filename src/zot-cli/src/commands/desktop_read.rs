//! Route item get, children, pdf, library search, and citation-key lookup.
//!
//! One command stays on one source. A desktop hit does not open the SQLite
//! snapshot. An object 404, identity conflict, or file error does not fall
//! through to the other source.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use zot_core::{
    AppConfig, CitationKeyMatch, Creator, Item, LibraryScope, SearchItemDirection, SearchItemSort,
    ZotError, ZotResult, canonicalize_or_original, extra_line_has_citation_key,
};
use zot_desktop::{
    DesktopRead, DesktopScanLimits, DesktopSearchQuery, LocalApiClient, LocalApiProbe,
};
use zot_local::{LocalLibrary, PdfCache, SearchOptions, SortDirection, SortField};
use zot_remote::{BetterBibTexClient, BetterBibTexSearchItem};

use crate::context::AppContext;
use crate::format::{EnvelopeMetaSeed, print_item, print_items};
use crate::output::CommandOutput;
use crate::util::{item_not_found, parse_page_range, require_item, require_item_pdf, run_pdf};

pub(crate) struct ReadIo {
    explicit: bool,
    running_zotero: bool,
    data_dir: Option<PathBuf>,
    binding_path: PathBuf,
    scope: LibraryScope,
    api: Option<LocalApiClient>,
    bbt: Option<BetterBibTexClient>,
    opens: Arc<AtomicUsize>,
    limits: DesktopScanLimits,
}

pub(crate) fn identity_explicit(
    profile: Option<&str>,
    configured_data_dir: &str,
    env_override: bool,
) -> bool {
    profile.is_some() || env_override || !configured_data_dir.trim().is_empty()
}

impl ReadIo {
    pub(crate) fn from_context(ctx: &AppContext, running_zotero: bool) -> Self {
        Self {
            explicit: identity_explicit(
                ctx.profile.as_deref(),
                &ctx.config.zotero.data_dir,
                std::env::var_os("ZOT_DATA_DIR").is_some(),
            ),
            running_zotero,
            data_dir: Some(zot_core::get_data_dir(&ctx.config)),
            binding_path: AppConfig::state_dir().join("desktop-read-bindings.json"),
            scope: ctx.scope.clone(),
            api: ctx.local_api().ok(),
            bbt: Some(BetterBibTexClient::new(ctx.http())),
            opens: Arc::new(AtomicUsize::new(0)),
            limits: DesktopScanLimits::default(),
        }
    }

    fn opens_handle(&self) -> Arc<AtomicUsize> {
        Arc::clone(&self.opens)
    }
}

enum Deny {
    Mismatch,
    Unbound,
    Unsupported,
}

struct DesktopChoice {
    server_id: Option<String>,
    source: &'static str,
}

enum Plan {
    Use(DesktopChoice),
    Mismatch,
    Fallback(Deny),
    Down(ZotError),
}

enum Route {
    Desktop(DesktopChoice),
    Snapshot(Box<LocalLibrary>),
}

pub(crate) async fn read_item(ctx: &AppContext, io: &ReadIo, key: &str) -> Result<CommandOutput> {
    let (item, notes, attachments, source) = match io.route(false).await? {
        Route::Desktop(choice) => {
            let view = io
                .reader(&choice)?
                .item_view(key)
                .await?
                .ok_or_else(|| item_not_found(key))?;
            (view.item, view.notes, view.attachments, choice.source)
        }
        Route::Snapshot(library) => {
            let item = require_item(library.as_ref(), key)?;
            let notes = library.get_notes(key)?;
            let attachments = library.get_attachments(key)?;
            (item, notes, attachments, "snapshot")
        }
    };
    let payload = serde_json::json!({
        "item": item,
        "notes": notes,
        "attachments": attachments,
    });
    CommandOutput::new(ctx, payload, source_seed(source), move |_| {
        print_item(&item, &notes, &attachments)
    })
}

pub(crate) async fn read_children(
    ctx: &AppContext,
    io: &ReadIo,
    keys: &[String],
) -> Result<CommandOutput> {
    let (children, source) = match io.route(false).await? {
        Route::Desktop(choice) => {
            let source = choice.source;
            let children = io.reader(&choice)?.children_for_keys(keys).await?;
            (children, source)
        }
        Route::Snapshot(library) => (library.get_items_children(keys)?, "snapshot"),
    };
    CommandOutput::new(ctx, children, source_seed(source), |children| {
        for (key, values) in children {
            println!("{key}");
            for value in values {
                println!("  - {} [{}]", value.key(), value.kind_label());
            }
        }
    })
}

pub(crate) async fn read_pdf(
    ctx: &AppContext,
    io: &ReadIo,
    key: &str,
    pages: Option<&str>,
    annotations: bool,
) -> Result<CommandOutput> {
    let pdf_path = match io.route(false).await? {
        Route::Desktop(choice) => {
            let source = choice.source;
            let path = io.reader(&choice)?.local_pdf_path(key).await?;
            return render_pdf(ctx, source, path, pages, annotations).await;
        }
        Route::Snapshot(library) => {
            let attachment = require_item_pdf(library.as_ref(), key)?;
            library.pdf_path(&attachment)
        }
    };
    render_pdf(ctx, "snapshot", pdf_path, pages, annotations).await
}

pub(crate) async fn read_search(
    ctx: &AppContext,
    io: &ReadIo,
    mut options: SearchOptions,
) -> Result<CommandOutput> {
    options.title = blank_to_none(options.title);
    options.journal = blank_to_none(options.journal);
    options.topic = blank_to_none(options.topic);
    let include_trashed = !options.exclude_trashed;
    let snapshot_first = !options.query.trim().is_empty();
    let (result, source) = match io.route(snapshot_first).await? {
        Route::Desktop(choice) => {
            let source = choice.source;
            let result = io.reader(&choice)?.search(&desktop_query(&options)).await?;
            (result, source)
        }
        Route::Snapshot(library) => (library.search(options)?, "snapshot"),
    };
    let seed = Some(EnvelopeMetaSeed {
        count: Some(result.items.len()),
        total: Some(result.total),
        trash_policy: Some(trash_policy(include_trashed)),
        fulltext_index: result.fulltext_index,
        read_source: Some(source.to_string()),
    });
    CommandOutput::new(ctx, result.items, seed, |items| print_items(items))
}

pub(crate) async fn read_citekey(
    ctx: &AppContext,
    io: &ReadIo,
    citekey: &str,
) -> Result<CommandOutput> {
    let (found, source) = match io.route(false).await? {
        Route::Desktop(choice) => {
            let source = choice.source;
            let reader = io.reader(&choice)?;
            let matches = reader.citation_key_matches(citekey).await?;
            let found = if matches.len() > 1 {
                return Err(ambiguous(citekey, &matches).into());
            } else if let Some(found) = matches.into_iter().next() {
                found
            } else {
                confirm_desktop(io, &reader, citekey).await?
            };
            (found, source)
        }
        Route::Snapshot(library) => {
            let found = match library.search_by_citation_key(citekey)? {
                Some(found) => found,
                None => confirm_snapshot(io, &library, citekey).await?,
            };
            (found, "snapshot")
        }
    };
    CommandOutput::new(ctx, found, source_seed(source), |found| {
        print_items(std::slice::from_ref(&found.item))
    })
}

async fn confirm_desktop(
    io: &ReadIo,
    reader: &DesktopRead,
    citekey: &str,
) -> ZotResult<CitationKeyMatch> {
    let candidates = plugin_candidates(io.bbt.as_ref(), citekey).await?;
    if candidates.is_empty() {
        return Err(not_found(citekey));
    }
    let mut verified = Vec::new();
    for candidate in candidates {
        let Some(view) = reader.item_view(&candidate.item_key).await? else {
            continue;
        };
        if item_carries_citekey(&view.item, citekey) {
            verified.push(CitationKeyMatch {
                citekey: citekey.to_string(),
                source: "better-bibtex".to_string(),
                item: view.item,
            });
        }
    }
    finish_verified(citekey, verified)
}

async fn confirm_snapshot(
    io: &ReadIo,
    library: &LocalLibrary,
    citekey: &str,
) -> ZotResult<CitationKeyMatch> {
    let candidates = plugin_candidates(io.bbt.as_ref(), citekey).await?;
    if candidates.is_empty() {
        return Err(not_found(citekey));
    }
    let mut verified = Vec::new();
    for candidate in candidates {
        let Some(item) = library.get_item(&candidate.item_key)? else {
            continue;
        };
        if item_carries_citekey(&item, citekey) {
            verified.push(CitationKeyMatch {
                citekey: citekey.to_string(),
                source: "better-bibtex".to_string(),
                item,
            });
        }
    }
    finish_verified(citekey, verified)
}

fn finish_verified(citekey: &str, verified: Vec<CitationKeyMatch>) -> ZotResult<CitationKeyMatch> {
    if verified.len() > 1 {
        return Err(ambiguous(citekey, &verified));
    }
    verified
        .into_iter()
        .next()
        .ok_or_else(|| unverified(citekey))
}

async fn plugin_candidates(
    bbt: Option<&BetterBibTexClient>,
    citekey: &str,
) -> ZotResult<Vec<BetterBibTexSearchItem>> {
    let Some(client) = bbt else {
        return Err(source_limited(citekey));
    };
    if !client.probe().await {
        return Err(source_limited(citekey));
    }
    let found = client.search(citekey).await?;
    Ok(found
        .into_iter()
        .filter(|candidate| candidate.citekey == citekey)
        .collect())
}

fn item_carries_citekey(item: &Item, citekey: &str) -> bool {
    item.extra
        .get("citationKey")
        .is_some_and(|value| value == citekey)
        || item
            .extra
            .get("extra")
            .is_some_and(|extra| extra_line_has_citation_key(extra, citekey))
}

impl ReadIo {
    async fn route(&self, snapshot_first: bool) -> ZotResult<Route> {
        if snapshot_first {
            match self.open_snapshot().await {
                Ok(library) => return Ok(Route::Snapshot(Box::new(library))),
                Err(snapshot_err) => {
                    return match self.plan().await {
                        Plan::Use(choice) => Ok(Route::Desktop(choice)),
                        Plan::Mismatch => {
                            Err(unavailable(deny_error(Deny::Mismatch), snapshot_err))
                        }
                        Plan::Fallback(deny) => Err(unavailable(deny_error(deny), snapshot_err)),
                        Plan::Down(desktop_err) => Err(unavailable(desktop_err, snapshot_err)),
                    };
                }
            }
        }
        match self.plan().await {
            Plan::Use(choice) => Ok(Route::Desktop(choice)),
            Plan::Mismatch => Err(deny_error(Deny::Mismatch)),
            Plan::Fallback(deny) => match self.open_snapshot().await {
                Ok(library) => Ok(Route::Snapshot(Box::new(library))),
                Err(snapshot_err) => Err(unavailable(deny_error(deny), snapshot_err)),
            },
            Plan::Down(desktop_err) => match self.open_snapshot().await {
                Ok(library) => Ok(Route::Snapshot(Box::new(library))),
                Err(snapshot_err) => Err(unavailable(desktop_err, snapshot_err)),
            },
        }
    }

    async fn plan(&self) -> Plan {
        let probe = match self.probe().await {
            Ok(probe) => probe,
            Err(error) => return Plan::Down(error),
        };
        match self.choose(&probe) {
            Ok(choice) => Plan::Use(choice),
            Err(Deny::Mismatch) => Plan::Mismatch,
            Err(deny) => Plan::Fallback(deny),
        }
    }

    async fn probe(&self) -> ZotResult<LocalApiProbe> {
        let Some(client) = &self.api else {
            return Err(local_api(
                "local-api-unavailable",
                "Local API client is unavailable",
                Some("Start Zotero and enable local communication."),
                None,
            ));
        };
        client.probe_for_read().await
    }

    fn choose(&self, probe: &LocalApiProbe) -> Result<DesktopChoice, Deny> {
        let server_id = probe
            .server_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        if !self.explicit {
            if probe.supported {
                return Ok(DesktopChoice {
                    server_id,
                    source: "desktop",
                });
            }
            return if self.running_zotero {
                Ok(DesktopChoice {
                    server_id,
                    source: "desktop-unbound",
                })
            } else {
                Err(Deny::Unsupported)
            };
        }
        let stored = self.stored_server_id();
        if let (Some(stored_id), Some(header)) = (stored.clone(), server_id.clone()) {
            if stored_id == header && probe.supported {
                return Ok(DesktopChoice {
                    server_id: Some(header),
                    source: "desktop",
                });
            }
            if stored_id != header {
                return if self.running_zotero {
                    Ok(DesktopChoice {
                        server_id: Some(header),
                        source: "desktop-unbound",
                    })
                } else {
                    Err(Deny::Mismatch)
                };
            }
            return if self.running_zotero {
                Ok(DesktopChoice {
                    server_id: Some(header),
                    source: "desktop-unbound",
                })
            } else {
                Err(Deny::Unsupported)
            };
        }
        if stored.is_some() && server_id.is_none() {
            return if self.running_zotero {
                Ok(DesktopChoice {
                    server_id: None,
                    source: "desktop-unbound",
                })
            } else {
                Err(Deny::Mismatch)
            };
        }
        if self.running_zotero {
            return Ok(DesktopChoice {
                server_id,
                source: "desktop-unbound",
            });
        }
        if probe.supported {
            Err(Deny::Unbound)
        } else {
            Err(Deny::Unsupported)
        }
    }

    fn stored_server_id(&self) -> Option<String> {
        let wanted = canonicalize_or_original(self.data_dir.as_ref()?);
        let text = std::fs::read_to_string(&self.binding_path).ok()?;
        let file: BindingFile = serde_json::from_str(&text).ok()?;
        file.bindings.into_iter().find_map(|entry| {
            let same = canonicalize_or_original(Path::new(&entry.data_dir)) == wanted;
            (same && !entry.server_id.trim().is_empty()).then_some(entry.server_id)
        })
    }

    fn reader(&self, choice: &DesktopChoice) -> ZotResult<DesktopRead> {
        let client = self.api.clone().ok_or_else(|| {
            local_api(
                "local-api-unavailable",
                "Local API client is unavailable",
                None,
                None,
            )
        })?;
        Ok(
            DesktopRead::new(client, choice.server_id.clone(), self.scope.clone())
                .with_limits(self.limits),
        )
    }

    async fn open_snapshot(&self) -> ZotResult<LocalLibrary> {
        let opens = self.opens_handle();
        let data_dir = self.data_dir.clone();
        let scope = self.scope.clone();
        let binding_path = self.binding_path.clone();
        tokio::task::spawn_blocking(move || {
            opens.fetch_add(1, Ordering::SeqCst);
            let Some(data_dir) = data_dir else {
                return Err(ZotError::Database {
                    code: "db-not-found".to_string(),
                    message: "No Zotero data directory is configured for a snapshot read"
                        .to_string(),
                    hint: Some(
                        "Set the data directory, or pass --running-zotero to read the running Zotero."
                            .to_string(),
                    ),
                });
            };
            let library = LocalLibrary::open(&data_dir, scope)?;
            if let Ok(Some(server_id)) = library.local_api_server_id() {
                let _ = save_binding(&binding_path, &data_dir, &server_id);
            }
            Ok(library)
        })
        .await
        .map_err(|join| {
            ZotError::InvalidInput {
                code: "local-task-join".to_string(),
                message: format!("Local database worker failed: {join}"),
                hint: Some("Retry the command; report the failure if it persists".to_string()),
            }
        })?
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct BindingFile {
    #[serde(default)]
    bindings: Vec<BindingEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BindingEntry {
    data_dir: String,
    server_id: String,
}

fn save_binding(path: &Path, data_dir: &Path, server_id: &str) -> ZotResult<()> {
    let server_id = server_id.trim();
    if server_id.is_empty() {
        return Ok(());
    }
    let key = canonicalize_or_original(data_dir).display().to_string();
    let mut file = std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<BindingFile>(&text).ok())
        .unwrap_or(BindingFile {
            bindings: Vec::new(),
        });
    file.bindings.retain(|entry| {
        canonicalize_or_original(Path::new(&entry.data_dir))
            .display()
            .to_string()
            != key
    });
    file.bindings.push(BindingEntry {
        data_dir: key,
        server_id: server_id.to_string(),
    });
    let payload = serde_json::to_vec_pretty(&file).map_err(|err| ZotError::InvalidInput {
        code: "desktop-read-binding".to_string(),
        message: err.to_string(),
        hint: None,
    })?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| ZotError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let temporary = path.with_extension("json.tmp");
    std::fs::write(&temporary, payload).map_err(|source| ZotError::Io {
        path: temporary.clone(),
        source,
    })?;
    // Windows rename does not replace an existing destination.
    #[cfg(target_os = "windows")]
    if path.exists() {
        std::fs::remove_file(path).map_err(|source| ZotError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    }
    if let Err(source) = std::fs::rename(&temporary, path) {
        let _ = std::fs::remove_file(&temporary);
        return Err(ZotError::Io {
            path: path.to_path_buf(),
            source,
        });
    }
    Ok(())
}

fn desktop_query(options: &SearchOptions) -> DesktopSearchQuery {
    DesktopSearchQuery {
        query: options.query.clone(),
        collection: options.collection.clone(),
        item_type: options.item_type.clone(),
        tag: options.tag.clone(),
        creator: options.creator.clone(),
        year: options.year.clone(),
        title: options.title.clone(),
        journal: options.journal.clone(),
        topic: options.topic.clone(),
        sort: options.sort.map(|sort| match sort {
            SortField::DateAdded => SearchItemSort::DateAdded,
            SortField::DateModified => SearchItemSort::DateModified,
            SortField::Title => SearchItemSort::Title,
            SortField::Creator => SearchItemSort::Creator,
        }),
        direction: match options.direction {
            SortDirection::Asc => SearchItemDirection::Asc,
            SortDirection::Desc => SearchItemDirection::Desc,
        },
        limit: options.limit,
        offset: options.offset,
        exclude_trashed: options.exclude_trashed,
    }
}

fn blank_to_none(value: Option<String>) -> Option<String> {
    value.and_then(|text| {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

async fn render_pdf(
    ctx: &AppContext,
    source: &str,
    pdf_path: PathBuf,
    pages: Option<&str>,
    annotations: bool,
) -> Result<CommandOutput> {
    let backend = ctx.pdf_backend();
    let cache = PdfCache::new(None)?;
    if annotations {
        let annotations = {
            let pdf_path = pdf_path.clone();
            run_pdf(move || backend.extract_annotations(&pdf_path)).await?
        };
        return CommandOutput::new(ctx, annotations, source_seed(source), |annotations| {
            for annotation in annotations {
                println!(
                    "[p.{}] {} {}",
                    annotation.page, annotation.annotation_type, annotation.content
                );
            }
        });
    }
    let page_range = parse_page_range(pages)?;
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
    CommandOutput::new(ctx, payload, source_seed(source), move |_| {
        println!("{text}")
    })
}

fn source_seed(source: &str) -> Option<EnvelopeMetaSeed> {
    Some(EnvelopeMetaSeed {
        read_source: Some(source.to_string()),
        ..EnvelopeMetaSeed::default()
    })
}

fn trash_policy(include_trashed: bool) -> String {
    if include_trashed {
        "included"
    } else {
        "excluded"
    }
    .to_string()
}

fn unavailable(desktop: ZotError, snapshot: ZotError) -> ZotError {
    ZotError::Database {
        code: "library-read-unavailable".to_string(),
        message: format!(
            "Desktop read failed: {}. Snapshot read failed: {}.",
            desktop.payload().message,
            snapshot.payload().message
        ),
        hint: Some(
            "Start Zotero and enable local communication, or close it so a snapshot can be read. Pass --running-zotero when the running library is not proved to match the configured data directory."
                .to_string(),
        ),
    }
}

fn deny_error(deny: Deny) -> ZotError {
    let (code, message, hint) = match deny {
        Deny::Mismatch => (
            "desktop-library-mismatch",
            "The running Zotero instance does not match the configured library",
            "Pass --running-zotero to read the running Zotero, or close Zotero and use the configured snapshot.",
        ),
        Deny::Unbound => (
            "desktop-library-unbound",
            "The running Zotero is not proved to match the configured library",
            "Pass --running-zotero to read the running Zotero, or close Zotero and retry the snapshot.",
        ),
        Deny::Unsupported => (
            "desktop-library-unbound",
            "This Zotero has no instance id for automatic binding",
            "Pass --running-zotero to read the running Zotero, or close Zotero and retry the snapshot.",
        ),
    };
    ZotError::InvalidInput {
        code: code.to_string(),
        message: message.to_string(),
        hint: Some(hint.to_string()),
    }
}

fn ambiguous(citekey: &str, matches: &[CitationKeyMatch]) -> ZotError {
    let listed = matches
        .iter()
        .map(|found| {
            let author = found
                .item
                .creators
                .first()
                .map(Creator::full_name)
                .unwrap_or_default();
            let year = found.item.date.as_deref().unwrap_or("");
            format!(
                "{} | {author} | {year} | {}",
                found.item.title, found.item.key
            )
        })
        .collect::<Vec<_>>()
        .join("; ");
    ZotError::InvalidInput {
        code: "citation-key-ambiguous".to_string(),
        message: format!("Citation key '{citekey}' matches multiple items: {listed}"),
        hint: Some("Choose one item key with 'zot item get <key>'".to_string()),
    }
}

fn not_found(citekey: &str) -> ZotError {
    ZotError::InvalidInput {
        code: "citation-key-not-found".to_string(),
        message: format!("Citation key '{citekey}' not found"),
        hint: None,
    }
}

fn source_limited(citekey: &str) -> ZotError {
    ZotError::InvalidInput {
        code: "citation-key-source-limited".to_string(),
        message: format!(
            "Citation key '{citekey}' is not in stored fields, and Better BibTeX cannot be asked"
        ),
        hint: Some(
            "Start Zotero with Better BibTeX, or store the key in citationKey or an Extra line."
                .to_string(),
        ),
    }
}

fn unverified(citekey: &str) -> ZotError {
    ZotError::InvalidInput {
        code: "citation-key-unverified".to_string(),
        message: format!(
            "Citation key '{citekey}' was suggested by Better BibTeX, but this library's item does not carry that key"
        ),
        hint: Some(
            "Better BibTeX libraryID is not a group id. Stay on the selected library and confirm the stored citation key."
                .to_string(),
        ),
    }
}

fn local_api(code: &str, message: &str, hint: Option<&str>, status: Option<u16>) -> ZotError {
    ZotError::LocalApi {
        code: code.to_string(),
        message: message.to_string(),
        hint: hint.map(str::to_string),
        status,
    }
}

#[cfg(test)]
#[path = "desktop_read_tests.rs"]
mod tests;
