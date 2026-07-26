use std::env;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::UNIX_EPOCH;

use flate2::read::GzDecoder;
use pdfium_render::prelude::*;
use reqwest::blocking::Client;
use rusqlite::{Connection, OptionalExtension, params};
use sha2::{Digest, Sha256};
use tar::Archive;
use tempfile::NamedTempFile;
use zot_core::{AnnotationSnippet, PdfOutlineEntry, ZotError, ZotResult};

#[derive(Debug, Clone, PartialEq)]
pub struct PdfMatchPosition {
    pub page_index: usize,
    pub page_label: String,
    pub matched_text: String,
    pub rects: Vec<[f32; 4]>,
    pub sort_index: String,
    /// Total number of times the queried text appears on the requested page,
    /// or `None` when the backend could not enumerate matches. Useful for
    /// reporting "there are still N more occurrences" hints.
    pub total_matches: Option<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PdfAreaPosition {
    pub page_index: usize,
    pub page_label: String,
    pub rects: Vec<[f32; 4]>,
    pub sort_index: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdfiumAvailability {
    pub available: bool,
    pub cached: bool,
    pub auto_download_supported: bool,
    pub note: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PdfiumDownloadTarget {
    archive_name: &'static str,
    library_path_in_archive: &'static str,
    archive_sha256: &'static str,
    library_sha256: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PdfiumLoadMode {
    ProbeOnly,
    AllowDownload,
}

const PDFIUM_VERSION: &str = "7543";
const PDFIUM_BASE_URL: &str =
    "https://github.com/bblanchon/pdfium-binaries/releases/download/chromium%2F7543";
const MAX_PDFIUM_ARCHIVE_BYTES: u64 = 32 * 1024 * 1024;
const MAX_PDFIUM_LIBRARY_BYTES: u64 = 128 * 1024 * 1024;
const PDFIUM_INSTALL_LOCK_FILE: &str = ".install.lock";
const ZOT_PDFIUM_LIB_PATH: &str = "ZOT_PDFIUM_LIB_PATH";
const ZOT_PDFIUM_CACHE_DIR: &str = "ZOT_PDFIUM_CACHE_DIR";
const PDFIUM_LIB_PATH: &str = "PDFIUM_LIB_PATH";

pub trait PdfBackend {
    fn availability_hint(&self) -> ZotResult<()>;
    fn extract_text(
        &self,
        pdf_path: &Path,
        page_range: Option<(usize, usize)>,
    ) -> ZotResult<String>;
    fn extract_annotations(&self, pdf_path: &Path) -> ZotResult<Vec<AnnotationSnippet>>;
    fn extract_outline(&self, pdf_path: &Path) -> ZotResult<Vec<PdfOutlineEntry>>;
    fn find_text_position(
        &self,
        pdf_path: &Path,
        page: usize,
        text: &str,
        occurrence: usize,
    ) -> ZotResult<Option<PdfMatchPosition>>;
    fn build_area_position(
        &self,
        pdf_path: &Path,
        page: usize,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    ) -> ZotResult<PdfAreaPosition>;
    fn extract_doi(&self, pdf_path: &Path) -> ZotResult<Option<String>> {
        let text = self.extract_text(pdf_path, Some((1, 2)))?;
        let re = regex::Regex::new(r"10\.\d{4,9}/[^\s]+").map_err(|err| ZotError::Pdf {
            code: "doi-regex".to_string(),
            message: err.to_string(),
            hint: None,
        })?;
        Ok(re.find(&text).map(|matched| {
            matched
                .as_str()
                .trim_end_matches(&['.', ',', ';', ')', ']', '}', '"', '\''][..])
                .to_string()
        }))
    }
}

/// Forwarding impl so an `Arc<dyn PdfBackend + Send + Sync>` (as held by the
/// CLI's `AppContext`) satisfies generic `B: PdfBackend` bounds. Every method
/// forwards explicitly — including defaulted `extract_doi` — so a backend that
/// overrides a default is never bypassed through the `Arc`.
impl<T: PdfBackend + ?Sized> PdfBackend for Arc<T> {
    fn availability_hint(&self) -> ZotResult<()> {
        (**self).availability_hint()
    }
    fn extract_text(
        &self,
        pdf_path: &Path,
        page_range: Option<(usize, usize)>,
    ) -> ZotResult<String> {
        (**self).extract_text(pdf_path, page_range)
    }
    fn extract_annotations(&self, pdf_path: &Path) -> ZotResult<Vec<AnnotationSnippet>> {
        (**self).extract_annotations(pdf_path)
    }
    fn extract_outline(&self, pdf_path: &Path) -> ZotResult<Vec<PdfOutlineEntry>> {
        (**self).extract_outline(pdf_path)
    }
    fn find_text_position(
        &self,
        pdf_path: &Path,
        page: usize,
        text: &str,
        occurrence: usize,
    ) -> ZotResult<Option<PdfMatchPosition>> {
        (**self).find_text_position(pdf_path, page, text, occurrence)
    }
    fn build_area_position(
        &self,
        pdf_path: &Path,
        page: usize,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    ) -> ZotResult<PdfAreaPosition> {
        (**self).build_area_position(pdf_path, page, x, y, width, height)
    }
    fn extract_doi(&self, pdf_path: &Path) -> ZotResult<Option<String>> {
        (**self).extract_doi(pdf_path)
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct PdfiumBackend;

impl PdfiumBackend {
    pub fn status(&self) -> PdfiumAvailability {
        let auto_download_supported = current_download_target().is_some();
        let cached = managed_cache_library_path().is_some_and(|path| path.exists());
        let available = self.pdfium(PdfiumLoadMode::ProbeOnly).is_ok();
        let note = if available {
            "Pdfium is ready for local PDF reads.".to_string()
        } else if auto_download_supported {
            if cached {
                "Managed Pdfium cache is present but not loadable; local PDF reads will retry a managed download on first use."
                    .to_string()
            } else {
                "Pdfium will auto-download on the first local PDF read.".to_string()
            }
        } else {
            format!(
                "Set {ZOT_PDFIUM_LIB_PATH} or {PDFIUM_LIB_PATH} to a compatible Pdfium library."
            )
        };
        PdfiumAvailability {
            available,
            cached,
            auto_download_supported,
            note,
        }
    }

    fn pdfium(&self, mode: PdfiumLoadMode) -> ZotResult<Pdfium> {
        let library_name = pdfium_library_name();
        let mut last_error = None;

        for candidate in candidate_library_paths(&library_name) {
            if !candidate.exists() {
                continue;
            }
            match bind_pdfium_from_path(&candidate) {
                Ok(pdfium) => return Ok(pdfium),
                Err(error) => {
                    if last_error.is_none() {
                        last_error = Some(error);
                    }
                }
            }
        }

        if matches!(mode, PdfiumLoadMode::AllowDownload)
            && let Some(target) = current_download_target()
        {
            let path = download_pdfium_library(target, &library_name)?;
            return bind_pdfium_from_path(&path);
        }

        Err(last_error.unwrap_or_else(pdfium_manual_setup_error))
    }
}

impl PdfBackend for PdfiumBackend {
    fn availability_hint(&self) -> ZotResult<()> {
        let _ = self.pdfium(PdfiumLoadMode::ProbeOnly)?;
        Ok(())
    }

    fn extract_text(
        &self,
        pdf_path: &Path,
        page_range: Option<(usize, usize)>,
    ) -> ZotResult<String> {
        let pdfium = self.pdfium(PdfiumLoadMode::AllowDownload)?;
        let document = pdfium
            .load_pdf_from_file(pdf_path, None)
            .map_err(|err| ZotError::Pdf {
                code: "pdf-open".to_string(),
                message: err.to_string(),
                hint: Some(format!("Failed to open PDF: {}", pdf_path.display())),
            })?;
        let page_count = document.pages().len() as usize;
        let (start, end) = page_range.unwrap_or((1, page_count));
        if start == 0 || end < start || start > page_count {
            return Err(ZotError::Pdf {
                code: "invalid-page-range".to_string(),
                message: format!(
                    "Invalid page range {start}-{end} for document with {page_count} pages"
                ),
                hint: None,
            });
        }

        let mut pages_text = Vec::new();
        for page_index in (start - 1)..usize::min(end, page_count) {
            let page = document
                .pages()
                .get(page_index as i32)
                .map_err(|err| ZotError::Pdf {
                    code: "pdf-page".to_string(),
                    message: err.to_string(),
                    hint: None,
                })?;
            let text = page.text().map_err(|err| ZotError::Pdf {
                code: "pdf-text".to_string(),
                message: err.to_string(),
                hint: None,
            })?;
            pages_text.push(text.all());
        }
        Ok(pages_text.join("\n"))
    }

    fn extract_annotations(&self, pdf_path: &Path) -> ZotResult<Vec<AnnotationSnippet>> {
        use pdfium_render::prelude::PdfPageAnnotationCommon;

        let pdfium = self.pdfium(PdfiumLoadMode::AllowDownload)?;
        let document = pdfium
            .load_pdf_from_file(pdf_path, None)
            .map_err(|err| ZotError::Pdf {
                code: "pdf-open".to_string(),
                message: err.to_string(),
                hint: Some(format!("Failed to open PDF: {}", pdf_path.display())),
            })?;
        let mut result = Vec::new();
        for (index, page) in document.pages().iter().enumerate() {
            let page_text = page.text().ok();
            for annotation in page.annotations().iter() {
                let bounds = annotation.bounds().ok();
                let quote = match (&page_text, bounds) {
                    (Some(text), Some(bounds)) => {
                        let extracted = text.inside_rect(bounds);
                        (!extracted.trim().is_empty()).then_some(extracted)
                    }
                    _ => None,
                };
                result.push(AnnotationSnippet {
                    annotation_type: format!("{:?}", annotation.annotation_type()),
                    page: index + 1,
                    content: annotation.contents().unwrap_or_default(),
                    quote,
                });
            }
        }
        Ok(result)
    }

    fn extract_outline(&self, pdf_path: &Path) -> ZotResult<Vec<PdfOutlineEntry>> {
        let pdfium = self.pdfium(PdfiumLoadMode::AllowDownload)?;
        let document = pdfium
            .load_pdf_from_file(pdf_path, None)
            .map_err(|err| ZotError::Pdf {
                code: "pdf-open".to_string(),
                message: err.to_string(),
                hint: Some(format!("Failed to open PDF: {}", pdf_path.display())),
            })?;
        let mut entries = Vec::new();
        for bookmark in document.bookmarks().iter() {
            // Walk the parent chain to determine the actual outline depth
            // instead of relying on dot-count heuristics in the title.
            // pdfium-render 0.9 omits the synthetic root from `iter()`, so
            // top-level bookmarks have one parent (the root) and resolve to
            // level = 1.
            let mut depth = 0_usize;
            let mut current = bookmark.parent();
            while let Some(parent) = current {
                depth += 1;
                current = parent.parent();
            }
            let level = depth.max(1);
            let title = bookmark.title().unwrap_or_default();
            let page = bookmark
                .destination()
                .and_then(|destination| destination.page_index().ok())
                .map(|page_index| (page_index + 1) as usize);
            entries.push(PdfOutlineEntry { level, title, page });
        }
        Ok(entries)
    }

    fn find_text_position(
        &self,
        pdf_path: &Path,
        page: usize,
        text: &str,
        occurrence: usize,
    ) -> ZotResult<Option<PdfMatchPosition>> {
        let occurrence = occurrence.max(1);
        let pdfium = self.pdfium(PdfiumLoadMode::AllowDownload)?;
        let document = pdfium
            .load_pdf_from_file(pdf_path, None)
            .map_err(|err| ZotError::Pdf {
                code: "pdf-open".to_string(),
                message: err.to_string(),
                hint: Some(format!("Failed to open PDF: {}", pdf_path.display())),
            })?;
        if page == 0 || page > document.pages().len() as usize {
            return Err(ZotError::Pdf {
                code: "invalid-page-range".to_string(),
                message: format!("Page {page} is out of bounds"),
                hint: None,
            });
        }
        let page_ref = document
            .pages()
            .get((page - 1) as i32)
            .map_err(|err| ZotError::Pdf {
                code: "pdf-page".to_string(),
                message: err.to_string(),
                hint: None,
            })?;
        let page_label = page_ref.label().unwrap_or("").to_string();
        let page_text = page_ref.text().map_err(|err| ZotError::Pdf {
            code: "pdf-text".to_string(),
            message: err.to_string(),
            hint: None,
        })?;
        let search = page_text
            .search(text, &PdfSearchOptions::new())
            .map_err(|err| ZotError::Pdf {
                code: "pdf-search".to_string(),
                message: err.to_string(),
                hint: None,
            })?;
        let mut selected: Option<(Vec<[f32; 4]>, String)> = None;
        let mut total = 0_usize;
        while let Some(result) = search.find_next() {
            total += 1;
            if total == occurrence {
                let rects = result
                    .iter()
                    .map(|segment| {
                        let bounds = segment.bounds();
                        [
                            bounds.left().value,
                            bounds.bottom().value,
                            bounds.right().value,
                            bounds.top().value,
                        ]
                    })
                    .collect::<Vec<_>>();
                let matched_text = result
                    .iter()
                    .map(|segment| segment.text())
                    .collect::<Vec<_>>()
                    .join(" ");
                selected = Some((rects, matched_text));
            }
        }
        let Some((rects, matched_text)) = selected else {
            return Ok(None);
        };
        let first = rects.first().copied().unwrap_or([0.0, 0.0, 0.0, 0.0]);
        let sort_index = format!(
            "{:05}|{:06}|{:05}",
            page.saturating_sub(1),
            first[1].round() as i64,
            first[0].round() as i64
        );
        Ok(Some(PdfMatchPosition {
            page_index: page.saturating_sub(1),
            page_label,
            matched_text,
            rects,
            sort_index,
            total_matches: Some(total),
        }))
    }

    fn build_area_position(
        &self,
        pdf_path: &Path,
        page: usize,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    ) -> ZotResult<PdfAreaPosition> {
        let pdfium = self.pdfium(PdfiumLoadMode::AllowDownload)?;
        let document = pdfium
            .load_pdf_from_file(pdf_path, None)
            .map_err(|err| ZotError::Pdf {
                code: "pdf-open".to_string(),
                message: err.to_string(),
                hint: Some(format!("Failed to open PDF: {}", pdf_path.display())),
            })?;
        if page == 0 || page > document.pages().len() as usize {
            return Err(ZotError::Pdf {
                code: "invalid-page-range".to_string(),
                message: format!("Page {page} is out of bounds"),
                hint: None,
            });
        }
        let page_ref = document
            .pages()
            .get((page - 1) as i32)
            .map_err(|err| ZotError::Pdf {
                code: "pdf-page".to_string(),
                message: err.to_string(),
                hint: None,
            })?;
        let page_size = page_ref.page_size();
        let page_width = page_size.width().value;
        let page_height = page_size.height().value;
        let left = x * page_width;
        let right = (x + width) * page_width;
        let top = page_height - (y * page_height);
        let bottom = page_height - ((y + height) * page_height);
        Ok(PdfAreaPosition {
            page_index: page.saturating_sub(1),
            page_label: page_ref.label().unwrap_or("").to_string(),
            rects: vec![[left, bottom, right, top]],
            sort_index: format!(
                "{:05}|{:06}|{:05}",
                page.saturating_sub(1),
                bottom.round() as i64,
                left.round() as i64
            ),
        })
    }
}

pub struct PdfCache {
    _path: PathBuf,
    conn: Connection,
}

impl PdfCache {
    pub fn new(path: Option<PathBuf>) -> ZotResult<Self> {
        let path = path.unwrap_or_else(default_cache_path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| ZotError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        let conn = Connection::open(&path).map_err(|err| ZotError::Database {
            code: "pdf-cache-open".to_string(),
            message: err.to_string(),
            hint: None,
        })?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS cache (cache_key TEXT PRIMARY KEY, content TEXT NOT NULL)",
            [],
        )
        .map_err(|err| ZotError::Database {
            code: "pdf-cache-schema".to_string(),
            message: err.to_string(),
            hint: None,
        })?;
        Ok(Self { _path: path, conn })
    }

    pub fn get(&self, pdf_path: &Path) -> ZotResult<Option<String>> {
        let cache_key = cache_key_for(pdf_path)?;
        self.conn
            .query_row(
                "SELECT content FROM cache WHERE cache_key = ?1",
                params![cache_key],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|err| ZotError::Database {
                code: "pdf-cache-get".to_string(),
                message: err.to_string(),
                hint: None,
            })
    }

    pub fn put(&self, pdf_path: &Path, content: &str) -> ZotResult<()> {
        let cache_key = cache_key_for(pdf_path)?;
        self.conn
            .execute(
                "INSERT OR REPLACE INTO cache (cache_key, content) VALUES (?1, ?2)",
                params![cache_key, content],
            )
            .map_err(|err| ZotError::Database {
                code: "pdf-cache-put".to_string(),
                message: err.to_string(),
                hint: None,
            })?;
        Ok(())
    }
}

fn default_cache_path() -> PathBuf {
    zot_core::AppConfig::state_dir()
        .join("cache")
        .join("pdf_cache.sqlite")
}

fn current_download_target() -> Option<PdfiumDownloadTarget> {
    let os = env::consts::OS;
    let arch = env::consts::ARCH;
    let target_env = if cfg!(target_env = "musl") {
        "musl"
    } else if cfg!(target_env = "gnu") {
        "gnu"
    } else {
        ""
    };
    download_target_for(os, arch, target_env)
}

fn download_target_for(os: &str, arch: &str, target_env: &str) -> Option<PdfiumDownloadTarget> {
    match (os, arch) {
        ("windows", "x86_64") => Some(PdfiumDownloadTarget {
            archive_name: "pdfium-win-x64.tgz",
            library_path_in_archive: "bin/pdfium.dll",
            archive_sha256: "0b08b606792a6cc593426efdefc6622611bce446d9e0270743846956ea1554ca",
            library_sha256: "6b963c2be9cacbaa0c0c7f4bf6d20d2fd16729ebdaa9989978b0f7b119c1c1cb",
        }),
        ("windows", "aarch64") => Some(PdfiumDownloadTarget {
            archive_name: "pdfium-win-arm64.tgz",
            library_path_in_archive: "bin/pdfium.dll",
            archive_sha256: "bb4a00113494e25bbee52d3d63b7f4ecf0de2d277b7de75ba9a1d5b987a74509",
            library_sha256: "368986d82c11a22e0c53728873899cf864dbd7b32a42214a660ac30fe8ba37f4",
        }),
        ("windows", "x86") => Some(PdfiumDownloadTarget {
            archive_name: "pdfium-win-x86.tgz",
            library_path_in_archive: "bin/pdfium.dll",
            archive_sha256: "25c635e70037c6a20a33126a812a63e891c70974982a2e00112b7aaa07eb3832",
            library_sha256: "51db7685cc3c9ee11bc4c101d44b4ba30cb11c911c31c5c6da79c5bea0d76ffa",
        }),
        ("macos", "x86_64") => Some(PdfiumDownloadTarget {
            archive_name: "pdfium-mac-x64.tgz",
            library_path_in_archive: "lib/libpdfium.dylib",
            archive_sha256: "2510460ac106f14b884598a0da3f53a99e23d79512acf027c5e101c2bb2f26cb",
            library_sha256: "c4ae7ca1583e04449d07f1985ce258a3f935583279fd46fa89f528106301b929",
        }),
        ("macos", "aarch64") => Some(PdfiumDownloadTarget {
            archive_name: "pdfium-mac-arm64.tgz",
            library_path_in_archive: "lib/libpdfium.dylib",
            archive_sha256: "41c269723b4711793de70ff34e65c00fa79907d6c023741837579e906b846faa",
            library_sha256: "858f0676a1ac5b666673fc6e56b4401f95907a3fc66fa4635d626097a04c205b",
        }),
        ("linux", "x86_64") if target_env != "musl" => Some(PdfiumDownloadTarget {
            archive_name: "pdfium-linux-x64.tgz",
            library_path_in_archive: "lib/libpdfium.so",
            archive_sha256: "9329a3c4b19b3c8d0a93af5440f44be84e4bd879a204e47b1a7a160e96809da4",
            library_sha256: "2383a414050dd21ae5300b119ad8a72360ef92cff820b4c685c047dc272c2794",
        }),
        ("linux", "aarch64") if target_env != "musl" => Some(PdfiumDownloadTarget {
            archive_name: "pdfium-linux-arm64.tgz",
            library_path_in_archive: "lib/libpdfium.so",
            archive_sha256: "4965a4c0b64c45b5edefa1072e2b483bf90b4d25d7deec44f104dcbdecf05c3e",
            library_sha256: "deab139b06cba02552d0d695eb4789600da41a2df9d9176f3ec5ce477bff53a8",
        }),
        _ => None,
    }
}

fn pdfium_library_name() -> PathBuf {
    PathBuf::from(Pdfium::pdfium_platform_library_name())
}

fn candidate_library_paths(library_name: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();

    /*
     * ========================================================================
     * 步骤1：收集显式覆盖路径
     * ========================================================================
     * 目标：
     * 1) 优先尊重用户显式提供的 Pdfium 路径
     * 2) 同时兼容 Zot 专用变量和通用变量
     */

    // 1.1 读取 Zot 专用路径变量
    if let Ok(value) = env::var(ZOT_PDFIUM_LIB_PATH) {
        push_candidate_path(&mut paths, candidate_from_env_value(&value, library_name));
    }

    // 1.2 读取通用 Pdfium 路径变量
    if let Ok(value) = env::var(PDFIUM_LIB_PATH) {
        push_candidate_path(&mut paths, candidate_from_env_value(&value, library_name));
    }

    /*
     * ========================================================================
     * 步骤2：收集受信任的本地落点
     * ========================================================================
     * 目标：
     * 1) 兼容与可执行文件同目录部署的 Pdfium
     * 2) 兼容受管缓存目录
     *
     * 安全约束：绝不把 current_dir() 纳入候选。CWD 对 CLI/agent 是不可信边界
     * （任意第三方仓库可放置同名 pdfium.dll/.so/.dylib 造成动态库劫持 RCE）。
     * 需要自定义路径时请显式设置 ZOT_PDFIUM_LIB_PATH / PDFIUM_LIB_PATH。
     */

    // 2.1 尝试可执行文件同目录
    if let Some(executable_dir) = env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf))
    {
        push_candidate_path(&mut paths, executable_dir.join(library_name));
    }

    // 2.2 尝试受管缓存目录
    if let Some(cache_path) = managed_cache_library_path() {
        push_candidate_path(&mut paths, cache_path);
    }

    paths
}

fn candidate_from_env_value(value: &str, library_name: &Path) -> PathBuf {
    let path = PathBuf::from(value);
    let is_explicit_file = path
        .file_name()
        .is_some_and(|file_name| file_name == library_name.as_os_str())
        || looks_like_library_file(&path);
    if is_explicit_file {
        path
    } else {
        path.join(library_name)
    }
}

fn looks_like_library_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| matches!(extension, "dll" | "so" | "dylib"))
}

fn push_candidate_path(paths: &mut Vec<PathBuf>, candidate: PathBuf) {
    if !paths.iter().any(|path| path == &candidate) {
        paths.push(candidate);
    }
}

fn managed_cache_library_path() -> Option<PathBuf> {
    let target = current_download_target()?;
    let library_name = pdfium_library_name();
    verified_managed_cache_path(&pdfium_cache_dir(), target, &library_name)
}

fn managed_cache_target_path_in(
    cache_dir: &Path,
    target: PdfiumDownloadTarget,
    library_name: &Path,
) -> PathBuf {
    let hash_prefix = &target.library_sha256[..16];
    let file_name = format!("sha256-{hash_prefix}-{}", library_name.to_string_lossy());
    cache_dir.join(file_name)
}

fn verified_managed_cache_path(
    cache_dir: &Path,
    target: PdfiumDownloadTarget,
    library_name: &Path,
) -> Option<PathBuf> {
    let path = managed_cache_target_path_in(cache_dir, target, library_name);
    file_matches_sha256(&path, target.library_sha256).then_some(path)
}

fn file_matches_sha256(path: &Path, expected: &str) -> bool {
    std::fs::File::open(path)
        .and_then(sha256_reader)
        .is_ok_and(|(actual, _)| actual == expected)
}

fn sha256_reader(mut reader: impl Read) -> std::io::Result<(String, u64)> {
    let mut hasher = Sha256::new();
    let mut total = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        total += read as u64;
    }
    Ok((format!("{:x}", hasher.finalize()), total))
}

fn pdfium_cache_dir() -> PathBuf {
    let base_dir = if let Ok(value) = env::var(ZOT_PDFIUM_CACHE_DIR) {
        PathBuf::from(value)
    } else {
        dirs::cache_dir().unwrap_or_else(env::temp_dir).join("zot")
    };
    base_dir.join(format!("pdfium-{PDFIUM_VERSION}"))
}

fn bind_pdfium_from_path(path: &Path) -> ZotResult<Pdfium> {
    match Pdfium::bind_to_library(path) {
        Ok(bindings) => Ok(Pdfium::new(bindings)),
        Err(PdfiumError::PdfiumLibraryBindingsAlreadyInitialized) => Ok(Pdfium),
        Err(error) => Err(pdfium_bind_error(error, path)),
    }
}

fn pdfium_bind_error(error: PdfiumError, path: &Path) -> ZotError {
    let hint = format!(
        "Failed to load Pdfium from {}. Set {ZOT_PDFIUM_LIB_PATH} or {PDFIUM_LIB_PATH} to a compatible library, or let Zot auto-download it on the first local PDF read.",
        path.display()
    );
    ZotError::Pdf {
        code: "pdfium-unavailable".to_string(),
        message: error.to_string(),
        hint: Some(hint),
    }
}

fn pdfium_manual_setup_error() -> ZotError {
    ZotError::Pdf {
        code: "pdfium-unavailable".to_string(),
        message: "No compatible Pdfium library is currently available".to_string(),
        hint: Some(format!(
            "Set {ZOT_PDFIUM_LIB_PATH} or {PDFIUM_LIB_PATH}, place Pdfium next to the executable, or use a supported platform so Zot can auto-download it on the first local PDF read."
        )),
    }
}

fn download_pdfium_library(
    target: PdfiumDownloadTarget,
    library_name: &Path,
) -> ZotResult<PathBuf> {
    let cache_dir = pdfium_cache_dir();
    install_pdfium_library(target, library_name, &cache_dir, |url, cache_dir| {
        download_archive_to_temp(url, cache_dir)
    })
}

fn install_pdfium_library<F>(
    target: PdfiumDownloadTarget,
    library_name: &Path,
    cache_dir: &Path,
    download: F,
) -> ZotResult<PathBuf>
where
    F: FnOnce(&str, &Path) -> ZotResult<NamedTempFile>,
{
    std::fs::create_dir_all(cache_dir).map_err(|source| ZotError::Io {
        path: cache_dir.to_path_buf(),
        source,
    })?;
    let lock_path = cache_dir.join(PDFIUM_INSTALL_LOCK_FILE);
    let lock_file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)
        .map_err(|source| ZotError::Io {
            path: lock_path.clone(),
            source,
        })?;
    fs4::FileExt::lock(&lock_file).map_err(|source| ZotError::Io {
        path: lock_path,
        source,
    })?;

