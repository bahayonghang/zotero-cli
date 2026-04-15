use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use pdfium_render::prelude::*;
use rusqlite::{Connection, OptionalExtension, params};
use zot_core::{AnnotationSnippet, ZotError, ZotResult};

pub trait PdfBackend {
    fn availability_hint(&self) -> ZotResult<()>;
    fn extract_text(
        &self,
        pdf_path: &Path,
        page_range: Option<(usize, usize)>,
    ) -> ZotResult<String>;
    fn extract_annotations(&self, pdf_path: &Path) -> ZotResult<Vec<AnnotationSnippet>>;
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
    fn pdfium() -> ZotResult<Pdfium> {
        let bindings = Pdfium::bind_to_system_library().map_err(|err| ZotError::Pdf {
            code: "pdfium-unavailable".to_string(),
            message: err.to_string(),
            hint: Some(
                "Install a compatible Pdfium library or place bundled binaries next to the executable".to_string(),
            ),
        })?;
        Ok(Pdfium::new(bindings))
    }
}

impl PdfBackend for PdfiumBackend {
    fn availability_hint(&self) -> ZotResult<()> {
        let _ = Self::pdfium()?;
        Ok(())
    }

    fn extract_text(
        &self,
        pdf_path: &Path,
        page_range: Option<(usize, usize)>,
    ) -> ZotResult<String> {
        let pdfium = Self::pdfium()?;
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

        let pdfium = Self::pdfium()?;
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
