use rusqlite::{Connection, params};
use tempfile::tempdir;
use zot_local::{HybridMode, RagIndex, compute_term_frequencies, tokenize};

fn insert_chunk_with_embedding(
    index: &RagIndex,
    item_key: &str,
    source: &str,
    content: &str,
    embedding: &[f32],
) -> i64 {
    let chunk_id = index
        .insert_chunk(item_key, source, content)
        .expect("insert chunk");
    let terms = compute_term_frequencies(&tokenize(content));
    index
        .insert_terms(chunk_id, &terms)
        .expect("insert terms");
    index
        .set_embedding(chunk_id, embedding)
        .expect("set embedding");
    chunk_id
}

#[test]
fn blob_embedding_round_trips_across_reopen() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("blob.idx.sqlite");

    let sample = vec![0.125_f32, -0.5, 1.0, 42.0, -0.0001];

    {
        let index = RagIndex::open(&path).expect("open v2");
        insert_chunk_with_embedding(&index, "ATTN001", "metadata", "transformer", &sample);
    }

    let index = RagIndex::open(&path).expect("reopen v2");
    assert_eq!(index.embedding_count().expect("embedding count"), 1);

    let hits = index
        .query("transformer", HybridMode::Semantic, Some(&sample), 10)
        .expect("semantic query");
    assert_eq!(hits.len(), 1, "embedding must survive BLOB round trip");
    assert_eq!(hits[0].item_key, "ATTN001");
    assert!(
        hits[0].score > 0.99,
        "self-similarity should be ~1.0, got {}",
        hits[0].score
    );
}

#[test]
fn legacy_text_embedding_migrates_to_blob_on_open() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("legacy.idx.sqlite");

    // Build a v1 schema manually: `embedding` as TEXT storing JSON.
    {
        let conn = Connection::open(&path).expect("open legacy");
        conn.execute_batch(
            "CREATE TABLE chunks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                item_key TEXT NOT NULL,
                source TEXT NOT NULL,
                content TEXT NOT NULL,
                embedding TEXT
            );
            CREATE TABLE bm25_terms (
                term TEXT NOT NULL,
                chunk_id INTEGER NOT NULL,
                tf REAL NOT NULL
            );
            CREATE INDEX idx_chunks_item ON chunks(item_key);
            CREATE INDEX idx_terms_term ON bm25_terms(term);
            CREATE TABLE index_meta (
                key TEXT PRIMARY KEY,
                value TEXT
            );",
        )
        .expect("legacy schema");
        let legacy_vec = vec![0.25_f32, 0.5, -0.75];
        let legacy_json = serde_json::to_string(&legacy_vec).expect("json");
        conn.execute(
            "INSERT INTO chunks (item_key, source, content, embedding)
             VALUES (?1, ?2, ?3, ?4)",
            params!["LGCY001", "metadata", "legacy sample", legacy_json],
        )
        .expect("seed legacy row");
        // Seed BM25 term so bm25 path finds the chunk.
        conn.execute(
            "INSERT INTO bm25_terms (term, chunk_id, tf) VALUES ('legacy', 1, 1.0)",
            [],
        )
        .expect("seed bm25 term");
    }

    // Re-open through the new code path: migration must run.
    let index = RagIndex::open(&path).expect("migrate legacy");
    assert_eq!(index.embedding_count().expect("embedding count"), 1);
    assert_eq!(index.chunk_count().expect("chunk count"), 1);

    // Query with the same vector; migrated BLOB should yield ~self-similarity.
    let probe = vec![0.25_f32, 0.5, -0.75];
    let hits = index
        .query("legacy", HybridMode::Semantic, Some(&probe), 10)
        .expect("semantic query post-migration");
    assert_eq!(hits.len(), 1, "migrated embedding should be recoverable");
    assert_eq!(hits[0].item_key, "LGCY001");
    assert!(
        hits[0].score > 0.99,
        "migrated self-similarity near 1.0, got {}",
        hits[0].score
    );

    // Column declaration must now be BLOB so writes round-trip binary bytes.
    let conn = Connection::open(&path).expect("reopen post-migration");
    let mut stmt = conn
        .prepare("PRAGMA table_info(chunks)")
        .expect("pragma");
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(1)?, row.get::<_, String>(2)?))
        })
        .expect("pragma rows");
    let mut saw_blob = false;
    for row in rows {
        let (name, decl) = row.expect("pragma row");
        if name == "embedding" {
            assert!(
                decl.eq_ignore_ascii_case("BLOB"),
                "embedding column should be BLOB after migration, got {decl}"
            );
            saw_blob = true;
        }
    }
    assert!(saw_blob, "migrated schema must still expose embedding column");
}