    let library_path = managed_cache_target_path_in(cache_dir, target, library_name);
    if file_matches_sha256(&library_path, target.library_sha256) {
        return Ok(library_path);
    }

    let archive_url = format!("{PDFIUM_BASE_URL}/{}", target.archive_name);
    let mut archive_temp = download(&archive_url, cache_dir)?;
    verify_file_sha256(
        archive_temp.path(),
        target.archive_sha256,
        MAX_PDFIUM_ARCHIVE_BYTES,
        "pdfium-archive-too-large",
        "pdfium-archive-checksum",
    )?;
    let library_temp = extract_verified_library(&mut archive_temp, target, cache_dir)?;

    if file_matches_sha256(&library_path, target.library_sha256) {
        return Ok(library_path);
    }
    if library_path.exists() {
        std::fs::remove_file(&library_path).map_err(|source| ZotError::Io {
            path: library_path.clone(),
            source,
        })?;
    }
    library_temp
        .persist(&library_path)
        .map_err(|error| ZotError::Io {
            path: library_path.clone(),
            source: error.error,
        })?;
    sync_cache_directory(cache_dir)?;
    Ok(library_path)
}

fn pdfium_download_client() -> ZotResult<Client> {
    Client::builder()
        .connect_timeout(zot_core::net::CONNECT_TIMEOUT)
        .timeout(zot_core::net::REQUEST_TIMEOUT)
        .user_agent(zot_core::net::USER_AGENT)
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .map_err(|error| ZotError::Remote {
            code: "pdfium-download-client".to_string(),
            message: error.to_string(),
            hint: Some("Failed to initialize the managed Pdfium downloader".to_string()),
            status: error.status().map(|status| status.as_u16()),
        })
}

