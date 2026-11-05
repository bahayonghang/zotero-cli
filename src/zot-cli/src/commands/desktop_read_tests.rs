use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::Result;
use rusqlite::{Connection, params};
use serde_json::{Value, json};
use tiny_http::{Header, Response as FakeResponse, Server};
use zot_core::{AnnotationSnippet, AppConfig, LibraryScope, PdfOutlineEntry, ZotError};
use zot_desktop::{DesktopScanLimits, LocalApiClient};
use zot_local::{
    PdfAreaPosition, PdfBackend, PdfMatchPosition, SearchOptions, SortDirection, SortField,
};
use zot_remote::{BetterBibTexClient, HttpRuntime};

use super::{
    ReadIo, identity_explicit, read_children, read_citekey, read_item, read_pdf, read_search,
};
use crate::context::AppContext;
use crate::output::CommandOutput;

struct Spec {
    id: i64,
    library: i64,
    key: &'static str,
    kind: &'static str,
    title: &'static str,
    abstract_note: Option<&'static str>,
    date: Option<&'static str>,
    cite: Option<&'static str>,
    publication: Option<&'static str>,
    abbr: Option<&'static str>,
    extra: Option<&'static str>,
    tag: Option<&'static str>,
    creator: Option<(&'static str, &'static str)>,
    parent: Option<&'static str>,
    path: Option<&'static str>,
    content_type: Option<&'static str>,
    note: Option<&'static str>,
    deleted: bool,
    fulltext: Option<&'static str>,
}

fn catalog() -> Vec<Spec> {
    vec![
        Spec {
            abstract_note: Some("transformers are enough"),
            publication: Some("Neural Computation"),
            tag: Some("transformer"),
            creator: Some(("Ashish", "Vaswani")),
            date: Some("2017"),
            ..item(
                1,
                1,
                "ATTN001",
                "Attention Is All You Need",
                Some("Smith2024"),
            )
        },
        Spec {
            abstract_note: Some("optimization"),
            publication: Some("Nature"),
            abbr: Some("Nature"),
            date: Some("2016"),
            tag: Some("learning"),
            ..item(2, 1, "DEEP002", "Deep  Learning", Some("Deep2016"))
        },
        Spec {
            date: Some("2018"),
            ..item(
                3,
                1,
                "DUPE003",
                "Attention Is All You Need",
                Some("Other2020"),
            )
        },
        item(4, 1, "PART004", "Attention Fragment", None),
        Spec {
            publication: Some("Quarterly Review of Biology"),
            abbr: Some("Q Rev Biol"),
            ..item(5, 1, "JFULL005", "Journal Full", None)
        },
        Spec {
            abstract_note: Some("Quarterly Review of Biology is mentioned"),
            publication: Some("Science"),
            ..item(6, 1, "JMESS006", "Abstract Mention", None)
        },
        Spec {
            abstract_note: Some("zeta signal"),
            ..item(7, 1, "TOPA007", "Topic Abstract", None)
        },
        Spec {
            tag: Some("zeta"),
            ..item(8, 1, "TOPT008", "Topic Tag", None)
        },
        item(9, 1, "BODY009", "Body Parent", None),
        Spec {
            deleted: true,
            ..item(10, 1, "TRSH010", "Trashed Attention Paper", None)
        },
        Spec {
            extra: Some("Citation Key: ExtraOnly"),
            ..item(11, 1, "EXTRA011", "Extra Only", None)
        },
        Spec {
            extra: Some("Citation Key: BothKey"),
            ..item(12, 1, "BOTH012", "Both Sources", Some("BothKey"))
        },
        Spec {
            creator: Some(("Ada", "Lovelace")),
            date: Some("2001"),
            ..item(13, 1, "DUPA015", "Dup A", Some("DupKey"))
        },
        Spec {
            creator: Some(("Grace", "Hopper")),
            date: Some("2002"),
            ..item(14, 1, "DUPB016", "Dup B", Some("DupKey"))
        },
        Spec {
            kind: "note",
            parent: Some("ATTN001"),
            note: Some("Hello"),
            ..item(15, 1, "NOTE015", "", None)
        },
        Spec {
            kind: "attachment",
            parent: Some("ATTN001"),
            path: Some("storage:paper.pdf"),
            content_type: Some("application/pdf"),
            ..item(16, 1, "ATCH016", "", None)
        },
        Spec {
            kind: "attachment",
            parent: Some("BODY009"),
            path: Some("storage:body.pdf"),
            content_type: Some("application/pdf"),
            fulltext: Some("zzbodyonly"),
            ..item(17, 1, "BODY017", "", None)
        },
        Spec {
            title: "User Paper",
            cite: Some("Alpha"),
            ..item(18, 1, "SAME0001", "User Paper", Some("Alpha"))
        },
        Spec {
            library: 2,
            title: "Group Only Paper",
            cite: Some("Beta"),
            ..item(19, 2, "SAME0001", "Group Only Paper", Some("Beta"))
        },
    ]
}

fn item(
    id: i64,
    library: i64,
    key: &'static str,
    title: &'static str,
    cite: Option<&'static str>,
) -> Spec {
    Spec {
        id,
        library,
        key,
        kind: "journalArticle",
        title,
        abstract_note: None,
        date: None,
        cite,
        publication: None,
        abbr: None,
        extra: None,
        tag: None,
        creator: None,
        parent: None,
        path: None,
        content_type: None,
        note: None,
        deleted: false,
        fulltext: None,
    }
}

