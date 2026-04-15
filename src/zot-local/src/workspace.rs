use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use chrono::Utc;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use zot_core::{Item, QueryChunk, Workspace, WorkspaceItem, ZotError, ZotResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HybridMode {
    Bm25,
    Semantic,
    Hybrid,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct WorkspaceFile {
    created: String,
    description: String,
    #[serde(default)]
    items: Vec<WorkspaceItem>,
}

pub struct WorkspaceStore {
    root: PathBuf,
}

impl WorkspaceStore {
    pub fn new(root: Option<PathBuf>) -> Self {
        Self {
            root: root.unwrap_or_else(default_workspaces_dir),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn exists(&self, name: &str) -> bool {
        self.path_for(name).exists()
    }

    pub fn path_for(&self, name: &str) -> PathBuf {
        self.root.join(format!("{name}.toml"))
    }

    pub fn create(&self, name: &str, description: &str) -> ZotResult<Workspace> {
        ensure_workspace_name(name)?;
        let ws = Workspace {
            name: name.to_string(),
            created: Utc::now().to_rfc3339(),
            description: description.to_string(),
            items: Vec::new(),
        };
        self.save(&ws)?;
        Ok(ws)
    }

    pub fn save(&self, workspace: &Workspace) -> ZotResult<()> {
        std::fs::create_dir_all(&self.root).map_err(|source| ZotError::Io {
            path: self.root.clone(),
            source,
        })?;
        let data = WorkspaceFile {
            created: workspace.created.clone(),
            description: workspace.description.clone(),
            items: workspace.items.clone(),
        };
        let raw = toml::to_string_pretty(&data).map_err(|err| ZotError::Database {
            code: "workspace-serialize".to_string(),
            message: err.to_string(),
            hint: None,
        })?;
        let path = self.path_for(&workspace.name);
        std::fs::write(&path, raw).map_err(|source| ZotError::Io { path, source })
    }

    pub fn load(&self, name: &str) -> ZotResult<Workspace> {
        let path = self.path_for(name);
        let raw = std::fs::read_to_string(&path).map_err(|source| ZotError::Io {
            path: path.clone(),
            source,
        })?;
        let parsed: WorkspaceFile = toml::from_str(&raw).map_err(|err| ZotError::Database {
            code: "workspace-parse".to_string(),
            message: err.to_string(),
            hint: None,
        })?;
        Ok(Workspace {
            name: name.to_string(),
            created: parsed.created,
            description: parsed.description,
            items: parsed.items,
        })
    }

    pub fn delete(&self, name: &str) -> ZotResult<()> {
        let path = self.path_for(name);
        std::fs::remove_file(&path).map_err(|source| ZotError::Io { path, source })
    }

    pub fn list(&self) -> ZotResult<Vec<Workspace>> {
        if !self.root.exists() {
            return Ok(Vec::new());
        }
        let mut workspaces = Vec::new();
        for entry in std::fs::read_dir(&self.root).map_err(|source| ZotError::Io {
            path: self.root.clone(),
            source,
        })? {
            let entry = entry.map_err(|source| ZotError::Io {
                path: self.root.clone(),
                source,
            })?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) == Some("toml")
                && let Some(name) = path.file_stem().and_then(|stem| stem.to_str())
                && let Ok(workspace) = self.load(name)
            {
                workspaces.push(workspace);
            }
        }
        workspaces.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(workspaces)
    }

    pub fn add_items(&self, workspace: &mut Workspace, items: &[Item]) -> usize {
        let mut added = 0;
        for item in items {
            if workspace.items.iter().any(|entry| entry.key == item.key) {
                continue;
            }
            workspace.items.push(WorkspaceItem {
                key: item.key.clone(),
                title: item.title.clone(),
                added: Utc::now().to_rfc3339(),
            });
            added += 1;
        }
        added
    }

    pub fn remove_keys(&self, workspace: &mut Workspace, keys: &[String]) -> usize {
        let before = workspace.items.len();
        workspace.items.retain(|entry| !keys.contains(&entry.key));
        before.saturating_sub(workspace.items.len())
    }
}

pub struct RagIndex {
    conn: Connection,
}

impl RagIndex {
    pub fn open(path: impl AsRef<Path>) -> ZotResult<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| ZotError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        let conn = Connection::open(&path).map_err(|err| ZotError::Database {
            code: "rag-open".to_string(),
            message: err.to_string(),
            hint: None,
        })?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS chunks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                item_key TEXT NOT NULL,
                source TEXT NOT NULL,
                content TEXT NOT NULL,
                embedding TEXT
            );
            CREATE TABLE IF NOT EXISTS bm25_terms (
                term TEXT NOT NULL,
                chunk_id INTEGER NOT NULL,
                tf REAL NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_chunks_item ON chunks(item_key);
            CREATE INDEX IF NOT EXISTS idx_terms_term ON bm25_terms(term);
            CREATE TABLE IF NOT EXISTS index_meta (
                key TEXT PRIMARY KEY,
                value TEXT
            );",
        )
        .map_err(|err| ZotError::Database {
            code: "rag-schema".to_string(),
            message: err.to_string(),
            hint: None,
        })?;
        Ok(Self { conn })
    }

    pub fn clear(&self) -> ZotResult<()> {
        self.conn
            .execute_batch("DELETE FROM bm25_terms; DELETE FROM chunks; DELETE FROM index_meta;")
            .map_err(|err| ZotError::Database {
                code: "rag-clear".to_string(),
                message: err.to_string(),
                hint: None,
            })?;
        Ok(())
    }

    pub fn insert_chunk(&self, item_key: &str, source: &str, content: &str) -> ZotResult<i64> {
        self.conn
            .execute(
                "INSERT INTO chunks (item_key, source, content) VALUES (?1, ?2, ?3)",
                params![item_key, source, content],
            )
            .map_err(|err| ZotError::Database {
                code: "rag-insert-chunk".to_string(),
                message: err.to_string(),
                hint: None,
            })?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn insert_terms(&self, chunk_id: i64, terms: &HashMap<String, f32>) -> ZotResult<()> {
        let tx = self
            .conn
            .unchecked_transaction()
            .map_err(|err| ZotError::Database {
                code: "rag-terms-tx".to_string(),
                message: err.to_string(),
                hint: None,
            })?;
        for (term, tf) in terms {
            tx.execute(
                "INSERT INTO bm25_terms (term, chunk_id, tf) VALUES (?1, ?2, ?3)",
                params![term, chunk_id, tf],
            )
            .map_err(|err| ZotError::Database {
                code: "rag-insert-term".to_string(),
                message: err.to_string(),
                hint: None,
            })?;
        }
        tx.commit().map_err(|err| ZotError::Database {
            code: "rag-terms-commit".to_string(),
            message: err.to_string(),
            hint: None,
        })?;
        Ok(())
    }

    pub fn set_embedding(&self, chunk_id: i64, embedding: &[f32]) -> ZotResult<()> {
        let raw = serde_json::to_string(embedding).map_err(|err| ZotError::Database {
            code: "rag-embedding-serialize".to_string(),
            message: err.to_string(),
            hint: None,
        })?;
        self.conn
            .execute(
                "UPDATE chunks SET embedding = ?1 WHERE id = ?2",
                params![raw, chunk_id],
            )
            .map_err(|err| ZotError::Database {
                code: "rag-set-embedding".to_string(),
                message: err.to_string(),
                hint: None,
            })?;
        Ok(())
    }

    pub fn set_meta(&self, key: &str, value: &str) -> ZotResult<()> {
        self.conn
            .execute(
                "INSERT OR REPLACE INTO index_meta (key, value) VALUES (?1, ?2)",
                params![key, value],
            )
            .map_err(|err| ZotError::Database {
                code: "rag-set-meta".to_string(),
                message: err.to_string(),
                hint: None,
            })?;
        Ok(())
    }

    pub fn get_meta(&self, key: &str) -> ZotResult<Option<String>> {
        self.conn
            .query_row(
                "SELECT value FROM index_meta WHERE key = ?1",
                params![key],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(db_err("rag-get-meta"))
    }

    pub fn indexed_keys(&self) -> ZotResult<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT DISTINCT item_key FROM chunks")
            .map_err(db_err("rag-indexed-keys"))?;
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(db_err("rag-indexed-keys"))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(db_err("rag-indexed-keys"))
    }

    pub fn chunk_count(&self) -> ZotResult<usize> {
        self.conn
            .query_row("SELECT COUNT(*) FROM chunks", [], |row| {
                row.get::<_, i64>(0)
            })
            .map(|count| count as usize)
            .map_err(db_err("rag-chunk-count"))
    }

    pub fn embedding_count(&self) -> ZotResult<usize> {
        self.conn
            .query_row(
                "SELECT COUNT(*) FROM chunks WHERE embedding IS NOT NULL AND embedding != ''",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map(|count| count as usize)
            .map_err(db_err("rag-embedding-count"))
    }

    pub fn query(
        &self,
        question: &str,
        mode: HybridMode,
        embedding: Option<&[f32]>,
        limit: usize,
    ) -> ZotResult<Vec<QueryChunk>> {
        let bm25 = self.score_bm25(question)?;
        let semantic = if matches!(mode, HybridMode::Semantic | HybridMode::Hybrid) {
            embedding
                .map(|values| self.score_semantic(values))
                .transpose()?
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        let merged = match mode {
            HybridMode::Bm25 => bm25,
            HybridMode::Semantic => semantic,
            HybridMode::Hybrid => reciprocal_rank_fusion(&bm25, &semantic),
        };

        Ok(merged.into_iter().take(limit).collect())
    }

    fn score_bm25(&self, question: &str) -> ZotResult<Vec<QueryChunk>> {
        let query_terms = tokenize(question);
        if query_terms.is_empty() {
            return Ok(Vec::new());
        }
        let chunks = self.load_chunks()?;
        let avg_doc_len = if chunks.is_empty() {
            1.0
        } else {
            chunks
                .iter()
                .map(|chunk| tokenize(&chunk.content).len() as f32)
                .sum::<f32>()
                / chunks.len() as f32
        };
        let mut df = HashMap::new();
        for term in &query_terms {
            let count = self
                .conn
                .query_row(
                    "SELECT COUNT(DISTINCT chunk_id) FROM bm25_terms WHERE term = ?1",
                    params![term],
                    |row| row.get::<_, i64>(0),
                )
                .optional()
                .map_err(db_err("rag-bm25-df"))?
                .unwrap_or(0);
            df.insert(term.clone(), count as f32);
        }

        let total_docs = chunks.len() as f32;
        let mut scored = Vec::new();
        for chunk in chunks {
            let doc_len = tokenize(&chunk.content).len() as f32;
            let terms = self.load_terms(chunk.id)?;
            let mut score = 0.0_f32;
            for term in &query_terms {
                let Some(df_value) = df.get(term) else {
                    continue;
                };
                if *df_value == 0.0 {
                    continue;
                }
                let tf = *terms.get(term).unwrap_or(&0.0);
                let idf = ((total_docs - *df_value + 0.5) / (*df_value + 0.5) + 1.0).ln();
                score +=
                    idf * (tf * 2.5) / (tf + 1.5 * (1.0 - 0.75 + 0.75 * doc_len / avg_doc_len));
            }
            if score > 0.0 {
                scored.push(QueryChunk {
                    item_key: chunk.item_key,
                    source: chunk.source,
                    score,
                    content: chunk.content,
                });
            }
        }
        scored.sort_by(|left, right| {
            right
                .score
                .partial_cmp(&left.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        Ok(scored)
    }

    fn score_semantic(&self, embedding: &[f32]) -> ZotResult<Vec<QueryChunk>> {
        let chunks = self.load_chunks()?;
        let mut scored = Vec::new();
        for chunk in chunks {
            if let Some(chunk_embedding) = chunk.embedding.as_deref() {
                let score = cosine_similarity(embedding, chunk_embedding);
                scored.push(QueryChunk {
                    item_key: chunk.item_key,
                    source: chunk.source,
                    score,
                    content: chunk.content,
                });
            }
        }
        scored.sort_by(|left, right| {
            right
                .score
                .partial_cmp(&left.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        Ok(scored)
    }

    fn load_chunks(&self) -> ZotResult<Vec<ChunkRow>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, item_key, source, content, embedding FROM chunks")
            .map_err(db_err("rag-load-chunks"))?;
        let rows = stmt
            .query_map([], |row| {
                let embedding_raw = row.get::<_, Option<String>>(4)?;
                let embedding =
                    embedding_raw.and_then(|raw| serde_json::from_str::<Vec<f32>>(&raw).ok());
                Ok(ChunkRow {
                    id: row.get::<_, i64>(0)?,
                    item_key: row.get::<_, String>(1)?,
                    source: row.get::<_, String>(2)?,
                    content: row.get::<_, String>(3)?,
                    embedding,
                })
            })
            .map_err(db_err("rag-load-chunks"))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(db_err("rag-load-chunks"))
    }

    fn load_terms(&self, chunk_id: i64) -> ZotResult<HashMap<String, f32>> {
        let mut stmt = self
            .conn
            .prepare("SELECT term, tf FROM bm25_terms WHERE chunk_id = ?1")
            .map_err(db_err("rag-load-terms"))?;
        let rows = stmt
            .query_map(params![chunk_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, f32>(1)?))
            })
            .map_err(db_err("rag-load-terms"))?;
        let mut terms = HashMap::new();
        for row in rows {
            let (term, tf) = row.map_err(db_err("rag-load-terms"))?;
            terms.insert(term, tf);
        }
        Ok(terms)
    }
}

#[derive(Debug)]
struct ChunkRow {
    id: i64,
    item_key: String,
    source: String,
    content: String,
    embedding: Option<Vec<f32>>,
}

pub fn tokenize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split_whitespace()
        .map(|word| word.trim_matches(|ch: char| !ch.is_alphanumeric()))
        .filter(|word| !word.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

pub fn compute_term_frequencies(tokens: &[String]) -> HashMap<String, f32> {
    let total = tokens.len() as f32;
    let mut counts = HashMap::new();
    if total == 0.0 {
        return counts;
    }
    for token in tokens {
        *counts.entry(token.clone()).or_insert(0.0) += 1.0;
    }
    for value in counts.values_mut() {
        *value /= total;
    }
    counts
}

pub fn build_metadata_chunk(item: &Item) -> String {
    let authors = item
        .creators
        .iter()
        .map(|creator| creator.full_name())
        .collect::<Vec<_>>()
        .join(", ");
    let mut parts = vec![
        format!("Title: {}", item.title),
        format!("Authors: {authors}"),
    ];
    if let Some(abstract_note) = item.abstract_note.as_deref() {
        parts.push(format!("Abstract: {abstract_note}"));
    }
    if !item.tags.is_empty() {
        parts.push(format!("Tags: {}", item.tags.join(", ")));
    }
    parts.join("\n")
}

pub fn chunk_text(text: &str, paper_title: &str, max_tokens: usize, overlap: usize) -> Vec<String> {
    let words = text.split_whitespace().collect::<Vec<_>>();
    if words.is_empty() {
        return Vec::new();
    }
    let step = max_tokens.saturating_sub(overlap).max(1);
    let mut chunks = Vec::new();
    let mut index = 0;
    while index < words.len() {
        let end = usize::min(index + max_tokens, words.len());
        let chunk = words[index..end].join(" ");
        chunks.push(format!("[{paper_title}] {chunk}"));
        if end == words.len() {
            break;
        }
        index += step;
    }
    chunks
}

fn default_workspaces_dir() -> PathBuf {
    zot_core::AppConfig::config_dir().join("workspaces")
}

fn ensure_workspace_name(name: &str) -> ZotResult<()> {
    let valid =
        regex::Regex::new(r"^[a-z0-9]+(-[a-z0-9]+)*$").map_err(|err| ZotError::InvalidInput {
            code: "workspace-regex".to_string(),
            message: err.to_string(),
            hint: None,
        })?;
    if valid.is_match(name) {
        Ok(())
    } else {
        Err(ZotError::InvalidInput {
            code: "invalid-workspace-name".to_string(),
            message: format!("Invalid workspace name: {name}"),
            hint: Some("Use kebab-case such as llm-safety".to_string()),
        })
    }
}

fn reciprocal_rank_fusion(left: &[QueryChunk], right: &[QueryChunk]) -> Vec<QueryChunk> {
    let mut scores: BTreeMap<(String, String, String), f32> = BTreeMap::new();
    let mut chunks = BTreeMap::new();
    for (rank, chunk) in left.iter().enumerate() {
        let key = (
            chunk.item_key.clone(),
            chunk.source.clone(),
            chunk.content.clone(),
        );
        *scores.entry(key.clone()).or_insert(0.0) += 1.0 / (60.0 + rank as f32 + 1.0);
        chunks.insert(key, chunk.clone());
    }
    for (rank, chunk) in right.iter().enumerate() {
        let key = (
            chunk.item_key.clone(),
            chunk.source.clone(),
            chunk.content.clone(),
        );
        *scores.entry(key.clone()).or_insert(0.0) += 1.0 / (60.0 + rank as f32 + 1.0);
        chunks.insert(key, chunk.clone());
    }
    let mut merged = scores
        .into_iter()
        .filter_map(|(key, score)| {
            chunks.get(&key).cloned().map(|mut chunk| {
                chunk.score = score;
                chunk
            })
        })
        .collect::<Vec<_>>();
    merged.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    merged
}

fn cosine_similarity(left: &[f32], right: &[f32]) -> f32 {
    if left.len() != right.len() || left.is_empty() {
        return 0.0;
    }
    let dot = left
        .iter()
        .zip(right.iter())
        .map(|(a, b)| a * b)
        .sum::<f32>();
    let left_norm = left.iter().map(|value| value * value).sum::<f32>().sqrt();
    let right_norm = right.iter().map(|value| value * value).sum::<f32>().sqrt();
    if left_norm == 0.0 || right_norm == 0.0 {
        0.0
    } else {
        dot / (left_norm * right_norm)
    }
}

fn db_err(code: &'static str) -> impl Fn(rusqlite::Error) -> ZotError {
    move |err| ZotError::Database {
        code: code.to_string(),
        message: err.to_string(),
        hint: None,
    }
}