fn download_archive_to_temp(url: &str, cache_dir: &Path) -> ZotResult<NamedTempFile> {
    let client = pdfium_download_client()?;
    let mut response = client.get(url).send().map_err(|error| ZotError::Remote {
        code: "pdfium-download".to_string(),
        message: error.to_string(),
        hint: Some("Failed to download Pdfium from the managed release source".to_string()),
        status: error.status().map(|status| status.as_u16()),
    })?;
    if !response.status().is_success() {
        return Err(ZotError::Remote {
            code: "pdfium-download".to_string(),
            message: format!("Pdfium download failed with status {}", response.status()),
            hint: Some(
                "Retry later or set ZOT_PDFIUM_LIB_PATH / PDFIUM_LIB_PATH manually".to_string(),
            ),
            status: Some(response.status().as_u16()),
        });
    }
    if response
        .content_length()
        .is_some_and(|length| length > MAX_PDFIUM_ARCHIVE_BYTES)
    {
        return Err(pdfium_integrity_error(
            "pdfium-archive-too-large",
            format!(
                "Pdfium archive exceeds the {} byte download limit",
                MAX_PDFIUM_ARCHIVE_BYTES
            ),
        ));
    }

    let mut temp = NamedTempFile::new_in(cache_dir).map_err(|source| ZotError::Io {
        path: cache_dir.to_path_buf(),
        source,
    })?;
    let mut total = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = response
            .read(&mut buffer)
            .map_err(|error| ZotError::Remote {
                code: "pdfium-download-read".to_string(),
                message: error.to_string(),
                hint: Some("Failed to read the downloaded Pdfium archive".to_string()),
                status: None,
            })?;
        if read == 0 {
            break;
        }
        total += read as u64;
        if total > MAX_PDFIUM_ARCHIVE_BYTES {
            return Err(pdfium_integrity_error(
                "pdfium-archive-too-large",
                format!(
                    "Pdfium archive exceeds the {} byte download limit",
                    MAX_PDFIUM_ARCHIVE_BYTES
                ),
            ));
        }
        temp.write_all(&buffer[..read])
            .map_err(|source| ZotError::Io {
                path: temp.path().to_path_buf(),
                source,
            })?;
    }
    sync_temp_file(&mut temp)?;
    Ok(temp)
}