fn seed_sqlite(dir: &Path) {
    let conn = Connection::open(dir.join("zotero.sqlite")).expect("create sqlite");
    conn.execute_batch(
        "
        CREATE TABLE libraries (libraryID INTEGER PRIMARY KEY, type TEXT NOT NULL);
        INSERT INTO libraries VALUES (1, 'user');
        INSERT INTO libraries VALUES (2, 'group');
        CREATE TABLE groups (groupID INTEGER PRIMARY KEY, libraryID INT NOT NULL, name TEXT NOT NULL, description TEXT NOT NULL DEFAULT '', version INT NOT NULL DEFAULT 1);
        INSERT INTO groups VALUES (99999, 2, 'Lab Group', '', 1);
        CREATE TABLE itemTypes (itemTypeID INTEGER PRIMARY KEY, typeName TEXT NOT NULL);
        INSERT INTO itemTypes VALUES (1, 'journalArticle');
        INSERT INTO itemTypes VALUES (2, 'attachment');
        INSERT INTO itemTypes VALUES (3, 'note');
        CREATE TABLE fields (fieldID INTEGER PRIMARY KEY, fieldName TEXT NOT NULL);
        INSERT INTO fields VALUES (1, 'title');
        INSERT INTO fields VALUES (2, 'abstractNote');
        INSERT INTO fields VALUES (3, 'date');
        INSERT INTO fields VALUES (4, 'extra');
        INSERT INTO fields VALUES (5, 'citationKey');
        INSERT INTO fields VALUES (6, 'publicationTitle');
        INSERT INTO fields VALUES (7, 'journalAbbreviation');
        CREATE TABLE items (
            itemID INTEGER PRIMARY KEY,
            itemTypeID INT NOT NULL,
            dateAdded TEXT,
            dateModified TEXT,
            clientDateModified TEXT,
            libraryID INT NOT NULL,
            key TEXT NOT NULL,
            UNIQUE (libraryID, key)
        );
        CREATE TABLE itemData (itemID INT NOT NULL, fieldID INT NOT NULL, valueID INT NOT NULL, PRIMARY KEY (itemID, fieldID));
        CREATE TABLE itemDataValues (valueID INTEGER PRIMARY KEY, value TEXT NOT NULL);
        CREATE TABLE creators (creatorID INTEGER PRIMARY KEY, firstName TEXT, lastName TEXT NOT NULL);
        CREATE TABLE creatorTypes (creatorTypeID INTEGER PRIMARY KEY, creatorType TEXT NOT NULL);
        INSERT INTO creatorTypes VALUES (1, 'author');
        CREATE TABLE itemCreators (itemID INT NOT NULL, creatorID INT NOT NULL, creatorTypeID INT NOT NULL DEFAULT 1, orderIndex INT NOT NULL DEFAULT 0, PRIMARY KEY (itemID, creatorID, creatorTypeID, orderIndex));
        CREATE TABLE tags (tagID INTEGER PRIMARY KEY, name TEXT NOT NULL);
        CREATE TABLE itemTags (itemID INT NOT NULL, tagID INT NOT NULL, type INT NOT NULL DEFAULT 0, PRIMARY KEY (itemID, tagID));
        CREATE TABLE collections (collectionID INTEGER PRIMARY KEY, collectionName TEXT, parentCollectionID INT, libraryID INT, key TEXT);
        CREATE TABLE collectionItems (collectionID INT, itemID INT, orderIndex INT);
        CREATE TABLE deletedItems (itemID INTEGER PRIMARY KEY, dateDeleted TEXT NOT NULL);
        CREATE TABLE itemNotes (itemID INT PRIMARY KEY, parentItemID INT, note TEXT, title TEXT);
        CREATE TABLE itemAttachments (itemID INT PRIMARY KEY, parentItemID INT, linkMode INT, contentType TEXT, charsetID INT, path TEXT);
        CREATE TABLE itemAnnotations (itemID INT PRIMARY KEY, parentItemID INT, type INT, text TEXT, comment TEXT, color TEXT, pageLabel TEXT);
        CREATE TABLE fulltextWords (wordID INTEGER PRIMARY KEY, word TEXT NOT NULL UNIQUE);
        CREATE TABLE fulltextItemWords (wordID INT NOT NULL, itemID INT NOT NULL, PRIMARY KEY (wordID, itemID));
        CREATE TABLE version (schema TEXT PRIMARY KEY, version INT NOT NULL);
        INSERT INTO version VALUES ('userdata', 120);
        CREATE TABLE settings (setting TEXT NOT NULL, key TEXT NOT NULL, value TEXT, PRIMARY KEY (setting, key));
        INSERT INTO settings VALUES ('localAPI', 'serverID', 'server-abc');
        ",
    )
    .expect("schema");
    let mut value_id = 1_i64;
    let mut creator_id = 1_i64;
    let mut tag_id = 1_i64;
    let mut word_id = 1_i64;
    for spec in catalog() {
        let type_id: i64 = match spec.kind {
            "attachment" => 2,
            "note" => 3,
            _ => 1,
        };
        conn.execute(
            "INSERT INTO items VALUES (?1, ?2, '2024-01-01 00:00:00', '2024-01-02 00:00:00', '2024-01-02 00:00:00', ?3, ?4)",
            params![spec.id, type_id, spec.library, spec.key],
        )
        .expect("item");
        let mut field = |name: &str, value: &str| {
            let field_id: i64 = conn
                .query_row(
                    "SELECT fieldID FROM fields WHERE fieldName = ?1",
                    [name],
                    |row| row.get(0),
                )
                .expect("field");
            conn.execute(
                "INSERT INTO itemDataValues VALUES (?1, ?2)",
                params![value_id, value],
            )
            .expect("value");
            conn.execute(
                "INSERT INTO itemData VALUES (?1, ?2, ?3)",
                params![spec.id, field_id, value_id],
            )
            .expect("data");
            value_id += 1;
        };
        if !spec.title.is_empty() {
            field("title", spec.title);
        }
        if let Some(value) = spec.abstract_note {
            field("abstractNote", value);
        }
        if let Some(value) = spec.date {
            field("date", value);
        }
        if let Some(value) = spec.cite {
            field("citationKey", value);
        }
        if let Some(value) = spec.publication {
            field("publicationTitle", value);
        }
        if let Some(value) = spec.abbr {
            field("journalAbbreviation", value);
        }
        if let Some(value) = spec.extra {
            field("extra", value);
        }
        if let Some((first, last)) = spec.creator {
            conn.execute(
                "INSERT INTO creators VALUES (?1, ?2, ?3)",
                params![creator_id, first, last],
            )
            .expect("creator");
            conn.execute(
                "INSERT INTO itemCreators VALUES (?1, ?2, 1, 0)",
                params![spec.id, creator_id],
            )
            .expect("item creator");
            creator_id += 1;
        }
        if let Some(tag) = spec.tag {
            conn.execute("INSERT INTO tags VALUES (?1, ?2)", params![tag_id, tag])
                .expect("tag");
            conn.execute(
                "INSERT INTO itemTags VALUES (?1, ?2, 0)",
                params![spec.id, tag_id],
            )
            .expect("item tag");
            tag_id += 1;
        }
        if spec.deleted {
            conn.execute(
                "INSERT INTO deletedItems VALUES (?1, '2024-02-01 00:00:00')",
                [spec.id],
            )
            .expect("trash");
        }
        if let Some(parent) = spec.parent {
            let parent_id: i64 = conn
                .query_row(
                    "SELECT itemID FROM items WHERE key = ?1 AND libraryID = ?2",
                    params![parent, spec.library],
                    |row| row.get(0),
                )
                .expect("parent");
            if spec.kind == "note" {
                conn.execute(
                    "INSERT INTO itemNotes VALUES (?1, ?2, ?3, NULL)",
                    params![spec.id, parent_id, spec.note.unwrap_or("")],
                )
                .expect("note");
            } else {
                conn.execute(
                    "INSERT INTO itemAttachments VALUES (?1, ?2, 0, ?3, NULL, ?4)",
                    params![
                        spec.id,
                        parent_id,
                        spec.content_type.unwrap_or(""),
                        spec.path.unwrap_or("")
                    ],
                )
                .expect("attachment");
            }
        }
        if let Some(word) = spec.fulltext {
            conn.execute(
                "INSERT INTO fulltextWords VALUES (?1, ?2)",
                params![word_id, word],
            )
            .expect("word");
            conn.execute(
                "INSERT INTO fulltextItemWords VALUES (?1, ?2)",
                params![word_id, spec.id],
            )
            .expect("word link");
            word_id += 1;
        }
    }
}