#[test]
fn with_write_tx_wraps_bulk_inserts_in_single_commit() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("tx.idx.sqlite");
    let index = RagIndex::open(&path).expect("open");

    let embeddings: Vec<Vec<f32>> = (0..5)
        .map(|idx| vec![idx as f32, idx as f32 + 0.5, -(idx as f32)])
        .collect();

    index
        .with_write_tx(|| {
            for (i, emb) in embeddings.iter().enumerate() {
                let content = format!("chunk-{i} body text");
                let chunk_id = index.insert_chunk("BULK001", "metadata", &content)?;
                index.insert_terms(
                    chunk_id,
                    &compute_term_frequencies(&tokenize(&content)),
                )?;
                index.set_embedding(chunk_id, emb)?;
            }
            Ok(())
        })
        .expect("bulk write tx");

    assert_eq!(
        index.chunk_count().expect("chunk count"),
        embeddings.len(),
        "all chunks committed"
    );
    assert_eq!(
        index.embedding_count().expect("embedding count"),
        embeddings.len(),
        "embeddings attached inside same tx"
    );
}

#[test]
fn bm25_average_doc_len_is_cached_after_first_query() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("cache.idx.sqlite");
    let index = RagIndex::open(&path).expect("open");

    let contents = ["alpha", "beta gamma", "delta epsilon zeta"];
    for (idx, content) in contents.iter().enumerate() {
        let chunk_id = index
            .insert_chunk(&format!("C{idx:03}"), "metadata", content)
            .expect("insert");
        index
            .insert_terms(chunk_id, &compute_term_frequencies(&tokenize(content)))
            .expect("terms");
    }

    // First query triggers compute + cache write.
    let _ = index
        .query("alpha", HybridMode::Bm25, None, 10)
        .expect("first bm25 query");

    // Cache must now carry avg_doc_len.
    let cached = index
        .get_meta("bm25.avg_doc_len")
        .expect("get meta")
        .expect("cached avg after first query");
    let value = cached
        .parse::<f32>()
        .expect("cached avg should parse as f32");
    assert!(value > 0.0);

    // Mutating the corpus invalidates the cache.
    let chunk_id = index
        .insert_chunk("C999", "metadata", "extra long content for new chunk")
        .expect("insert invalidator");
    index
        .insert_terms(
            chunk_id,
            &compute_term_frequencies(&tokenize("extra long content for new chunk")),
        )
        .expect("terms invalidator");
    let post_invalidation = index
        .get_meta("bm25.avg_doc_len")
        .expect("get meta post insert");
    assert!(
        post_invalidation.is_none(),
        "cache must be invalidated after insert_chunk"
    );

    // Remove should also invalidate.
    let _ = index
        .query("alpha", HybridMode::Bm25, None, 10)
        .expect("second bm25 query");
    assert!(
        index
            .get_meta("bm25.avg_doc_len")
            .expect("get meta")
            .is_some()
    );
    index
        .remove_item_chunks("C999")
        .expect("remove invalidator");
    assert!(
        index
            .get_meta("bm25.avg_doc_len")
            .expect("get meta post remove")
            .is_none(),
        "cache must be invalidated after remove_item_chunks"
    );
}