fn extract_verified_library(
    archive_temp: &mut NamedTempFile,
    target: PdfiumDownloadTarget,
    cache_dir: &Path,
) -> ZotResult<NamedTempFile> {
    archive_temp
        .as_file_mut()
        .seek(SeekFrom::Start(0))
        .map_err(|source| ZotError::Io {
            path: archive_temp.path().to_path_buf(),
            source,
        })?;
    let decoder = GzDecoder::new(archive_temp.as_file_mut());
    let mut archive = Archive::new(decoder);
    for entry in archive.entries().map_err(|error| ZotError::Pdf {
        code: "pdfium-archive-open".to_string(),
        message: error.to_string(),
        hint: Some("Failed to inspect the downloaded Pdfium archive".to_string()),
    })? {
        let mut entry = entry.map_err(|error| ZotError::Pdf {
            code: "pdfium-archive-entry".to_string(),
            message: error.to_string(),
            hint: Some("Failed to read an entry from the downloaded Pdfium archive".to_string()),
        })?;
        let entry_path = entry.path().map_err(|error| ZotError::Pdf {
            code: "pdfium-archive-path".to_string(),
            message: error.to_string(),
            hint: Some(
                "Failed to resolve an entry path inside the downloaded Pdfium archive".to_string(),
            ),
        })?;
        if entry_path.to_string_lossy() == target.library_path_in_archive {
            if !entry.header().entry_type().is_file() {
                return Err(pdfium_integrity_error(
                    "pdfium-archive-entry-type",
                    "Expected Pdfium archive entry is not a regular file".to_string(),
                ));
            }
            if entry.size() > MAX_PDFIUM_LIBRARY_BYTES {
                return Err(pdfium_integrity_error(
                    "pdfium-library-too-large",
                    format!(
                        "Pdfium library exceeds the {} byte extraction limit",
                        MAX_PDFIUM_LIBRARY_BYTES
                    ),
                ));
            }
            let mut library_temp =
                NamedTempFile::new_in(cache_dir).map_err(|source| ZotError::Io {
                    path: cache_dir.to_path_buf(),
                    source,
                })?;
            let copied = std::io::copy(
                &mut entry.by_ref().take(MAX_PDFIUM_LIBRARY_BYTES + 1),
                &mut library_temp,
            )
            .map_err(|error| ZotError::Pdf {
                code: "pdfium-archive-entry-read".to_string(),
                message: error.to_string(),
                hint: Some("Downloaded Pdfium archive is truncated or invalid".to_string()),
            })?;
            if copied > MAX_PDFIUM_LIBRARY_BYTES {
                return Err(pdfium_integrity_error(
                    "pdfium-library-too-large",
                    format!(
                        "Pdfium library exceeds the {} byte extraction limit",
                        MAX_PDFIUM_LIBRARY_BYTES
                    ),
                ));
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;

                let mut permissions = library_temp
                    .as_file()
                    .metadata()
                    .map_err(|source| ZotError::Io {
                        path: library_temp.path().to_path_buf(),
                        source,
                    })?
                    .permissions();
                permissions.set_mode(permissions.mode() | 0o755);
                library_temp
                    .as_file()
                    .set_permissions(permissions)
                    .map_err(|source| ZotError::Io {
                        path: library_temp.path().to_path_buf(),
                        source,
                    })?;
            }
            sync_temp_file(&mut library_temp)?;
            verify_file_sha256(
                library_temp.path(),
                target.library_sha256,
                MAX_PDFIUM_LIBRARY_BYTES,
                "pdfium-library-too-large",
                "pdfium-library-checksum",
            )?;
            return Ok(library_temp);
        }
    }
    Err(ZotError::Pdf {
        code: "pdfium-archive-missing-library".to_string(),
        message: format!(
            "Pdfium archive did not contain the expected library entry {}",
            target.library_path_in_archive
        ),
        hint: Some(format!(
            "Delete {} and retry the command",
            cache_dir.display()
        )),
    })
}