fn spec_json(spec: &Spec) -> Value {
    let mut data = serde_json::Map::new();
    data.insert("key".to_string(), json!(spec.key));
    data.insert("itemType".to_string(), json!(spec.kind));
    data.insert("title".to_string(), json!(spec.title));
    if let Some(value) = spec.abstract_note {
        data.insert("abstractNote".to_string(), json!(value));
    }
    if let Some(value) = spec.date {
        data.insert("date".to_string(), json!(value));
    }
    if let Some(value) = spec.cite {
        data.insert("citationKey".to_string(), json!(value));
    }
    if let Some(value) = spec.publication {
        data.insert("publicationTitle".to_string(), json!(value));
    }
    if let Some(value) = spec.abbr {
        data.insert("journalAbbreviation".to_string(), json!(value));
    }
    if let Some(value) = spec.extra {
        data.insert("extra".to_string(), json!(value));
    }
    if let Some(value) = spec.note {
        data.insert("note".to_string(), json!(value));
    }
    if let Some(value) = spec.path {
        data.insert("path".to_string(), json!(value));
    }
    if let Some(value) = spec.content_type {
        data.insert("contentType".to_string(), json!(value));
    }
    if let Some(parent) = spec.parent {
        data.insert("parentItem".to_string(), json!(parent));
    }
    if let Some((first, last)) = spec.creator {
        data.insert(
            "creators".to_string(),
            json!([{"creatorType": "author", "firstName": first, "lastName": last}]),
        );
    } else {
        data.insert("creators".to_string(), json!([]));
    }
    if let Some(tag) = spec.tag {
        data.insert("tags".to_string(), json!([{"tag": tag}]));
    } else {
        data.insert("tags".to_string(), json!([]));
    }
    data.insert("collections".to_string(), json!([]));
    data.insert("dateAdded".to_string(), json!("2024-01-01 00:00:00"));
    data.insert("dateModified".to_string(), json!("2024-01-02 00:00:00"));
    if spec.deleted {
        data.insert("deleted".to_string(), json!(true));
    }
    json!({"key": spec.key, "data": data})
}

#[derive(Clone, Copy)]
enum Mode {
    Serve,
    ProbeForbidden,
    ProbeRedirect,
    ItemMissing,
    ItemConflict,
    ChildrenMissing,
    FileRedirect,
    FileUnsupported,
    ItemRedirect,
    Paged,
    VersionFlip,
}

struct Hit {
    method: String,
    url: String,
    headers: Vec<(String, String)>,
}

struct Running {
    base: String,
    hits: Arc<Mutex<Vec<Hit>>>,
    stop: Arc<AtomicBool>,
}

impl Drop for Running {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

fn serve(mode: Mode, file_url: &str) -> Running {
    let http = Server::http("127.0.0.1:0").expect("server");
    let base = format!("http://{}/", http.server_addr());
    let hits = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&hits);
    let stop = Arc::new(AtomicBool::new(false));
    let running = Arc::clone(&stop);
    let file_url = file_url.to_string();
    let user: Vec<Value> = catalog()
        .iter()
        .filter(|spec| spec.library == 1)
        .map(spec_json)
        .collect();
    let group: Vec<Value> = catalog()
        .iter()
        .filter(|spec| spec.library == 2)
        .map(spec_json)
        .collect();
    let steps = Arc::new(AtomicUsize::new(0));
    std::thread::spawn(move || {
        while !running.load(Ordering::SeqCst) {
            let Ok(Some(request)) = http.recv_timeout(Duration::from_millis(200)) else {
                continue;
            };
            let url = request.url().to_string();
            let method = request.method().to_string();
            let headers = request
                .headers()
                .iter()
                .map(|header| {
                    (
                        header.field.to_string().to_ascii_lowercase(),
                        header.value.to_string(),
                    )
                })
                .collect();
            recorded.lock().expect("hits").push(Hit {
                method,
                url: url.clone(),
                headers,
            });
            let (status, body, extra) = reply(mode, &url, &user, &group, &file_url, &steps);
            let mut response = FakeResponse::from_string(body).with_status_code(status);
            for (name, value) in standard_headers(mode, &url, &steps)
                .into_iter()
                .chain(extra)
            {
                response.add_header(Header::from_bytes(name, value).expect("header"));
            }
            let _ = request.respond(response);
        }
    });
    Running { base, hits, stop }
}

fn standard_headers(mode: Mode, url: &str, steps: &AtomicUsize) -> Vec<(&'static str, String)> {
    let version = if matches!(mode, Mode::VersionFlip) && !is_probe(url) {
        match steps.fetch_add(1, Ordering::SeqCst) {
            0 => "1",
            1 => "2",
            _ => "5",
        }
    } else {
        "5"
    };
    vec![
        ("X-Zotero-Version", "10.0.7".to_string()),
        ("Zotero-API-Version", "3".to_string()),
        ("Zotero-Schema-Version", "36".to_string()),
        ("Zotero-Server-ID", "server-abc".to_string()),
        ("Last-Modified-Version", version.to_string()),
    ]
}

fn is_probe(url: &str) -> bool {
    url.trim_end_matches('/') == "/api"
}

