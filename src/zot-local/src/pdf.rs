use std::env;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use flate2::read::GzDecoder;
use pdfium_render::prelude::*;
use reqwest::blocking::Client;
use rusqlite::{Connection, OptionalExtension, params};
use tar::Archive;
use zot_core::{AnnotationSnippet, PdfOutlineEntry, ZotError, ZotResult};

#[derive(Debug, Clone, PartialEq)]
pub struct PdfMatchPosition {
    pub page_index: usize,
    pub page_label: String,
    pub matched_text: String,
    pub rects: Vec<[f32; 4]>,
    pub sort_index: String,
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PdfiumLoadMode {
    ProbeOnly,
    AllowDownload,
}

const PDFIUM_VERSION: &str = "7543";
const PDFIUM_BASE_URL: &str =
    "https://github.com/bblanchon/pdfium-binaries/releases/download/chromium%2F7543";
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

        match bind_pdfium_from_system() {
            Ok(pdfium) => return Ok(pdfium),
            Err(error) => {
                if last_error.is_none() {
                    last_error = Some(error);
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
            let level = bookmark
                .title()
                .as_deref()
                .map(|title| title.matches('.').count() + 1)
                .unwrap_or(1);
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
    ) -> ZotResult<Option<PdfMatchPosition>> {
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
        if let Some(result) = search.find_next() {
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
            let first = rects.first().copied().unwrap_or([0.0, 0.0, 0.0, 0.0]);
            let sort_index = format!(
                "{:05}|{:06}|{:05}",
                page.saturating_sub(1),
                first[1].round() as i64,
                first[0].round() as i64
            );
            return Ok(Some(PdfMatchPosition {
                page_index: page.saturating_sub(1),
                page_label,
                matched_text: result
                    .iter()
                    .map(|segment| segment.text())
                    .collect::<Vec<_>>()
                    .join(" "),
                rects,
                sort_index,
            }));
        }
        Ok(None)
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
    zot_core::AppConfig::config_dir()
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
        }),
        ("windows", "aarch64") => Some(PdfiumDownloadTarget {
            archive_name: "pdfium-win-arm64.tgz",
            library_path_in_archive: "bin/pdfium.dll",
        }),
        ("windows", "x86") => Some(PdfiumDownloadTarget {
            archive_name: "pdfium-win-x86.tgz",
            library_path_in_archive: "bin/pdfium.dll",
        }),
        ("macos", "x86_64") => Some(PdfiumDownloadTarget {
            archive_name: "pdfium-mac-x64.tgz",
            library_path_in_archive: "lib/libpdfium.dylib",
        }),
        ("macos", "aarch64") => Some(PdfiumDownloadTarget {
            archive_name: "pdfium-mac-arm64.tgz",
            library_path_in_archive: "lib/libpdfium.dylib",
        }),
        ("linux", "x86_64") if target_env != "musl" => Some(PdfiumDownloadTarget {
            archive_name: "pdfium-linux-x64.tgz",
            library_path_in_archive: "lib/libpdfium.so",
        }),
        ("linux", "aarch64") if target_env != "musl" => Some(PdfiumDownloadTarget {
            archive_name: "pdfium-linux-arm64.tgz",
            library_path_in_archive: "lib/libpdfium.so",
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
     * 步骤2：收集本地常见落点
     * ========================================================================
     * 目标：
     * 1) 兼容与可执行文件同目录部署的 Pdfium
     * 2) 兼容当前工作目录和受管缓存目录
     */

    // 2.1 尝试可执行文件同目录
    if let Some(executable_dir) = env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf))
    {
        push_candidate_path(&mut paths, executable_dir.join(library_name));
    }

    // 2.2 尝试当前工作目录
    if let Ok(current_dir) = env::current_dir() {
        push_candidate_path(&mut paths, current_dir.join(library_name));
    }

    // 2.3 尝试受管缓存目录
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
    let library_name = pdfium_library_name();
    Some(pdfium_cache_dir().join(library_name))
}

fn pdfium_cache_dir() -> PathBuf {
    let base_dir = if let Ok(value) = env::var(ZOT_PDFIUM_CACHE_DIR) {
        PathBuf::from(value)
    } else {
        dirs::cache_dir().unwrap_or_else(env::temp_dir).join("zot")
    };
    base_dir.join(format!("pdfium-{PDFIUM_VERSION}"))
}

fn bind_pdfium_from_system() -> ZotResult<Pdfium> {
    match Pdfium::bind_to_system_library() {
        Ok(bindings) => Ok(Pdfium::new(bindings)),
        Err(PdfiumError::PdfiumLibraryBindingsAlreadyInitialized) => Ok(Pdfium),
        Err(error) => Err(pdfium_bind_error(error, None)),
    }
}

fn bind_pdfium_from_path(path: &Path) -> ZotResult<Pdfium> {
    match Pdfium::bind_to_library(path) {
        Ok(bindings) => Ok(Pdfium::new(bindings)),
        Err(PdfiumError::PdfiumLibraryBindingsAlreadyInitialized) => Ok(Pdfium),
        Err(error) => Err(pdfium_bind_error(error, Some(path))),
    }
}

