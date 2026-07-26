use once_cell::sync::Lazy;
use quick_xml::Reader;
use quick_xml::events::Event;
use regex::Regex;
use serde::Deserialize;
use zot_core::{ZotError, ZotResult};

use crate::http::{HttpRuntime, ensure_status, remote_err, send_with_retry};

const CONTACT_EMAIL_ENV: &str = "ZOT_CONTACT_EMAIL";
const DEFAULT_CONTACT_EMAIL: &str = "noreply@zot.local";

fn contact_email() -> String {
    std::env::var(CONTACT_EMAIL_ENV)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_CONTACT_EMAIL.to_string())
}

fn polite_user_agent() -> String {
    format!(
        "{} (mailto:{})",
        concat!("zot-cli/", env!("CARGO_PKG_VERSION")),
        contact_email()
    )
}

static DOI_URL_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"doi\.org/(10\.\d{4,9}/[^\s?#]+)").expect("valid DOI regex"));
static DOI_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^10\.\d{4,9}/\S+$").expect("valid DOI regex"));
static ARXIV_URL_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"arxiv\.org/(?:abs|pdf)/([0-9]{4}\.[0-9]{4,5}(?:v\d+)?|[a-z\-]+/\d{7}(?:v\d+)?)")
        .expect("valid arXiv regex")
});
static ARXIV_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^([0-9]{4}\.[0-9]{4,5}(?:v\d+)?|[a-z\-]+/\d{7}(?:v\d+)?)$")
        .expect("valid arXiv regex")
});
static ARXIV_DOI_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"arXiv\.(\d{4}\.\d{4,5}(?:v\d+)?)").expect("valid arXiv DOI regex"));

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreatorName {
    pub first_name: String,
    pub last_name: String,
    pub creator_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrossRefRelation {
    pub rel_type: String,
    pub id_type: Option<String>,
    pub id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrossRefWork {
    pub record_type: String,
    pub title: Option<String>,
    pub creators: Vec<CreatorName>,
    pub date: Option<String>,
    pub doi: String,
    pub url: Option<String>,
    pub volume: Option<String>,
    pub issue: Option<String>,
    pub pages: Option<String>,
    pub publisher: Option<String>,
    pub issn: Option<String>,
    pub publication_title: Option<String>,
    pub abstract_note: Option<String>,
    pub relations: Vec<CrossRefRelation>,
    pub alternative_ids: Vec<String>,
    pub links: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArxivWork {
    pub arxiv_id: String,
    pub title: String,
    pub abstract_note: Option<String>,
    pub date: Option<String>,
    pub creators: Vec<CreatorName>,
    pub abs_url: String,
    pub pdf_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedPdfUrl {
    pub source: String,
    pub url: String,
}

#[derive(Clone)]
pub struct OaClient {
    client: reqwest::Client,
    crossref_base: String,
    unpaywall_base: String,
    pmc_base: String,
    ss_base: String,
}

impl OaClient {
    pub fn new(runtime: &HttpRuntime) -> Self {
        Self {
            client: runtime.client_clone(),
            crossref_base: std::env::var("ZOT_CROSSREF_API_BASE")
                .unwrap_or_else(|_| "https://api.crossref.org".to_string()),
            unpaywall_base: std::env::var("ZOT_UNPAYWALL_API_BASE")
                .unwrap_or_else(|_| "https://api.unpaywall.org".to_string()),
            pmc_base: std::env::var("ZOT_PMC_API_BASE")
                .unwrap_or_else(|_| "https://pmc.ncbi.nlm.nih.gov".to_string()),
            ss_base: std::env::var("ZOT_SEMANTIC_SCHOLAR_GRAPH_BASE")
                .unwrap_or_else(|_| "https://api.semanticscholar.org/graph/v1".to_string()),
        }
    }

    pub async fn fetch_crossref_work(&self, doi: &str) -> ZotResult<CrossRefWork> {
        let response = send_with_retry(
            self.client
                .get(format!("{}/works/{}", self.crossref_base, doi))
                .header("Accept", "application/json")
                .header("User-Agent", polite_user_agent()),
            "crossref-request",
        )
        .await?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(ZotError::Remote {
                code: "crossref-not-found".to_string(),
                message: format!("DOI not found on CrossRef: {doi}"),
                hint: None,
                status: Some(404),
            });
        }
        let response = ensure_status(response, "crossref-http").await?;
        let payload: CrossRefEnvelope =
            response.json().await.map_err(remote_err("crossref-json"))?;
        Ok(payload.message.into_work(doi))
    }

    pub async fn fetch_arxiv_work(&self, arxiv_id: &str) -> ZotResult<ArxivWork> {
        let response = send_with_retry(
            self.client.get(format!(
                "https://export.arxiv.org/api/query?id_list={arxiv_id}"
            )),
            "arxiv-request",
        )
        .await?;
        let response = ensure_status(response, "arxiv-http").await?;
        let body = response.text().await.map_err(remote_err("arxiv-body"))?;
        parse_arxiv_atom(arxiv_id, &body)
    }

    pub async fn resolve_open_access_pdf(
        &self,
        doi: &str,
        crossref: Option<&CrossRefWork>,
    ) -> ZotResult<Option<ResolvedPdfUrl>> {
        if let Some(url) = self.try_unpaywall(doi).await? {
            return Ok(Some(ResolvedPdfUrl {
                source: "Unpaywall".to_string(),
                url,
            }));
        }
        if let Some(crossref) = crossref {
            if let Some(url) = try_arxiv_from_crossref(crossref) {
                return Ok(Some(ResolvedPdfUrl {
                    source: "arXiv (via CrossRef)".to_string(),
                    url,
                }));
            }
        }
        if let Some(url) = self.try_semantic_scholar(doi).await? {
            return Ok(Some(ResolvedPdfUrl {
                source: "Semantic Scholar".to_string(),
                url,
            }));
        }
        if let Some(url) = self.try_pmc(doi).await? {
            return Ok(Some(ResolvedPdfUrl {
                source: "PubMed Central".to_string(),
                url,
            }));
        }
        Ok(None)
    }

    async fn try_unpaywall(&self, doi: &str) -> ZotResult<Option<String>> {
        let response = send_with_retry(
            self.client.get(format!(
                "{}/v2/{}?email={}",
                self.unpaywall_base,
                doi,
                urlencoding::encode(&contact_email())
            )),
            "unpaywall-request",
        )
        .await?;
        if !response.status().is_success() {
            return Ok(None);
        }
        let payload: UnpaywallPayload = response
            .json()
            .await
            .map_err(remote_err("unpaywall-json"))?;
        if let Some(best) = payload
            .best_oa_location
            .as_ref()
            .and_then(|location| location.url_for_pdf.clone())
        {
            return Ok(Some(best));
        }
        for location in payload.oa_locations {
            if let Some(url) = location.url_for_pdf {
                return Ok(Some(url));
            }
        }
        Ok(payload
            .best_oa_location
            .and_then(|location| location.url)
            .or(None))
    }

    async fn try_semantic_scholar(&self, doi: &str) -> ZotResult<Option<String>> {
        let response = send_with_retry(
            self.client.get(format!(
                "{}/paper/DOI:{}?fields=openAccessPdf",
                self.ss_base, doi
            )),
            "ss-oa-request",
        )
        .await?;
        if !response.status().is_success() {
            return Ok(None);
        }
        let payload: SemanticScholarOaPayload =
            response.json().await.map_err(remote_err("ss-oa-json"))?;
        Ok(payload.open_access_pdf.and_then(|pdf| pdf.url))
    }

    async fn try_pmc(&self, doi: &str) -> ZotResult<Option<String>> {
        let response = send_with_retry(
            self.client.get(format!(
                "{}/tools/idconv/api/v1/articles/?ids={}&format=json&tool=zot&email={}",
                self.pmc_base,
                urlencoding::encode(doi),
                urlencoding::encode(&contact_email())
            )),
            "pmc-request",
        )
        .await?;
        if !response.status().is_success() {
            return Ok(None);
        }
        let payload: PmcPayload = response.json().await.map_err(remote_err("pmc-json"))?;
        Ok(payload
            .records
            .into_iter()
            .find_map(|record| record.pmcid)
            .map(|pmcid| format!("{}/articles/{pmcid}/pdf/", self.pmc_base)))
    }
}

pub fn normalize_doi(raw: &str) -> Option<String> {
    let mut value = raw.trim();
    if value.to_lowercase().starts_with("doi:") {
        value = value[4..].trim();
    }
    let normalized = if value.starts_with("http://") || value.starts_with("https://") {
        DOI_URL_RE
            .captures(value)
            .and_then(|captures| captures.get(1).map(|matched| matched.as_str().to_string()))
    } else {
        Some(value.to_string())
    }?;
    let normalized = normalized.trim_end_matches(&['.', ',', ';', ')', ']'][..]);
    DOI_RE
        .is_match(normalized)
        .then_some(normalized.to_string())
}

pub fn normalize_arxiv_id(raw: &str) -> Option<String> {
    let value = raw.trim().trim_start_matches("arXiv:");
    if let Some(captures) = ARXIV_URL_RE.captures(value) {
        return captures.get(1).map(|matched| matched.as_str().to_string());
    }
    ARXIV_RE.is_match(value).then_some(value.to_string())
}

fn parse_arxiv_atom(arxiv_id: &str, body: &str) -> ZotResult<ArxivWork> {
    let mut reader = Reader::from_str(body);
    reader.config_mut().trim_text(false);
    let mut stack = Vec::<Vec<u8>>::new();
    let mut entry_depth = None;
    let mut entry_complete = false;
    let mut author_depth = None;
    let mut capture: Option<(AtomField, usize, String)> = None;
    let mut title = None;
    let mut abstract_note = None;
    let mut published = None;
    let mut creators = Vec::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(start)) => {
                let local = start.local_name().as_ref().to_vec();
                let depth = stack.len() + 1;
                if entry_depth.is_none() && !entry_complete && local.as_slice() == b"entry" {
                    entry_depth = Some(depth);
                } else if entry_depth.is_some() {
                    if local.as_slice() == b"author" {
                        author_depth = Some(depth);
                    } else if capture.is_none() {
                        let field = match local.as_slice() {
                            b"title" => Some(AtomField::Title),
                            b"summary" => Some(AtomField::Summary),
                            b"published" => Some(AtomField::Published),
                            b"name" if author_depth.is_some() => Some(AtomField::Author),
                            _ => None,
                        };
                        if let Some(field) = field {
                            capture = Some((field, depth, String::new()));
                        }
                    }
                }
                stack.push(local);
            }
            Ok(Event::Text(text)) => {
                if let Some((_, _, value)) = capture.as_mut() {
                    let decoded = text.decode().map_err(arxiv_xml_error)?;
                    let unescaped =
                        quick_xml::escape::unescape(decoded.as_ref()).map_err(arxiv_xml_error)?;
                    value.push_str(unescaped.as_ref());
                }
            }
            Ok(Event::CData(text)) => {
                if let Some((_, _, value)) = capture.as_mut() {
                    value.push_str(text.decode().map_err(arxiv_xml_error)?.as_ref());
                }
            }
            Ok(Event::GeneralRef(reference)) => {
                if let Some((_, _, value)) = capture.as_mut() {
                    if let Some(character) =
                        reference.resolve_char_ref().map_err(arxiv_xml_error)?
                    {
                        value.push(character);
                    } else {
                        let name = reference.decode().map_err(arxiv_xml_error)?;
                        let resolved = quick_xml::escape::resolve_xml_entity(name.as_ref())
                            .ok_or_else(|| {
                                arxiv_xml_error(format!("unknown XML entity &{name};"))
                            })?;
                        value.push_str(resolved);
                    }
                }
            }
            Ok(Event::End(end)) => {
                let local = end.local_name();
                let depth = stack.len();
                let capture_complete = capture
                    .as_ref()
                    .is_some_and(|(_, capture_depth, _)| *capture_depth == depth);
                if capture_complete {
                    if let Some((field, _, value)) = capture.take() {
                        let value = normalize_xml_text(&value);
                        if !value.is_empty() {
                            match field {
                                AtomField::Title => title = Some(value),
                                AtomField::Summary => abstract_note = Some(value),
                                AtomField::Published => published = Some(value),
                                AtomField::Author => creators.push(split_creator_name(&value)),
                            }
                        }
                    }
                }
                if author_depth == Some(depth) && local.as_ref() == b"author" {
                    author_depth = None;
                }
                if entry_depth == Some(depth) && local.as_ref() == b"entry" {
                    entry_depth = None;
                    entry_complete = true;
                    author_depth = None;
                }
                let _ = stack.pop();
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(arxiv_xml_error(error)),
        }
    }

    let title = title.ok_or_else(|| ZotError::Remote {
        code: "arxiv-parse".to_string(),
        message: format!("No arXiv entry found for {arxiv_id}"),
        hint: None,
        status: None,
    })?;
    let date = published
        .as_deref()
        .and_then(|value| value.get(0..10))
        .map(str::to_string);
    Ok(ArxivWork {
        arxiv_id: arxiv_id.to_string(),
        title,
        abstract_note,
        date,
        creators,
        abs_url: format!("https://arxiv.org/abs/{arxiv_id}"),
        pdf_url: format!("https://arxiv.org/pdf/{arxiv_id}.pdf"),
    })
}

#[derive(Clone, Copy)]
enum AtomField {
    Title,
    Summary,
    Published,
    Author,
}

fn arxiv_xml_error(error: impl std::fmt::Display) -> ZotError {
    ZotError::Remote {
        code: "arxiv-parse".to_string(),
        message: format!("Unable to parse arXiv Atom response: {error}"),
        hint: None,
        status: None,
    }
}

fn normalize_xml_text(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn split_creator_name(name: &str) -> CreatorName {
    match name.rsplit_once(' ') {
        Some((first_name, last_name)) => CreatorName {
            first_name: first_name.to_string(),
            last_name: last_name.to_string(),
            creator_type: "author".to_string(),
        },
        None => CreatorName {
            first_name: String::new(),
            last_name: name.to_string(),
            creator_type: "author".to_string(),
        },
    }
}

fn html_unescape(value: &str) -> String {
    value
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn try_arxiv_from_crossref(crossref: &CrossRefWork) -> Option<String> {
    for relation in &crossref.relations {
        if let (Some(id_type), Some(id)) = (relation.id_type.as_deref(), relation.id.as_deref()) {
            if id_type.eq_ignore_ascii_case("arxiv") {
                return Some(format!("https://arxiv.org/pdf/{id}.pdf"));
            }
            if id_type.eq_ignore_ascii_case("doi") && id.contains("arXiv.") {
                if let Some(captures) = ARXIV_DOI_RE.captures(id) {
                    if let Some(matched) = captures.get(1) {
                        return Some(format!("https://arxiv.org/pdf/{}.pdf", matched.as_str()));
                    }
                }
            }
        }
    }
    for alternative_id in &crossref.alternative_ids {
        if ARXIV_RE.is_match(alternative_id) {
            return Some(format!("https://arxiv.org/pdf/{alternative_id}.pdf"));
        }
    }
    for link in &crossref.links {
        if let Some(captures) = ARXIV_URL_RE.captures(link) {
            if let Some(matched) = captures.get(1) {
                return Some(format!("https://arxiv.org/pdf/{}.pdf", matched.as_str()));
            }
        }
    }
    None
}

#[derive(Debug, Deserialize)]
struct CrossRefEnvelope {
    message: CrossRefMessage,
}

#[derive(Debug, Deserialize)]
struct CrossRefMessage {
    #[serde(rename = "type")]
    record_type: Option<String>,
    title: Option<Vec<String>>,
    author: Option<Vec<CrossRefPerson>>,
    editor: Option<Vec<CrossRefPerson>>,
    published: Option<CrossRefPublished>,
    created: Option<CrossRefPublished>,
    #[serde(rename = "DOI")]
    doi: Option<String>,
    #[serde(rename = "URL")]
    url: Option<String>,
    volume: Option<String>,
    issue: Option<String>,
    page: Option<String>,
    publisher: Option<String>,
    #[serde(rename = "ISSN")]
    issn: Option<Vec<String>>,
    #[serde(rename = "container-title")]
    container_title: Option<Vec<String>>,
    #[serde(rename = "abstract")]
    abstract_field: Option<String>,
    relation: Option<std::collections::BTreeMap<String, Vec<CrossRefRelationPayload>>>,
    #[serde(rename = "alternative-id")]
    alternative_ids: Option<Vec<String>>,
    link: Option<Vec<CrossRefLinkPayload>>,
}

impl CrossRefMessage {
    fn into_work(self, normalized_doi: &str) -> CrossRefWork {
        let date = self
            .published
            .or(self.created)
            .and_then(|published| published.date_parts.first().cloned())
            .map(|parts| {
                parts
                    .into_iter()
                    .map(|part| part.to_string())
                    .collect::<Vec<_>>()
                    .join("-")
            });
        let mut creators = Vec::new();
        for author in self.author.unwrap_or_default() {
            creators.push(author.into_creator("author"));
        }
        for editor in self.editor.unwrap_or_default() {
            creators.push(editor.into_creator("editor"));
        }
        CrossRefWork {
            record_type: self.record_type.unwrap_or_else(|| "document".to_string()),
            title: self.title.and_then(|mut values| values.drain(..).next()),
            creators,
            date,
            doi: self.doi.unwrap_or_else(|| normalized_doi.to_string()),
            url: self.url,
            volume: self.volume,
            issue: self.issue,
            pages: self.page,
            publisher: self.publisher,
            issn: self.issn.and_then(|mut values| values.drain(..).next()),
            publication_title: self
                .container_title
                .and_then(|mut values| values.drain(..).next()),
            abstract_note: self.abstract_field.map(|value| {
                html_unescape(&value.replace("<jats:p>", "").replace("</jats:p>", ""))
            }),
            relations: self
                .relation
                .unwrap_or_default()
                .into_iter()
                .flat_map(|(rel_type, relations)| {
                    relations.into_iter().map(move |relation| CrossRefRelation {
                        rel_type: rel_type.clone(),
                        id_type: relation.id_type,
                        id: relation.id,
                    })
                })
                .collect(),
            alternative_ids: self.alternative_ids.unwrap_or_default(),
            links: self
                .link
                .unwrap_or_default()
                .into_iter()
                .filter_map(|link| link.url)
                .collect(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct CrossRefPerson {
    given: Option<String>,
    family: Option<String>,
    name: Option<String>,
}

impl CrossRefPerson {
    fn into_creator(self, creator_type: &str) -> CreatorName {
        if let Some(family) = self.family {
            CreatorName {
                first_name: self.given.unwrap_or_default(),
                last_name: family,
                creator_type: creator_type.to_string(),
            }
        } else {
            CreatorName {
                first_name: String::new(),
                last_name: self.name.unwrap_or_default(),
                creator_type: creator_type.to_string(),
            }
        }
    }
}

#[derive(Debug, Deserialize)]
struct CrossRefPublished {
    #[serde(rename = "date-parts")]
    date_parts: Vec<Vec<i64>>,
}

#[derive(Debug, Deserialize)]
struct CrossRefRelationPayload {
    #[serde(rename = "id-type")]
    id_type: Option<String>,
    id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CrossRefLinkPayload {
    #[serde(rename = "URL")]
    url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UnpaywallPayload {
    #[serde(rename = "best_oa_location")]
    best_oa_location: Option<UnpaywallLocation>,
    #[serde(default)]
    oa_locations: Vec<UnpaywallLocation>,
}

#[derive(Debug, Deserialize)]
struct UnpaywallLocation {
    url: Option<String>,
    url_for_pdf: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SemanticScholarOaPayload {
    #[serde(rename = "openAccessPdf")]
    open_access_pdf: Option<OpenAccessPdf>,
}

#[derive(Debug, Deserialize)]
struct OpenAccessPdf {
    url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PmcPayload {
    #[serde(default)]
    records: Vec<PmcRecord>,
}

#[derive(Debug, Deserialize)]
struct PmcRecord {
    pmcid: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::{
        CrossRefEnvelope, CrossRefRelation, CrossRefWork, normalize_arxiv_id, normalize_doi,
        parse_arxiv_atom, try_arxiv_from_crossref,
    };

    #[test]
    fn normalizes_doi_variants() {
        assert_eq!(
            normalize_doi("doi:10.1038/nature12373"),
            Some("10.1038/nature12373".to_string())
        );
        assert_eq!(
            normalize_doi("https://doi.org/10.1038/nature12373."),
            Some("10.1038/nature12373".to_string())
        );
        assert_eq!(normalize_doi("not-a-doi"), None);
    }

    #[test]
    fn normalizes_arxiv_variants() {
        assert_eq!(
            normalize_arxiv_id("arXiv:2301.00774"),
            Some("2301.00774".to_string())
        );
        assert_eq!(
            normalize_arxiv_id("https://arxiv.org/abs/2301.00774v2"),
            Some("2301.00774v2".to_string())
        );
        assert_eq!(normalize_arxiv_id("definitely-not-arxiv"), None);
    }

    #[test]
    fn parses_arxiv_atom_and_discovers_crossref_relations() {
        let atom = r#"
            <feed xmlns="http://www.w3.org/2005/Atom">
              <title>ArXiv Query Results</title>
              <entry>
                <title> Attention Is All You Need </title>
                <summary> We study transformers &amp; attention. </summary>
                <published>2017-06-12T00:00:00Z</published>
                <author><name>Ashish Vaswani</name></author>
                <author><name>Noam Shazeer</name></author>
              </entry>
            </feed>
        "#;
        let work = match parse_arxiv_atom("1706.03762", atom) {
            Ok(work) => work,
            Err(err) => panic!("parse arxiv atom failed: {err}"),
        };
        assert_eq!(work.title, "Attention Is All You Need");
        assert_eq!(
            work.abstract_note.as_deref(),
            Some("We study transformers & attention.")
        );
        assert_eq!(work.date.as_deref(), Some("2017-06-12"));
        assert_eq!(work.creators.len(), 2);

        let crossref = CrossRefWork {
            record_type: "journal-article".to_string(),
            title: None,
            creators: Vec::new(),
            date: None,
            doi: "10.1103/PhysRevD.110.L081901".to_string(),
            url: None,
            volume: None,
            issue: None,
            pages: None,
            publisher: None,
            issn: None,
            publication_title: None,
            abstract_note: None,
            relations: vec![CrossRefRelation {
                rel_type: "has-preprint".to_string(),
                id_type: Some("arxiv".to_string()),
                id: Some("2301.00774".to_string()),
            }],
            alternative_ids: Vec::new(),
            links: Vec::new(),
        };
        assert_eq!(
            try_arxiv_from_crossref(&crossref),
            Some("https://arxiv.org/pdf/2301.00774.pdf".to_string())
        );
    }

    #[test]
    fn parses_namespaced_nested_and_cdata_arxiv_atom() {
        let atom = r#"
            <atom:feed xmlns:atom="http://www.w3.org/2005/Atom">
              <atom:title>Feed title</atom:title>
              <atom:entry>
                <atom:title>Structured <em>XML</em> &amp; safety</atom:title>
                <atom:summary><![CDATA[Bounded <downloads> stay intact.]]></atom:summary>
                <atom:published>2026-07-26T12:30:00Z</atom:published>
                <atom:author><atom:name>Ada <suffix>Lovelace</suffix></atom:name></atom:author>
                <atom:author><atom:name>Grace Hopper</atom:name></atom:author>
              </atom:entry>
            </atom:feed>
        "#;
        let work = parse_arxiv_atom("2607.12345", atom).expect("parse structured Atom");
        assert_eq!(work.title, "Structured XML & safety");
        assert_eq!(
            work.abstract_note.as_deref(),
            Some("Bounded <downloads> stay intact.")
        );
        assert_eq!(work.date.as_deref(), Some("2026-07-26"));
        assert_eq!(work.creators.len(), 2);
        assert_eq!(work.creators[0].first_name, "Ada");
        assert_eq!(work.creators[0].last_name, "Lovelace");
    }

    #[test]
    fn rejects_malformed_or_entryless_arxiv_atom() {
        for atom in [
            "<feed><entry><title>broken</entry></feed>",
            "<feed><title>feed only</title></feed>",
            "<feed><entry><title>valid first entry</title></entry><broken></feed>",
        ] {
            let error = parse_arxiv_atom("2607.12345", atom).expect_err("Atom must fail");
            assert_eq!(error.payload().code, "arxiv-parse");
        }
    }

    #[test]
    fn parses_crossref_abstract_with_jats_wrapper() {
        let payload = r#"{
            "message": {
                "type": "journal-article",
                "DOI": "10.1234/example",
                "abstract": "<jats:p>foo</jats:p>"
            }
        }"#;
        let envelope: CrossRefEnvelope = match serde_json::from_str(payload) {
            Ok(value) => value,
            Err(err) => panic!("deserialize crossref envelope failed: {err}"),
        };
        let work = envelope.message.into_work("10.1234/example");
        assert_eq!(work.abstract_note.as_deref(), Some("foo"));
    }
}