fn reply(
    mode: Mode,
    url: &str,
    user: &[Value],
    group: &[Value],
    file_url: &str,
    _steps: &AtomicUsize,
) -> (u16, String, Vec<(&'static str, String)>) {
    if is_probe(url) {
        return match mode {
            Mode::ProbeForbidden => (403, String::new(), Vec::new()),
            Mode::ProbeRedirect => (
                302,
                String::new(),
                vec![("Location", "http://example.com/api/".to_string())],
            ),
            _ => (200, "{}".to_string(), Vec::new()),
        };
    }
    if matches!(mode, Mode::ItemConflict) && url.contains("/items/") && !url.contains('?') {
        return (412, String::new(), Vec::new());
    }
    if matches!(mode, Mode::ItemRedirect)
        && url.contains("/items/")
        && !url.contains('?')
        && !url.contains("/children")
    {
        return (
            302,
            String::new(),
            vec![(
                "Location",
                "http://example.com/api/users/0/items/ATTN001".to_string(),
            )],
        );
    }
    if matches!(mode, Mode::ItemMissing) && item_key(url).is_some() && !url.contains("/children") {
        return (404, "{}".to_string(), Vec::new());
    }
    if url.contains("/file/view/url") {
        return match mode {
            Mode::FileRedirect => (
                302,
                String::new(),
                vec![("Location", "https://example.com/paper.pdf".to_string())],
            ),
            Mode::FileUnsupported => (405, String::new(), Vec::new()),
            _ => (200, file_url.to_string(), Vec::new()),
        };
    }
    if url.contains("/children") {
        if matches!(mode, Mode::ChildrenMissing) {
            return (404, "[]".to_string(), Vec::new());
        }
        let key = url
            .split("/items/")
            .nth(1)
            .and_then(|rest| rest.split('/').next())
            .unwrap_or("");
        let rows: Vec<&Value> = user
            .iter()
            .filter(|row| row["data"]["parentItem"] == key)
            .collect();
        return (
            200,
            serde_json::to_string(&rows).expect("children"),
            Vec::new(),
        );
    }
    if let Some(key) = item_key(url) {
        let rows = if url.contains("/groups/") {
            group
        } else {
            user
        };
        let found = rows.iter().find(|row| row["key"] == key);
        return match found {
            Some(row) => (200, row.to_string(), Vec::new()),
            None => (404, "{}".to_string(), Vec::new()),
        };
    }
    if url.contains("/items?") {
        let rows = if url.contains("/groups/") {
            group
        } else {
            user
        };
        if matches!(mode, Mode::Paged) {
            let start = query_u64(url, "start").unwrap_or(0) as usize;
            let page = rows.get(start).cloned().into_iter().collect::<Vec<_>>();
            let mut extra = vec![("Total-Results", rows.len().to_string())];
            if start + 1 < rows.len() {
                extra.push((
                    "Link",
                    format!("</api/users/0/items?start={}>; rel=\"next\"", start + 1),
                ));
            }
            return (200, serde_json::to_string(&page).expect("page"), extra);
        }
        return (200, serde_json::to_string(rows).expect("items"), Vec::new());
    }
    (404, "{}".to_string(), Vec::new())
}

fn item_key(url: &str) -> Option<&str> {
    let rest = url.split("/items/").nth(1)?;
    if rest.contains('?') || rest.contains('/') {
        return None;
    }
    Some(rest)
}

fn query_u64(url: &str, name: &str) -> Option<u64> {
    url.split('?').nth(1)?.split('&').find_map(|pair| {
        let (key, value) = pair.split_once('=')?;
        if key == name {
            value.parse().ok()
        } else {
            None
        }
    })
}

fn client(base: &str) -> LocalApiClient {
    LocalApiClient::with_base_url_for_tests(base, Duration::from_secs(2)).expect("client")
}

fn closed_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("addr").port();
    drop(listener);
    port
}

fn io_for(
    dir: &Path,
    api: Option<LocalApiClient>,
    explicit: bool,
    running: bool,
    scope: LibraryScope,
) -> (ReadIo, PathBuf) {
    let binding = std::env::temp_dir().join(format!(
        "zot-desktop-read-{}-{}.json",
        std::process::id(),
        unique()
    ));
    let io = ReadIo {
        explicit,
        running_zotero: running,
        data_dir: Some(dir.to_path_buf()),
        binding_path: binding.clone(),
        scope,
        api,
        bbt: None,
        opens: Arc::new(AtomicUsize::new(0)),
        limits: DesktopScanLimits::default(),
    };
    (io, binding)
}

fn file_url(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    let mut encoded = String::from("file:///");
    for ch in text.chars() {
        match ch {
            '/' | ':' | '.' | '-' | '_' | 'A'..='Z' | 'a'..='z' | '0'..='9' => encoded.push(ch),
            _ => {
                let mut buf = [0_u8; 4];
                for byte in ch.encode_utf8(&mut buf).as_bytes() {
                    encoded.push_str(&format!("%{byte:02X}"));
                }
            }
        }
    }
    encoded
}

fn unique() -> u64 {
    static NEXT: AtomicUsize = AtomicUsize::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed) as u64
}

fn ctx(pdf_fail: bool) -> AppContext {
    AppContext {
        json: true,
        profile: None,
        scope: LibraryScope::User,
        config: AppConfig::default(),
        http: Arc::new(HttpRuntime::new().expect("http")),
        pdf: Arc::new(PageBackend { fail: pdf_fail }),
    }
}

struct PageBackend {
    fail: bool,
}

impl PdfBackend for PageBackend {
    fn availability_hint(&self) -> zot_core::ZotResult<()> {
        Ok(())
    }
    fn extract_text(
        &self,
        pdf_path: &Path,
        page_range: Option<(usize, usize)>,
    ) -> zot_core::ZotResult<String> {
        if self.fail {
            return Err(zot_core::ZotError::Pdf {
                code: "pdf-test".to_string(),
                message: "pdfium failed".to_string(),
                hint: None,
            });
        }
        let bytes = std::fs::read(pdf_path).map_err(|source| zot_core::ZotError::Io {
            path: pdf_path.to_path_buf(),
            source,
        })?;
        let text = String::from_utf8_lossy(&bytes);
        if page_range == Some((1, 1)) && text.contains("FIXED-PAGE-1-TEXT") {
            return Ok("FIXED-PAGE-1-TEXT".to_string());
        }
        Ok(text.into_owned())
    }
    fn extract_annotations(&self, _: &Path) -> zot_core::ZotResult<Vec<AnnotationSnippet>> {
        Ok(Vec::new())
    }
    fn extract_outline(&self, _: &Path) -> zot_core::ZotResult<Vec<PdfOutlineEntry>> {
        Ok(Vec::new())
    }
    fn find_text_position(
        &self,
        _: &Path,
        _: usize,
        _: &str,
        _: usize,
    ) -> zot_core::ZotResult<Option<PdfMatchPosition>> {
        Ok(None)
    }
    fn build_area_position(
        &self,
        _: &Path,
        _: usize,
        _: f32,
        _: f32,
        _: f32,
        _: f32,
    ) -> zot_core::ZotResult<PdfAreaPosition> {
        Err(zot_core::ZotError::Pdf {
            code: "pdf-test".to_string(),
            message: "unused".to_string(),
            hint: None,
        })
    }
}

fn envelope(output: CommandOutput) -> Value {
    serde_json::from_str(output.as_json().expect("json")).expect("envelope")
}

fn error_code(result: Result<CommandOutput>) -> String {
    result
        .expect_err("error")
        .downcast_ref::<ZotError>()
        .expect("zot error")
        .payload()
        .code
}

fn keys_of(value: &Value) -> Vec<String> {
    value["data"]
        .as_array()
        .expect("items")
        .iter()
        .map(|item| item["key"].as_str().expect("key").to_string())
        .collect()
}

fn assert_read_only(hits: &[Hit]) {
    assert!(hits.iter().all(|hit| hit.method == "GET"));
    assert!(hits.iter().all(|hit| !hit.url.contains("authorize")));
    assert!(
        hits.iter()
            .all(|hit| { hit.headers.iter().all(|(name, _)| name != "zotero-api-key") })
    );
}

fn hold_exclusive(path: &Path) -> Connection {
    let conn = Connection::open(path).expect("exclusive");
    conn.execute_batch("PRAGMA locking_mode=EXCLUSIVE; BEGIN EXCLUSIVE; COMMIT;")
        .expect("hold lock");
    conn
}