fn sync_temp_file(temp: &mut NamedTempFile) -> ZotResult<()> {
    temp.flush().map_err(|source| ZotError::Io {
        path: temp.path().to_path_buf(),
        source,
    })?;
    temp.as_file_mut()
        .sync_all()
        .map_err(|source| ZotError::Io {
            path: temp.path().to_path_buf(),
            source,
        })
}

fn verify_file_sha256(
    path: &Path,
    expected: &str,
    max_bytes: u64,
    size_code: &str,
    checksum_code: &str,
) -> ZotResult<()> {
    let file = File::open(path).map_err(|source| ZotError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let (actual, size) = sha256_reader(file).map_err(|source| ZotError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    if size > max_bytes {
        return Err(pdfium_integrity_error(
            size_code,
            format!("Pdfium artifact exceeds the {max_bytes} byte limit"),
        ));
    }
    if actual != expected {
        return Err(pdfium_integrity_error(
            checksum_code,
            format!("Pdfium SHA-256 mismatch: expected {expected}, received {actual}"),
        ));
    }
    Ok(())
}

fn pdfium_integrity_error(code: &str, message: String) -> ZotError {
    ZotError::Pdf {
        code: code.to_string(),
        message,
        hint: Some(
            "Managed Pdfium installation was rejected; retry or set an explicit reviewed library path"
                .to_string(),
        ),
    }
}

#[cfg(unix)]
fn sync_cache_directory(cache_dir: &Path) -> ZotResult<()> {
    File::open(cache_dir)
        .and_then(|directory| directory.sync_all())
        .map_err(|source| ZotError::Io {
            path: cache_dir.to_path_buf(),
            source,
        })
}

#[cfg(not(unix))]
fn sync_cache_directory(_cache_dir: &Path) -> ZotResult<()> {
    Ok(())
}

fn cache_key_for(pdf_path: &Path) -> ZotResult<String> {
    let metadata = std::fs::metadata(pdf_path).map_err(|source| ZotError::Io {
        path: pdf_path.to_path_buf(),
        source,
    })?;
    let modified = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    let raw = format!("{}:{modified}:{}", pdf_path.display(), metadata.len());
    Ok(format!("{:x}", md5::compute(raw)))
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Barrier, Mutex};
    use std::thread;
    use std::time::Duration;

    use super::*;
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use tar::Builder;

    static PDFIUM_ENV_LOCK: Mutex<()> = Mutex::new(());

    fn sha256_bytes(bytes: &[u8]) -> String {
        sha256_reader(Cursor::new(bytes)).expect("hash bytes").0
    }

    fn regular_archive(path: &str, payload: &[u8]) -> Vec<u8> {
        let encoder = GzEncoder::new(Vec::new(), Compression::default());
        let mut builder = Builder::new(encoder);
        let mut header = tar::Header::new_gnu();
        header.set_path(path).expect("path");
        header.set_size(payload.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder.append(&header, payload).expect("append");
        let encoder = builder.into_inner().expect("finish tar");
        encoder.finish().expect("finish gzip")
    }

    fn symlink_archive(path: &str) -> Vec<u8> {
        let encoder = GzEncoder::new(Vec::new(), Compression::default());
        let mut builder = Builder::new(encoder);
        let mut header = tar::Header::new_gnu();
        header.set_entry_type(tar::EntryType::Symlink);
        header.set_size(0);
        header.set_mode(0o777);
        header.set_cksum();
        builder
            .append_link(&mut header, path, "other-file")
            .expect("append link");
        let encoder = builder.into_inner().expect("finish tar");
        encoder.finish().expect("finish gzip")
    }

    fn fixture_target(
        archive: &[u8],
        library_path_in_archive: &'static str,
        expected_library: &[u8],
    ) -> PdfiumDownloadTarget {
        PdfiumDownloadTarget {
            archive_name: "fixture.tgz",
            library_path_in_archive,
            archive_sha256: Box::leak(sha256_bytes(archive).into_boxed_str()),
            library_sha256: Box::leak(sha256_bytes(expected_library).into_boxed_str()),
        }
    }

    fn archive_temp(cache_dir: &Path, bytes: &[u8]) -> ZotResult<NamedTempFile> {
        let mut temp = NamedTempFile::new_in(cache_dir).map_err(|source| ZotError::Io {
            path: cache_dir.to_path_buf(),
            source,
        })?;
        temp.write_all(bytes).map_err(|source| ZotError::Io {
            path: temp.path().to_path_buf(),
            source,
        })?;
        sync_temp_file(&mut temp)?;
        Ok(temp)
    }

    #[test]
    fn download_client_builds_with_shared_net_defaults() {
        // 冒烟:引导下载 client 用 zot_core::net 共享常量(超时/UA 与主栈对齐)
        // 能成功构建;不发起真实网络请求。
        let client = pdfium_download_client();
        assert!(client.is_ok());
        assert!(zot_core::net::USER_AGENT.starts_with("zot-cli/"));
    }

    #[test]
    fn resolves_download_targets_for_supported_platforms() {
        /*
         * ========================================================================
         * 步骤1：校验三平台下载映射
         * ========================================================================
         * 目标：
         * 1) 保证 Windows、macOS、Linux 的归档名固定
         * 2) 保证 musl Linux 不会被误判为自动下载可用
         */
        eprintln!("开始校验 Pdfium 下载目标映射...");

        let cases = [
            (
                "windows",
                "x86_64",
                "",
                PdfiumDownloadTarget {
                    archive_name: "pdfium-win-x64.tgz",
                    library_path_in_archive: "bin/pdfium.dll",
                    archive_sha256: "0b08b606792a6cc593426efdefc6622611bce446d9e0270743846956ea1554ca",
                    library_sha256: "6b963c2be9cacbaa0c0c7f4bf6d20d2fd16729ebdaa9989978b0f7b119c1c1cb",
                },
            ),
            (
                "windows",
                "aarch64",
                "",
                PdfiumDownloadTarget {
                    archive_name: "pdfium-win-arm64.tgz",
                    library_path_in_archive: "bin/pdfium.dll",
                    archive_sha256: "bb4a00113494e25bbee52d3d63b7f4ecf0de2d277b7de75ba9a1d5b987a74509",
                    library_sha256: "368986d82c11a22e0c53728873899cf864dbd7b32a42214a660ac30fe8ba37f4",
                },
            ),
            (
                "windows",
                "x86",
                "",
                PdfiumDownloadTarget {
                    archive_name: "pdfium-win-x86.tgz",
                    library_path_in_archive: "bin/pdfium.dll",
                    archive_sha256: "25c635e70037c6a20a33126a812a63e891c70974982a2e00112b7aaa07eb3832",
                    library_sha256: "51db7685cc3c9ee11bc4c101d44b4ba30cb11c911c31c5c6da79c5bea0d76ffa",
                },
            ),
            (
                "macos",
                "x86_64",
                "",
                PdfiumDownloadTarget {
                    archive_name: "pdfium-mac-x64.tgz",
                    library_path_in_archive: "lib/libpdfium.dylib",
                    archive_sha256: "2510460ac106f14b884598a0da3f53a99e23d79512acf027c5e101c2bb2f26cb",
                    library_sha256: "c4ae7ca1583e04449d07f1985ce258a3f935583279fd46fa89f528106301b929",
                },
            ),
            (
                "macos",
                "aarch64",
                "",
                PdfiumDownloadTarget {
                    archive_name: "pdfium-mac-arm64.tgz",
                    library_path_in_archive: "lib/libpdfium.dylib",
                    archive_sha256: "41c269723b4711793de70ff34e65c00fa79907d6c023741837579e906b846faa",
                    library_sha256: "858f0676a1ac5b666673fc6e56b4401f95907a3fc66fa4635d626097a04c205b",
                },
            ),
            (
                "linux",
                "x86_64",
                "gnu",
                PdfiumDownloadTarget {
                    archive_name: "pdfium-linux-x64.tgz",
                    library_path_in_archive: "lib/libpdfium.so",
                    archive_sha256: "9329a3c4b19b3c8d0a93af5440f44be84e4bd879a204e47b1a7a160e96809da4",
                    library_sha256: "2383a414050dd21ae5300b119ad8a72360ef92cff820b4c685c047dc272c2794",
                },
            ),
            (
                "linux",
                "aarch64",
                "gnu",
                PdfiumDownloadTarget {
                    archive_name: "pdfium-linux-arm64.tgz",
                    library_path_in_archive: "lib/libpdfium.so",
                    archive_sha256: "4965a4c0b64c45b5edefa1072e2b483bf90b4d25d7deec44f104dcbdecf05c3e",
                    library_sha256: "deab139b06cba02552d0d695eb4789600da41a2df9d9176f3ec5ce477bff53a8",
                },
            ),
        ];
        for (os, arch, target_env, expected) in cases {
            assert_eq!(download_target_for(os, arch, target_env), Some(expected));
        }

        assert_eq!(download_target_for("linux", "x86_64", "musl"), None);

        eprintln!("Pdfium 下载目标映射校验完成");
    }

    #[test]
    fn cache_dir_uses_override_and_version_suffix() {
        let _env_guard = PDFIUM_ENV_LOCK.lock().expect("env lock");
        /*
         * ========================================================================
         * 步骤2：校验缓存目录规则
         * ========================================================================
         * 目标：
         * 1) 保证自定义缓存根目录会生效
         * 2) 保证版本号后缀不会丢失
         */
        eprintln!("开始校验 Pdfium 缓存目录规则...");

        // 2.1 设置自定义缓存根目录
        let tempdir = tempfile::tempdir().expect("tempdir");
        unsafe {
            env::set_var(ZOT_PDFIUM_CACHE_DIR, tempdir.path());
        }

        // 2.2 读取缓存目录并断言版本后缀
        let cache_dir = pdfium_cache_dir();
        assert!(cache_dir.starts_with(tempdir.path()));
        assert!(cache_dir.ends_with(format!("pdfium-{PDFIUM_VERSION}")));

        // 2.3 清理环境变量
        unsafe {
            env::remove_var(ZOT_PDFIUM_CACHE_DIR);
        }

        eprintln!("Pdfium 缓存目录规则校验完成");
    }

    #[test]
    fn env_candidates_prefer_explicit_file_and_directory_inputs() {
        /*
         * ========================================================================
         * 步骤3：校验显式路径候选生成
         * ========================================================================
         * 目标：
         * 1) 保证文件路径不会被错误拼接
         * 2) 保证目录路径会补上当前平台库名
         */
        eprintln!("开始校验 Pdfium 显式路径候选生成...");

        // 3.1 准备当前平台库名
        let library_name = pdfium_library_name();

        // 3.2 校验显式文件路径
        let explicit_file = candidate_from_env_value("C:\\pdfium\\pdfium.dll", &library_name);
        assert_eq!(explicit_file, PathBuf::from("C:\\pdfium\\pdfium.dll"));

        // 3.3 校验目录路径
        let explicit_dir = candidate_from_env_value("C:\\pdfium", &library_name);
        assert_eq!(
            explicit_dir,
            PathBuf::from("C:\\pdfium").join(&library_name)
        );

        eprintln!("Pdfium 显式路径候选生成校验完成");
    }

    #[test]
    fn candidate_library_paths_only_uses_trusted_sources() {
        let _env_guard = PDFIUM_ENV_LOCK.lock().expect("env lock");
        // Security regression (P0-01 / QW-01): the discovery result must be
        // exactly the documented trusted sources. In particular, it must not
        // grow an implicit current_dir() or bare system-library candidate.
        let library_name = pdfium_library_name();
        let candidates = candidate_library_paths(&library_name);
        let mut expected = Vec::new();

        if let Ok(value) = env::var(ZOT_PDFIUM_LIB_PATH) {
            push_candidate_path(
                &mut expected,
                candidate_from_env_value(&value, &library_name),
            );
        }
        if let Ok(value) = env::var(PDFIUM_LIB_PATH) {
            push_candidate_path(
                &mut expected,
                candidate_from_env_value(&value, &library_name),
            );
        }
        if let Some(executable_dir) = env::current_exe()
            .ok()
            .and_then(|path| path.parent().map(Path::to_path_buf))
        {
            push_candidate_path(&mut expected, executable_dir.join(&library_name));
        }
        if let Some(cache_path) = managed_cache_library_path() {
            push_candidate_path(&mut expected, cache_path);
        }

        assert_eq!(candidates, expected);
    }

    #[test]
    fn installs_verified_library_atomically() {
        let payload = b"pdfium";
        let archive = regular_archive("bin/pdfium.dll", payload);
        let target = fixture_target(&archive, "bin/pdfium.dll", payload);
        let tempdir = tempfile::tempdir().expect("tempdir");
        let output = install_pdfium_library(
            target,
            Path::new("pdfium.dll"),
            tempdir.path(),
            |_, cache_dir| archive_temp(cache_dir, &archive),
        )
        .expect("install");

        assert_eq!(std::fs::read(&output).expect("read"), payload);
        assert!(file_matches_sha256(&output, target.library_sha256));
        assert!(
            output
                .file_name()
                .is_some_and(|name| { name.to_string_lossy().starts_with("sha256-") })
        );
    }

    #[test]
    fn managed_cache_rejects_legacy_and_tampered_libraries() {
        let payload = b"verified-pdfium";
        let target = fixture_target(&[], "bin/pdfium.dll", payload);
        let tempdir = tempfile::tempdir().expect("tempdir");
        let library_name = Path::new("pdfium.dll");
        std::fs::write(tempdir.path().join(library_name), payload).expect("write legacy cache");

        assert_eq!(
            verified_managed_cache_path(tempdir.path(), target, library_name),
            None,
            "the legacy bare filename is not a verified managed candidate"
        );

        let verified_path = managed_cache_target_path_in(tempdir.path(), target, library_name);
        std::fs::write(&verified_path, b"tampered").expect("write tampered cache");
        assert_eq!(
            verified_managed_cache_path(tempdir.path(), target, library_name),
            None
        );

        std::fs::write(&verified_path, payload).expect("write verified cache");
        assert_eq!(
            verified_managed_cache_path(tempdir.path(), target, library_name),
            Some(verified_path)
        );
    }

    #[test]
    fn tampered_truncated_and_wrong_platform_archives_fail_closed() {
        let payload = b"pdfium";
        let valid_archive = regular_archive("bin/pdfium.dll", payload);
        let target = fixture_target(&valid_archive, "bin/pdfium.dll", payload);
        let mut tampered = valid_archive.clone();
        let last = tampered.last_mut().expect("non-empty archive");
        *last ^= 0xff;
        let mut truncated = valid_archive.clone();
        truncated.truncate(truncated.len() / 2);
        let wrong_platform = regular_archive("lib/libpdfium.so", payload);

        for (label, archive) in [
            ("tampered", tampered),
            ("truncated", truncated),
            ("wrong-platform", wrong_platform),
        ] {
            let tempdir = tempfile::tempdir().expect("tempdir");
            let final_path =
                managed_cache_target_path_in(tempdir.path(), target, Path::new("pdfium.dll"));
            let err = install_pdfium_library(
                target,
                Path::new("pdfium.dll"),
                tempdir.path(),
                |_, cache_dir| archive_temp(cache_dir, &archive),
            )
            .expect_err(label);
            assert_eq!(
                err.payload().code,
                "pdfium-archive-checksum",
                "case: {label}"
            );
            assert!(!final_path.exists(), "case published a file: {label}");
        }
    }

    #[test]
    fn archive_shape_and_library_checksum_fail_closed() {
        let payload = b"pdfium";
        let missing = regular_archive("bin/other.dll", payload);
        let link = symlink_archive("bin/pdfium.dll");
        let mismatched_library = regular_archive("bin/pdfium.dll", payload);
        let cases = [
            (
                "missing",
                missing.clone(),
                fixture_target(&missing, "bin/pdfium.dll", payload),
                "pdfium-archive-missing-library",
            ),
            (
                "symlink",
                link.clone(),
                fixture_target(&link, "bin/pdfium.dll", payload),
                "pdfium-archive-entry-type",
            ),
            (
                "library-mismatch",
                mismatched_library.clone(),
                fixture_target(&mismatched_library, "bin/pdfium.dll", b"different"),
                "pdfium-library-checksum",
            ),
        ];

        for (label, archive, target, expected_code) in cases {
            let tempdir = tempfile::tempdir().expect("tempdir");
            let final_path =
                managed_cache_target_path_in(tempdir.path(), target, Path::new("pdfium.dll"));
            let err = install_pdfium_library(
                target,
                Path::new("pdfium.dll"),
                tempdir.path(),
                |_, cache_dir| archive_temp(cache_dir, &archive),
            )
            .expect_err(label);
            assert_eq!(err.payload().code, expected_code, "case: {label}");
            assert!(!final_path.exists(), "case published a file: {label}");
        }
    }

    #[test]
    fn artifact_size_limit_is_enforced_before_checksum_acceptance() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let path = tempdir.path().join("oversize.tgz");
        std::fs::write(&path, b"12345").expect("write fixture");

        let err = verify_file_sha256(
            &path,
            &sha256_bytes(b"12345"),
            4,
            "pdfium-archive-too-large",
            "pdfium-archive-checksum",
        )
        .expect_err("oversize file must fail");
        assert_eq!(err.payload().code, "pdfium-archive-too-large");
    }

    #[test]
    fn existing_verified_library_survives_failed_redownload_attempt() {
        let payload = b"verified-pdfium";
        let archive = regular_archive("bin/pdfium.dll", payload);
        let target = fixture_target(&archive, "bin/pdfium.dll", payload);
        let tempdir = tempfile::tempdir().expect("tempdir");
        let first = install_pdfium_library(
            target,
            Path::new("pdfium.dll"),
            tempdir.path(),
            |_, cache_dir| archive_temp(cache_dir, &archive),
        )
        .expect("first install");

        let second =
            install_pdfium_library(target, Path::new("pdfium.dll"), tempdir.path(), |_, _| {
                panic!("verified install must skip the downloader")
            })
            .expect("reuse verified install");

        assert_eq!(first, second);
        assert_eq!(std::fs::read(second).expect("read library"), payload);
    }

    #[test]
    fn concurrent_installers_download_once() {
        let payload = b"verified-pdfium";
        let archive = Arc::new(regular_archive("bin/pdfium.dll", payload));
        let target = fixture_target(&archive, "bin/pdfium.dll", payload);
        let tempdir = tempfile::tempdir().expect("tempdir");
        let cache_dir = Arc::new(tempdir.path().to_path_buf());
        let barrier = Arc::new(Barrier::new(3));
        let downloads = Arc::new(AtomicUsize::new(0));
        let mut workers = Vec::new();

        for _ in 0..2 {
            let archive = Arc::clone(&archive);
            let cache_dir = Arc::clone(&cache_dir);
            let barrier = Arc::clone(&barrier);
            let downloads = Arc::clone(&downloads);
            workers.push(thread::spawn(move || {
                barrier.wait();
                install_pdfium_library(
                    target,
                    Path::new("pdfium.dll"),
                    &cache_dir,
                    |_, cache_dir| {
                        downloads.fetch_add(1, Ordering::SeqCst);
                        thread::sleep(Duration::from_millis(50));
                        archive_temp(cache_dir, &archive)
                    },
                )
            }));
        }

        barrier.wait();
        let first = workers
            .remove(0)
            .join()
            .expect("first worker")
            .expect("install");
        let second = workers
            .remove(0)
            .join()
            .expect("second worker")
            .expect("install");
        assert_eq!(first, second);
        assert_eq!(downloads.load(Ordering::SeqCst), 1);
        assert_eq!(std::fs::read(first).expect("read library"), payload);
    }
}