fn pdfium_bind_error(error: PdfiumError, path: Option<&Path>) -> ZotError {
    let hint = path
        .map(|path| {
            format!(
                "Failed to load Pdfium from {}. Set {ZOT_PDFIUM_LIB_PATH} or {PDFIUM_LIB_PATH} to a compatible library, or let Zot auto-download it on the first local PDF read.",
                path.display()
            )
        })
        .unwrap_or_else(|| {
            format!(
                "Install a compatible Pdfium library, place it next to the executable, or set {ZOT_PDFIUM_LIB_PATH} / {PDFIUM_LIB_PATH}."
            )
        });
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
    std::fs::create_dir_all(&cache_dir).map_err(|source| ZotError::Io {
        path: cache_dir.clone(),
        source,
    })?;
    let archive_url = format!("{PDFIUM_BASE_URL}/{}", target.archive_name);
    let archive_bytes = download_archive_bytes(&archive_url)?;
    let library_path = cache_dir.join(library_name);
    extract_library_from_archive(
        &archive_bytes,
        target.library_path_in_archive,
        &library_path,
        &cache_dir,
    )?;
    Ok(library_path)
}

fn download_archive_bytes(url: &str) -> ZotResult<Vec<u8>> {
    let client = Client::builder()
        .user_agent(concat!("zot/", env!("CARGO_PKG_VERSION")))
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .map_err(|error| ZotError::Remote {
            code: "pdfium-download-client".to_string(),
            message: error.to_string(),
            hint: Some("Failed to initialize the managed Pdfium downloader".to_string()),
            status: error.status().map(|status| status.as_u16()),
        })?;
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
    let mut bytes = Vec::new();
    response
        .read_to_end(&mut bytes)
        .map_err(|error| ZotError::Remote {
            code: "pdfium-download-read".to_string(),
            message: error.to_string(),
            hint: Some("Failed to read the downloaded Pdfium archive".to_string()),
            status: None,
        })?;
    Ok(bytes)
}

fn extract_library_from_archive(
    archive_bytes: &[u8],
    library_path_in_archive: &str,
    library_path: &Path,
    cache_dir: &Path,
) -> ZotResult<()> {
    let decoder = GzDecoder::new(archive_bytes);
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
        if entry_path.to_string_lossy() == library_path_in_archive {
            entry.unpack(library_path).map_err(|error| ZotError::Io {
                path: library_path.to_path_buf(),
                source: error,
            })?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;

                let mut permissions = std::fs::metadata(library_path)
                    .map_err(|source| ZotError::Io {
                        path: library_path.to_path_buf(),
                        source,
                    })?
                    .permissions();
                permissions.set_mode(permissions.mode() | 0o755);
                std::fs::set_permissions(library_path, permissions).map_err(|source| {
                    ZotError::Io {
                        path: library_path.to_path_buf(),
                        source,
                    }
                })?;
            }
            return Ok(());
        }
    }
    Err(ZotError::Pdf {
        code: "pdfium-archive-missing-library".to_string(),
        message: format!(
            "Pdfium archive did not contain the expected library entry {library_path_in_archive}"
        ),
        hint: Some(format!(
            "Delete {} and retry the command",
            cache_dir.display()
        )),
    })
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
    use super::*;
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use tar::Builder;

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

        // 1.1 校验 Windows x64
        assert_eq!(
            download_target_for("windows", "x86_64", ""),
            Some(PdfiumDownloadTarget {
                archive_name: "pdfium-win-x64.tgz",
                library_path_in_archive: "bin/pdfium.dll",
            })
        );

        // 1.2 校验 macOS arm64
        assert_eq!(
            download_target_for("macos", "aarch64", ""),
            Some(PdfiumDownloadTarget {
                archive_name: "pdfium-mac-arm64.tgz",
                library_path_in_archive: "lib/libpdfium.dylib",
            })
        );

        // 1.3 校验 Linux x64 glibc
        assert_eq!(
            download_target_for("linux", "x86_64", "gnu"),
            Some(PdfiumDownloadTarget {
                archive_name: "pdfium-linux-x64.tgz",
                library_path_in_archive: "lib/libpdfium.so",
            })
        );

        // 1.4 校验 Linux x64 musl 不自动下载
        assert_eq!(download_target_for("linux", "x86_64", "musl"), None);

        eprintln!("Pdfium 下载目标映射校验完成");
    }

    #[test]
    fn cache_dir_uses_override_and_version_suffix() {
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
    fn extracts_library_from_memory_archive() {
        /*
         * ========================================================================
         * 步骤4：校验内存归档解压
         * ========================================================================
         * 目标：
         * 1) 保证内存 tar.gz 能正确抽出目标库文件
         * 2) 保证解压结果可写入目标路径
         */
        eprintln!("开始校验 Pdfium 内存归档解压...");

        // 4.1 构造内存 tar.gz 夹具
        let mut tar_bytes = Vec::new();
        {
            let encoder = GzEncoder::new(&mut tar_bytes, Compression::default());
            let mut builder = Builder::new(encoder);
            let mut header = tar::Header::new_gnu();
            let payload = b"pdfium";
            header.set_path("bin/pdfium.dll").expect("path");
            header.set_size(payload.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            builder.append(&header, &payload[..]).expect("append");
            builder.finish().expect("finish");
        }

        // 4.2 解压到临时目录
        let tempdir = tempfile::tempdir().expect("tempdir");
        let output = tempdir.path().join("pdfium.dll");
        extract_library_from_archive(&tar_bytes, "bin/pdfium.dll", &output, tempdir.path())
            .expect("extract");

        // 4.3 校验目标文件内容
        assert_eq!(std::fs::read(&output).expect("read"), b"pdfium");

        eprintln!("Pdfium 内存归档解压校验完成");
    }
}