fn library_dir() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("temp");
    seed_sqlite(dir.path());
    dir
}

fn search_options(query: &str) -> SearchOptions {
    SearchOptions {
        query: query.to_string(),
        sort: Some(SortField::Title),
        direction: SortDirection::Asc,
        ..SearchOptions::default()
    }
}

#[test]
fn identity_explicit_uses_only_the_supplied_flags() {
    assert!(!identity_explicit(None, "", false));
    assert!(identity_explicit(Some("work"), "", false));
    assert!(identity_explicit(None, " D:\\Zotero ", false));
    assert!(identity_explicit(None, "", true));
    assert!(!identity_explicit(None, "   ", false));
}

#[tokio::test]
async fn desktop_hit_skips_snapshot_even_when_sqlite_is_locked() {
    let dir = library_dir();
    let _lock = hold_exclusive(&dir.path().join("zotero.sqlite"));
    let running = serve(Mode::Serve, "");
    let (io, _) = io_for(
        dir.path(),
        Some(client(&running.base)),
        false,
        false,
        LibraryScope::User,
    );
    let value = envelope(
        read_item(&ctx(false), &io, "ATTN001")
            .await
            .expect("desktop item"),
    );
    assert_eq!(value["meta"]["read_source"], "desktop");
    assert_eq!(value["data"]["item"]["key"], "ATTN001");
    assert_eq!(value["data"]["item"]["title"], "Attention Is All You Need");
    assert_eq!(value["data"]["notes"][0]["content"], "Hello");
    assert_eq!(value["data"]["attachments"][0]["filename"], "paper.pdf");
    assert_eq!(io.opens.load(Ordering::SeqCst), 0);
    let hits = running.hits.lock().expect("hits");
    assert_read_only(&hits);
    assert!(hits.iter().any(|hit| hit.url.contains("/items/ATTN001")));
    assert!(hits.iter().any(|hit| is_probe(&hit.url)));
    assert!(
        hits.iter()
            .filter(|hit| !is_probe(&hit.url))
            .all(|hit| hit.url.contains("/users/0/"))
    );
}

#[tokio::test]
async fn closed_and_open_reads_resolve_the_same_fixture() {
    let dir = library_dir();
    let running = serve(Mode::Serve, "");
    let (open_io, _) = io_for(
        dir.path(),
        Some(client(&running.base)),
        false,
        false,
        LibraryScope::User,
    );
    let closed_api = LocalApiClient::with_base_url_for_tests(
        &format!("http://127.0.0.1:{}/", closed_port()),
        Duration::from_secs(2),
    )
    .expect("closed client");
    let (closed_io, _) = io_for(
        dir.path(),
        Some(closed_api),
        false,
        false,
        LibraryScope::User,
    );
    let context = ctx(false);
    let mut open_options = search_options("");
    open_options.title = Some("attention".to_string());
    let closed_options = open_options.clone();
    let open_search = envelope(
        read_search(&context, &open_io, open_options)
            .await
            .expect("open search"),
    );
    let closed_search = envelope(
        read_search(&context, &closed_io, closed_options)
            .await
            .expect("closed search"),
    );
    assert_eq!(open_search["meta"]["read_source"], "desktop");
    assert_eq!(closed_search["meta"]["read_source"], "snapshot");
    assert_eq!(keys_of(&open_search), keys_of(&closed_search));
    assert_eq!(open_io.opens.load(Ordering::SeqCst), 0);
    assert!(closed_io.opens.load(Ordering::SeqCst) >= 1);
    assert!(open_search["meta"].get("fulltext_index").is_none());

    let open_item = envelope(
        read_item(&context, &open_io, "ATTN001")
            .await
            .expect("open item"),
    );
    let closed_item = envelope(
        read_item(&context, &closed_io, "ATTN001")
            .await
            .expect("closed item"),
    );
    assert_eq!(
        open_item["data"]["item"]["key"],
        closed_item["data"]["item"]["key"]
    );
    assert_eq!(
        open_item["data"]["item"]["title"],
        closed_item["data"]["item"]["title"]
    );
    assert_eq!(open_item["meta"]["read_source"], "desktop");
    assert_eq!(closed_item["meta"]["read_source"], "snapshot");
}

#[tokio::test]
async fn search_applies_fields_before_the_page_and_skips_body_only_hits() {
    let dir = library_dir();
    let running = serve(Mode::Paged, "");
    let (mut io, _) = io_for(
        dir.path(),
        Some(client(&running.base)),
        false,
        false,
        LibraryScope::User,
    );
    io.limits.page_size = 1;
    io.limits.max_pages = 20;
    let context = ctx(false);
    let mut title = search_options("");
    title.title = Some("  DEEP   learning ".to_string());
    let found = envelope(read_search(&context, &io, title).await.expect("title"));
    assert_eq!(keys_of(&found), vec!["DEEP002".to_string()]);
    assert_eq!(found["meta"]["total"], 1);
    assert!(found["meta"].get("fulltext_index").is_none());

    let mut dup = search_options("");
    dup.title = Some("Attention Is All You Need".to_string());
    let dups = envelope(read_search(&context, &io, dup).await.expect("duplicates"));
    assert_eq!(
        keys_of(&dups),
        vec!["ATTN001".to_string(), "DUPE003".to_string()]
    );

    let mut partial = search_options("");
    partial.title = Some("fragment".to_string());
    assert_eq!(
        keys_of(&envelope(
            read_search(&context, &io, partial).await.expect("partial")
        )),
        vec!["PART004".to_string()]
    );

    let mut journal = search_options("");
    journal.journal = Some("quarterly review of biology".to_string());
    assert_eq!(
        keys_of(&envelope(
            read_search(&context, &io, journal).await.expect("journal")
        )),
        vec!["JFULL005".to_string()]
    );
    let mut abbr = search_options("");
    abbr.journal = Some("q rev biol".to_string());
    assert_eq!(
        keys_of(&envelope(
            read_search(&context, &io, abbr).await.expect("abbr")
        )),
        vec!["JFULL005".to_string()]
    );

    let mut topic = search_options("");
    topic.topic = Some("zeta".to_string());
    let topics = keys_of(&envelope(
        read_search(&context, &io, topic).await.expect("topic"),
    ));
    assert_eq!(topics, vec!["TOPA007".to_string(), "TOPT008".to_string()]);
    assert!(!topics.iter().any(|key| key == "BODY009"));

    let mut ordered = search_options("");
    ordered.title = Some("dup".to_string());
    assert_eq!(
        keys_of(&envelope(
            read_search(&context, &io, ordered).await.expect("order")
        )),
        vec!["DUPA015".to_string(), "DUPB016".to_string()]
    );

    let mut trashed = search_options("");
    trashed.title = Some("Trashed Attention".to_string());
    let hidden = envelope(
        read_search(&context, &io, trashed)
            .await
            .expect("trash excluded"),
    );
    assert!(!keys_of(&hidden).iter().any(|key| key == "TRSH010"));
    assert_eq!(io.opens.load(Ordering::SeqCst), 0);
    assert_read_only(&running.hits.lock().expect("hits"));

    io.limits.max_pages = 1;
    let mut incomplete = search_options("");
    incomplete.title = Some("attention".to_string());
    assert_eq!(
        error_code(read_search(&context, &io, incomplete).await),
        "desktop-search-incomplete"
    );
}

