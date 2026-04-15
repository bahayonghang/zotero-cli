use std::collections::BTreeMap;

use zot_core::{Creator, Item, ZotError, ZotResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CitationStyle {
    Apa,
    Nature,
    Vancouver,
}

pub fn export_item(item: &Item, format: &str) -> ZotResult<String> {
    match format {
        "bibtex" => Ok(to_bibtex(item)),
        "csl-json" | "csl" | "json" => {
            serde_json::to_string_pretty(&to_csl_json(item)).map_err(|err| ZotError::InvalidInput {
                code: "serialize".to_string(),
                message: format!("Failed to serialize CSL-JSON: {err}"),
                hint: None,
            })
        }
        "ris" => Ok(to_ris(item)),
        other => Err(ZotError::InvalidInput {
            code: "invalid-export-format".to_string(),
            message: format!("Unsupported export format: {other}"),
            hint: Some("Use bibtex, csl-json, ris, or json".to_string()),
        }),
    }
}

pub fn format_citation(item: &Item, style: CitationStyle) -> String {
    match style {
        CitationStyle::Apa => format_apa(item),
        CitationStyle::Nature => format_nature(item),
        CitationStyle::Vancouver => format_vancouver(item),
    }
}

fn escape_bibtex(value: &str) -> String {
    value
        .replace('&', "\\&")
        .replace('%', "\\%")
        .replace('#', "\\#")
        .replace('_', "\\_")
}

fn to_bibtex(item: &Item) -> String {
    let bib_type = match item.item_type.as_str() {
        "journalArticle" => "article",
        "book" => "book",
        "thesis" => "phdthesis",
        "conferencePaper" => "inproceedings",
        _ => "misc",
    };

    let authors = item
        .creators
        .iter()
        .filter(|c| c.creator_type == "author")
        .map(|creator| {
            format!(
                "{}, {}",
                escape_bibtex(&creator.last_name),
                escape_bibtex(&creator.first_name)
            )
        })
        .collect::<Vec<_>>()
        .join(" and ");

    let mut lines = vec![format!("@{}{{{},", bib_type, item.key.to_lowercase())];
    if !item.title.is_empty() {
        lines.push(format!("  title = {{{}}},", escape_bibtex(&item.title)));
    }
    if !authors.is_empty() {
        lines.push(format!("  author = {{{}}},", authors));
    }
    if let Some(year) = item.date.as_deref() {
        lines.push(format!("  year = {{{year}}},"));
    }
    if let Some(doi) = item.doi.as_deref() {
        lines.push(format!("  doi = {{{doi}}},"));
    }
    if let Some(url) = item.url.as_deref() {
        lines.push(format!("  url = {{{url}}},"));
    }
    lines.push("}".to_string());
    lines.join("\n")
}

fn to_csl_json(item: &Item) -> BTreeMap<String, serde_json::Value> {
    let mut csl = BTreeMap::new();
    let item_type = match item.item_type.as_str() {
        "journalArticle" => "article-journal",
        "book" => "book",
        "bookSection" => "chapter",
        "conferencePaper" => "paper-conference",
        "thesis" => "thesis",
        "report" => "report",
        "webpage" => "webpage",
        "preprint" => "article",
        _ => "article",
    };
    csl.insert(
        "id".to_string(),
        serde_json::Value::String(item.key.clone()),
    );
    csl.insert(
        "type".to_string(),
        serde_json::Value::String(item_type.to_string()),
    );
    csl.insert(
        "title".to_string(),
        serde_json::Value::String(item.title.clone()),
    );
    if !item.creators.is_empty() {
        let authors = item
            .creators
            .iter()
            .filter(|creator| creator.creator_type == "author")
            .map(|creator| {
                serde_json::json!({
                    "family": creator.last_name,
                    "given": creator.first_name,
                })
            })
            .collect::<Vec<_>>();
        csl.insert("author".to_string(), serde_json::Value::Array(authors));
    }
    if let Some(date) = item.date.as_deref() {
        csl.insert("issued".to_string(), serde_json::json!({ "raw": date }));
    }
    if let Some(abstract_note) = item.abstract_note.as_deref() {
        csl.insert(
            "abstract".to_string(),
            serde_json::Value::String(abstract_note.to_string()),
        );
    }
    if let Some(doi) = item.doi.as_deref() {
        csl.insert(
            "DOI".to_string(),
            serde_json::Value::String(doi.to_string()),
        );
    }
    if let Some(url) = item.url.as_deref() {
        csl.insert(
            "URL".to_string(),
            serde_json::Value::String(url.to_string()),
        );
    }
    csl
}

fn to_ris(item: &Item) -> String {
    let ris_type = match item.item_type.as_str() {
        "journalArticle" => "JOUR",
        "book" => "BOOK",
        "bookSection" => "CHAP",
        "conferencePaper" => "CONF",
        "thesis" => "THES",
        "report" => "RPRT",
        "webpage" => "ELEC",
        "preprint" => "JOUR",
        _ => "GEN",
    };

    let mut lines = vec![format!("TY  - {ris_type}")];
    if !item.title.is_empty() {
        lines.push(format!("TI  - {}", item.title));
    }
    for creator in item
        .creators
        .iter()
        .filter(|creator| creator.creator_type == "author")
    {
        lines.push(format!(
            "AU  - {}, {}",
            creator.last_name, creator.first_name
        ));
    }
    if let Some(date) = item.date.as_deref() {
        lines.push(format!("PY  - {date}"));
    }
    if let Some(abstract_note) = item.abstract_note.as_deref() {
        lines.push(format!("AB  - {abstract_note}"));
    }
    if let Some(doi) = item.doi.as_deref() {
        lines.push(format!("DO  - {doi}"));
    }
    if let Some(url) = item.url.as_deref() {
        lines.push(format!("UR  - {url}"));
    }
    for tag in &item.tags {
        lines.push(format!("KW  - {tag}"));
    }
    lines.push("ER  - ".to_string());
    lines.join("\n")
}

fn format_apa(item: &Item) -> String {
    let authors = format_apa_authors(&item.creators);
    let year = extract_year(item.date.as_deref());
    let mut parts = Vec::new();
    if !authors.is_empty() {
        parts.push(format!("{authors} ({year})."));
    } else {
        parts.push(format!("({year})."));
    }
    parts.push(format!("{}.", item.title));

    let journal = item
        .extra
        .get("publicationTitle")
        .cloned()
        .unwrap_or_default();
    let volume = item.extra.get("volume").cloned().unwrap_or_default();
    let issue = item.extra.get("issue").cloned().unwrap_or_default();
    let pages = item.extra.get("pages").cloned().unwrap_or_default();
    if !journal.is_empty() {
        let volume_part = if volume.is_empty() {
            String::new()
        } else {
            format!(", {volume}")
        };
        let issue_part = if issue.is_empty() {
            String::new()
        } else {
            format!("({issue})")
        };
        let page_part = if pages.is_empty() {
            String::new()
        } else {
            format!(", {pages}")
        };
        parts.push(format!("{journal}{volume_part}{issue_part}{page_part}."));
    }
    if let Some(doi) = item.doi.as_deref() {
        parts.push(format!("https://doi.org/{doi}"));
    }

    parts.join(" ")
}

fn format_nature(item: &Item) -> String {
    let authors = format_nature_authors(&item.creators);
    let year = extract_year(item.date.as_deref());
    let journal = item
        .extra
        .get("publicationTitle")
        .or_else(|| item.extra.get("journalAbbreviation"))
        .cloned()
        .unwrap_or_default();
    let volume = item.extra.get("volume").cloned().unwrap_or_default();
    let pages = item.extra.get("pages").cloned().unwrap_or_default();

    let mut text = if authors.is_empty() {
        format!("{}.", item.title)
    } else {
        format!("{authors} {}.", item.title)
    };
    if !journal.is_empty() {
        text.push_str(&format!(" {journal}"));
        if !volume.is_empty() {
            text.push_str(&format!(" **{volume}**"));
        }
        if !pages.is_empty() {
            text.push_str(&format!(", {pages}"));
        }
        text.push_str(&format!(" ({year})."));
    } else {
        text.push_str(&format!(" ({year})."));
    }
    if let Some(doi) = item.doi.as_deref() {
        text.push_str(&format!(" https://doi.org/{doi}"));
    }
    text
}

fn format_vancouver(item: &Item) -> String {
    let authors = format_vancouver_authors(&item.creators);
    let year = extract_year(item.date.as_deref());
    let journal = item
        .extra
        .get("journalAbbreviation")
        .or_else(|| item.extra.get("publicationTitle"))
        .cloned()
        .unwrap_or_default();
    let volume = item.extra.get("volume").cloned().unwrap_or_default();
    let issue = item.extra.get("issue").cloned().unwrap_or_default();
    let pages = item.extra.get("pages").cloned().unwrap_or_default();

    let mut result = if authors.is_empty() {
        format!("{}.", item.title)
    } else {
        format!("{authors}. {}.", item.title)
    };
    if !journal.is_empty() {
        result.push_str(&format!(" {journal}. {year}"));
        if !volume.is_empty() {
            result.push_str(&format!(";{volume}"));
        }
        if !issue.is_empty() {
            result.push_str(&format!("({issue})"));
        }
        if !pages.is_empty() {
            result.push_str(&format!(":{pages}"));
        }
        result.push('.');
    } else {
        result.push_str(&format!(" {year}."));
    }
    if let Some(doi) = item.doi.as_deref() {
        result.push_str(&format!(" doi:{doi}"));
    }
    result
}

fn format_apa_authors(creators: &[Creator]) -> String {
    let authors = creators
        .iter()
        .filter(|creator| creator.creator_type == "author")
        .collect::<Vec<_>>();
    match authors.len() {
        0 => String::new(),
        1 => format_apa_author(authors[0]),
        2 => format!(
            "{} & {}",
            format_apa_author(authors[0]),
            format_apa_author(authors[1])
        ),
        _ => {
            let mut parts = authors
                .iter()
                .take(19)
                .map(|author| format_apa_author(author))
                .collect::<Vec<_>>();
            if authors.len() > 20 {
                parts.push("...".to_string());
                parts.push(format_apa_author(authors.last().expect("non-empty")));
            }
            let last = parts.pop().unwrap_or_default();
            format!("{}, & {}", parts.join(", "), last)
        }
    }
}

fn format_apa_author(author: &Creator) -> String {
    let initials = author
        .first_name
        .split_whitespace()
        .filter_map(|part| part.chars().next())
        .map(|c| format!("{c}."))
        .collect::<Vec<_>>()
        .join(" ");
    if initials.is_empty() {
        author.last_name.clone()
    } else {
        format!("{}, {}", author.last_name, initials)
    }
}

fn format_nature_authors(creators: &[Creator]) -> String {
    let authors = creators
        .iter()
        .filter(|creator| creator.creator_type == "author")
        .collect::<Vec<_>>();
    if authors.is_empty() {
        return String::new();
    }
    let mut parts = authors
        .iter()
        .map(|author| {
            let initials = author
                .first_name
                .split_whitespace()
                .filter_map(|part| part.chars().next())
                .map(|c| format!("{c}."))
                .collect::<Vec<_>>()
                .join(" ");
            if initials.is_empty() {
                author.last_name.clone()
            } else {
                format!("{}, {}", author.last_name, initials)
            }
        })
        .collect::<Vec<_>>();
    if parts.len() > 5 {
        parts.truncate(5);
        return format!("{} et al.", parts.join(", "));
    }
    if parts.len() == 1 {
        parts.remove(0)
    } else {
        let last = parts.pop().unwrap_or_default();
        format!("{} & {}", parts.join(", "), last)
    }
}

fn format_vancouver_authors(creators: &[Creator]) -> String {
    let authors = creators
        .iter()
        .filter(|creator| creator.creator_type == "author")
        .collect::<Vec<_>>();
    let mut parts = authors
        .iter()
        .take(6)
        .map(|author| {
            let initials = author
                .first_name
                .split_whitespace()
                .filter_map(|part| part.chars().next())
                .collect::<String>();
            if initials.is_empty() {
                author.last_name.clone()
            } else {
                format!("{} {}", author.last_name, initials)
            }
        })
        .collect::<Vec<_>>();
    if authors.len() > 6 {
        parts.push("et al".to_string());
    }
    parts.join(", ")
}

fn extract_year(date: Option<&str>) -> String {
    let Some(date) = date else {
        return "n.d.".to_string();
    };
    for part in date.replace('/', "-").split('-') {
        if part.len() == 4 && part.chars().all(|ch| ch.is_ascii_digit()) {
            return part.to_string();
        }
    }
    date.to_string()
}