#[tokio::test]
async fn generic_query_keeps_sqlite_fulltext_and_does_not_mix_sources() {
    let dir = library_dir();
    let running = serve(Mode::Serve, "");
    let (io, _) = io_for(
        dir.path(),
        Some(client(&running.base)),
        false,
        false,
        LibraryScope::User,
    );
    let found = envelope(
        read_search(&ctx(false), &io, search_options("zzbodyonly"))
            .await
            .expect("body"),
    );
    assert_eq!(found["meta"]["read_source"], "snapshot");
    assert_eq!(keys_of(&found), vec!["BODY009".to_string()]);
    assert_eq!(found["meta"]["fulltext_index"], "legacy-tables");
    assert!(running.hits.lock().expect("hits").is_empty());
    assert_eq!(io.opens.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn locked_generic_query_uses_desktop_metadata_without_claiming_body_hits() {
    let dir = library_dir();
    let _lock = hold_exclusive(&dir.path().join("zotero.sqlite"));
    let running = serve(Mode::Serve, "");
    let (io, _) = io_for(
        dir.path(),
        Some(client(&running.base)),
        false,
        false,
        LibraryScope::User,
    );
    let found = envelope(
        read_search(&ctx(false), &io, search_options("zzbodyonly"))
            .await
            .expect("desktop metadata"),
    );
    assert_eq!(found["meta"]["read_source"], "desktop");
    assert_eq!(found["meta"]["fulltext_index"], "unavailable");
    assert!(keys_of(&found).is_empty());
    assert!(io.opens.load(Ordering::SeqCst) >= 1);
    assert_read_only(&running.hits.lock().expect("hits"));
}

#[tokio::test]
async fn identity_failures_do_not_fall_through() {
    let dir = library_dir();
    let running = serve(Mode::Serve, "");
    let (mut mismatch, binding) = io_for(
        dir.path(),
        Some(client(&running.base)),
        true,
        false,
        LibraryScope::User,
    );
    std::fs::write(
        &binding,
        serde_json::json!({
            "bindings": [{"data_dir": dir.path().display().to_string(), "server_id": "other-server"}]
        })
        .to_string(),
    )
    .expect("binding");
    assert_eq!(
        error_code(read_item(&ctx(false), &mismatch, "ATTN001").await),
        "desktop-library-mismatch"
    );
    assert_eq!(mismatch.opens.load(Ordering::SeqCst), 0);
    {
        let hits = running.hits.lock().expect("hits");
        assert!(hits.iter().all(|hit| is_probe(&hit.url)));
    }

    mismatch.running_zotero = true;
    let unbound = envelope(
        read_item(&ctx(false), &mismatch, "ATTN001")
            .await
            .expect("opt in"),
    );
    assert_eq!(unbound["meta"]["read_source"], "desktop-unbound");
    assert_eq!(mismatch.opens.load(Ordering::SeqCst), 0);

    let (group_io, _) = io_for(
        dir.path(),
        Some(client(&running.base)),
        false,
        false,
        LibraryScope::Group { group_id: 99999 },
    );
    let group = envelope(
        read_item(&ctx(false), &group_io, "SAME0001")
            .await
            .expect("group"),
    );
    assert_eq!(group["data"]["item"]["title"], "Group Only Paper");
    assert!(
        running
            .hits
            .lock()
            .expect("hits")
            .iter()
            .any(|hit| hit.url.contains("/groups/99999/items/SAME0001"))
    );

    let missing = serve(Mode::ItemMissing, "");
    let (missing_io, _) = io_for(
        dir.path(),
        Some(client(&missing.base)),
        false,
        false,
        LibraryScope::User,
    );
    assert_eq!(
        error_code(read_item(&ctx(false), &missing_io, "ATTN001").await),
        "item-not-found"
    );
    assert_eq!(missing_io.opens.load(Ordering::SeqCst), 0);
    assert!(
        missing
            .hits
            .lock()
            .expect("hits")
            .iter()
            .all(|hit| !hit.url.contains("/children"))
    );

    let conflict = serve(Mode::ItemConflict, "");
    let (conflict_io, _) = io_for(
        dir.path(),
        Some(client(&conflict.base)),
        false,
        false,
        LibraryScope::User,
    );
    assert_eq!(
        error_code(read_item(&ctx(false), &conflict_io, "ATTN001").await),
        "local-api-conflict"
    );
    assert_eq!(conflict_io.opens.load(Ordering::SeqCst), 0);

    let redirected = serve(Mode::ProbeRedirect, "");
    let (redirect_io, _) = io_for(
        dir.path(),
        Some(client(&redirected.base)),
        false,
        false,
        LibraryScope::User,
    );
    let snapped = envelope(
        read_item(&ctx(false), &redirect_io, "ATTN001")
            .await
            .expect("snapshot after redirect"),
    );
    assert_eq!(snapped["meta"]["read_source"], "snapshot");
    assert_eq!(redirected.hits.lock().expect("hits").len(), 1);

    let disabled = serve(Mode::ProbeForbidden, "");
    let (disabled_io, _) = io_for(
        dir.path(),
        Some(client(&disabled.base)),
        false,
        false,
        LibraryScope::User,
    );
    let after_disabled = envelope(
        read_item(&ctx(false), &disabled_io, "ATTN001")
            .await
            .expect("snapshot after 403"),
    );
    assert_eq!(after_disabled["meta"]["read_source"], "snapshot");

    let hard = serve(Mode::FileRedirect, "");
    let (hard_io, _) = io_for(
        dir.path(),
        Some(client(&hard.base)),
        false,
        false,
        LibraryScope::User,
    );
    assert_eq!(
        error_code(read_item(&ctx(false), &hard_io, "NOPE0000").await),
        "item-not-found"
    );
}

#[tokio::test]
async fn committed_redirect_and_unreadable_snapshot_are_not_mixed() {
    let dir = library_dir();
    let running = serve(Mode::ItemConflict, "");
    let (io, _) = io_for(
        dir.path(),
        Some(client(&running.base)),
        false,
        false,
        LibraryScope::User,
    );
    assert_eq!(
        error_code(read_pdf(&ctx(false), &io, "ATTN001", None, false).await),
        "local-api-conflict"
    );
    assert_eq!(io.opens.load(Ordering::SeqCst), 0);

    let redirect = serve(Mode::FileRedirect, "");
    let (redirect_io, _) = io_for(
        dir.path(),
        Some(client(&redirect.base)),
        false,
        false,
        LibraryScope::User,
    );
    assert_eq!(
        error_code(read_pdf(&ctx(false), &redirect_io, "ATTN001", Some("1"), false).await),
        "local-api-redirect"
    );
    assert_eq!(redirect_io.opens.load(Ordering::SeqCst), 0);
    assert!(
        redirect
            .hits
            .lock()
            .expect("hits")
            .iter()
            .all(|hit| !hit.url.contains("example.com"))
    );

    let unsupported = serve(Mode::FileUnsupported, "");
    let (unsupported_io, _) = io_for(
        dir.path(),
        Some(client(&unsupported.base)),
        false,
        false,
        LibraryScope::User,
    );
    assert_eq!(
        error_code(read_pdf(&ctx(false), &unsupported_io, "ATTN001", None, false).await),
        "desktop-file-url-unsupported"
    );

    let blocked = dir.path().join("not-a-directory");
    std::fs::write(&blocked, b"x").expect("file");
    let port = closed_port();
    let (blocked_io, _) = io_for(
        &blocked,
        Some(
            LocalApiClient::with_base_url_for_tests(
                &format!("http://127.0.0.1:{port}/"),
                Duration::from_secs(2),
            )
            .expect("client"),
        ),
        false,
        false,
        LibraryScope::User,
    );
    assert_eq!(
        error_code(read_item(&ctx(false), &blocked_io, "ATTN001").await),
        "library-read-unavailable"
    );
    assert!(blocked_io.opens.load(Ordering::SeqCst) >= 1);
}

#[tokio::test]
async fn pdf_reads_local_files_and_stops_on_bad_files() {
    let dir = library_dir();
    let stored = dir.path().join("storage").join("ATCH016");
    std::fs::create_dir_all(&stored).expect("storage");
    let stored_pdf = stored.join("paper.pdf");
    std::fs::write(&stored_pdf, b"%PDF-1.4\nFIXED-PAGE-1-TEXT\n").expect("pdf");
    let linked_dir = std::env::temp_dir().join(format!("zot-论文-{}", std::process::id()));
    std::fs::create_dir_all(&linked_dir).expect("linked dir");
    let linked = linked_dir.join("论文.pdf");
    std::fs::write(&linked, b"%PDF-1.4\nFIXED-PAGE-1-TEXT\n").expect("linked");
    let stored_url = file_url(&stored_pdf);
    let running = serve(Mode::Serve, &stored_url);
    let (io, _) = io_for(
        dir.path(),
        Some(client(&running.base)),
        false,
        false,
        LibraryScope::User,
    );
    let context = ctx(false);
    let page = envelope(
        read_pdf(&context, &io, "ATTN001", Some("1"), false)
            .await
            .expect("page"),
    );
    assert_eq!(page["data"]["text"], "FIXED-PAGE-1-TEXT");
    assert_eq!(page["meta"]["read_source"], "desktop");
    assert_eq!(io.opens.load(Ordering::SeqCst), 0);

    let linked_url = file_url(&linked);
    let linked_server = serve(Mode::Serve, &linked_url);
    let (linked_io, _) = io_for(
        dir.path(),
        Some(client(&linked_server.base)),
        false,
        false,
        LibraryScope::User,
    );
    let linked_text = envelope(
        read_pdf(&context, &linked_io, "ATTN001", Some("1"), false)
            .await
            .expect("linked"),
    );
    assert_eq!(linked_text["data"]["text"], "FIXED-PAGE-1-TEXT");

    let missing = dir.path().join("missing").join("no-such-论文.pdf");
    let missing_server = serve(Mode::Serve, &file_url(&missing));
    let (missing_io, _) = io_for(
        dir.path(),
        Some(client(&missing_server.base)),
        false,
        false,
        LibraryScope::User,
    );
    assert_eq!(
        error_code(read_pdf(&context, &missing_io, "ATTN001", None, false).await),
        "desktop-file-missing"
    );

    let bad = dir.path().join("notes.txt");
    std::fs::write(&bad, b"hello").expect("text");
    let bad_url = file_url(&bad);
    let bad_server = serve(Mode::Serve, &bad_url);
    let (bad_io, _) = io_for(
        dir.path(),
        Some(client(&bad_server.base)),
        false,
        false,
        LibraryScope::User,
    );
    assert_eq!(
        error_code(read_pdf(&context, &bad_io, "ATTN001", None, false).await),
        "desktop-file-type"
    );

    let failing = ctx(true);
    assert_eq!(
        error_code(read_pdf(&failing, &io, "ATTN001", Some("1"), false).await),
        "pdf-test"
    );
    let _ = std::fs::remove_dir_all(&linked_dir);
}

#[tokio::test]
async fn citation_keys_stay_exact_and_inside_one_library() {
    let dir = library_dir();
    let running = serve(Mode::Serve, "");
    let (mut io, _) = io_for(
        dir.path(),
        Some(client(&running.base)),
        false,
        false,
        LibraryScope::User,
    );
    let context = ctx(false);
    let exact = envelope(
        read_citekey(&context, &io, "Smith2024")
            .await
            .expect("cite"),
    );
    assert_eq!(exact["data"]["item"]["key"], "ATTN001");
    assert_eq!(exact["data"]["source"], "citation-key");
    assert_eq!(exact["data"]["citekey"], "Smith2024");
    assert_eq!(io.opens.load(Ordering::SeqCst), 0);

    let both = envelope(read_citekey(&context, &io, "BothKey").await.expect("both"));
    assert_eq!(both["data"]["source"], "citation-key");
    let extra = envelope(
        read_citekey(&context, &io, "ExtraOnly")
            .await
            .expect("extra"),
    );
    assert_eq!(extra["data"]["source"], "extra");
    assert_eq!(extra["data"]["citekey"], "ExtraOnly");

    assert_eq!(
        error_code(read_citekey(&context, &io, "smith2024").await),
        "citation-key-source-limited"
    );
    assert_eq!(
        error_code(read_citekey(&context, &io, "Smith").await),
        "citation-key-source-limited"
    );
    let ambiguous = read_citekey(&context, &io, "DupKey")
        .await
        .expect_err("ambiguous");
    let ambiguous = ambiguous.downcast_ref::<ZotError>().expect("zot error");
    assert_eq!(ambiguous.payload().code, "citation-key-ambiguous");
    assert!(ambiguous.payload().message.contains("DUPA015"));
    assert!(ambiguous.payload().message.contains("DUPB016"));
    assert!(ambiguous.payload().message.contains("Dup A"));
    assert!(ambiguous.payload().message.contains("Dup B"));

    let (empty_base, _) = bbt_server(json!([]));
    io.bbt = Some(BetterBibTexClient::with_base_url_for_tests(
        context.http(),
        &empty_base,
    ));
    assert_eq!(
        error_code(read_citekey(&context, &io, "NotStored").await),
        "citation-key-not-found"
    );

    let (bbt_base, bbt_hits) = bbt_server(json!([{
        "citekey": "Beta",
        "itemKey": "SAME0001",
        "libraryID": 2
    }]));
    io.bbt = Some(BetterBibTexClient::with_base_url_for_tests(
        context.http(),
        &bbt_base,
    ));
    assert_eq!(
        error_code(read_citekey(&context, &io, "Beta").await),
        "citation-key-unverified"
    );
    {
        let calls = bbt_hits.lock().expect("bbt");
        assert!(
            calls
                .iter()
                .all(|body| body.contains("\"method\":\"item.search\""))
        );
        assert!(
            running
                .hits
                .lock()
                .expect("hits")
                .iter()
                .all(|hit| !hit.url.contains("/groups/"))
        );
    }

    let (group_io, _) = io_for(
        dir.path(),
        Some(client(&running.base)),
        false,
        false,
        LibraryScope::Group { group_id: 99999 },
    );
    let group = envelope(
        read_citekey(&context, &group_io, "Beta")
            .await
            .expect("group cite"),
    );
    assert_eq!(group["data"]["item"]["title"], "Group Only Paper");
    assert_ne!(group["data"]["item"]["title"], "User Paper");

    let (closed, _) = io_for(dir.path(), None, false, false, LibraryScope::User);
    let snap = envelope(
        read_citekey(&context, &closed, "Alpha")
            .await
            .expect("snapshot cite"),
    );
    assert_eq!(snap["meta"]["read_source"], "snapshot");
    assert_eq!(snap["data"]["item"]["title"], "User Paper");
    assert_eq!(snap["data"]["source"], "citation-key");
    assert!(
        read_citekey(&context, &closed, "Beta")
            .await
            .expect_err("group key")
            .to_string()
            .contains("cannot be asked")
            || error_code(read_citekey(&context, &closed, "Beta").await)
                == "citation-key-source-limited"
    );
}

#[tokio::test]
async fn item_redirect_after_a_successful_probe_does_not_fall_through() {
    let dir = library_dir();
    let running = serve(Mode::ItemRedirect, "");
    let (io, _) = io_for(
        dir.path(),
        Some(client(&running.base)),
        false,
        false,
        LibraryScope::User,
    );
    assert_eq!(
        error_code(read_item(&ctx(false), &io, "ATTN001").await),
        "local-api-redirect"
    );
    assert_eq!(io.opens.load(Ordering::SeqCst), 0);
    let hits = running.hits.lock().expect("hits");
    assert!(hits.iter().any(|hit| is_probe(&hit.url)));
    assert!(hits.iter().any(|hit| hit.url.contains("/items/ATTN001")));
    assert!(hits.iter().all(|hit| !hit.url.contains("example.com")));
}

#[tokio::test]
async fn binding_file_round_trips_and_replaces_the_server_id() {
    let dir = library_dir();
    let (io, binding) = io_for(dir.path(), None, true, false, LibraryScope::User);
    let context = ctx(false);
    read_item(&context, &io, "ATTN001")
        .await
        .expect("first snapshot");
    let first = std::fs::read_to_string(&binding).expect("binding");
    assert!(first.contains("server-abc"));
    let conn = Connection::open(dir.path().join("zotero.sqlite")).expect("sqlite");
    conn.execute(
        "UPDATE settings SET value = 'server-next' WHERE setting = 'localAPI' AND key = 'serverID'",
        [],
    )
    .expect("update server id");
    drop(conn);
    read_item(&context, &io, "ATTN001")
        .await
        .expect("second snapshot");
    let second = std::fs::read_to_string(&binding).expect("updated binding");
    assert!(second.contains("server-next"));
    assert!(!second.contains("server-abc"));
    let _ = std::fs::remove_file(&binding);
}

#[tokio::test]
async fn failed_snapshot_and_refused_desktop_report_both_reasons() {
    let dir = library_dir();
    let _lock = hold_exclusive(&dir.path().join("zotero.sqlite"));
    let running = serve(Mode::Serve, "");
    let (io, binding) = io_for(
        dir.path(),
        Some(client(&running.base)),
        true,
        false,
        LibraryScope::User,
    );
    std::fs::write(
        &binding,
        serde_json::json!({
            "bindings": [{"data_dir": dir.path().display().to_string(), "server_id": "other-server"}]
        })
        .to_string(),
    )
    .expect("binding");
    let error = read_search(&ctx(false), &io, search_options("zzbodyonly"))
        .await
        .expect_err("both failed");
    let error = error.downcast_ref::<ZotError>().expect("zot error");
    assert_eq!(error.payload().code, "library-read-unavailable");
    assert!(error.payload().message.contains("does not match"));
    assert!(error.payload().message.contains("Snapshot read failed"));
    assert!(
        running
            .hits
            .lock()
            .expect("hits")
            .iter()
            .all(|hit| is_probe(&hit.url))
    );
    let _ = std::fs::remove_file(&binding);
}

#[tokio::test]
async fn children_of_a_missing_key_stay_empty_on_the_desktop_source() {
    let dir = library_dir();
    let running = serve(Mode::ChildrenMissing, "");
    let (io, _) = io_for(
        dir.path(),
        Some(client(&running.base)),
        false,
        false,
        LibraryScope::User,
    );
    let value = envelope(
        read_children(&ctx(false), &io, &["ATTN001".to_string()])
            .await
            .expect("children"),
    );
    assert_eq!(value["meta"]["read_source"], "desktop");
    assert!(
        value["data"]["ATTN001"]
            .as_array()
            .expect("list")
            .is_empty()
    );
    assert_eq!(io.opens.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn version_change_retries_without_opening_sqlite() {
    let dir = library_dir();
    let running = serve(Mode::VersionFlip, "");
    let (io, _) = io_for(
        dir.path(),
        Some(client(&running.base)),
        false,
        false,
        LibraryScope::User,
    );
    let value = envelope(read_item(&ctx(false), &io, "ATTN001").await.expect("retry"));
    assert_eq!(value["data"]["item"]["key"], "ATTN001");
    assert_eq!(io.opens.load(Ordering::SeqCst), 0);
    assert!(running.hits.lock().expect("hits").len() >= 4);
}

fn bbt_server(result: Value) -> (String, Arc<Mutex<Vec<String>>>) {
    let http = Server::http("127.0.0.1:0").expect("bbt");
    let base = format!("http://{}/better-bibtex", http.server_addr());
    let hits = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&hits);
    std::thread::spawn(move || {
        while let Ok(Some(mut request)) = http.recv_timeout(Duration::from_secs(3)) {
            let mut body = String::new();
            let _ = request.as_reader().read_to_string(&mut body);
            let url = request.url().to_string();
            if url.contains("probe=true") {
                let _ = request.respond(FakeResponse::from_string("ready"));
                continue;
            }
            recorded.lock().expect("bbt").push(body);
            let payload = json!({"jsonrpc": "2.0", "result": result, "id": 1});
            let _ = request.respond(FakeResponse::from_string(payload.to_string()));
        }
    });
    (base, hits)
}
