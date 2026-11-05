//! Field and sort predicates shared by the SQLite snapshot and desktop item JSON.
//!
//! Title, journal, and topic filters collapse case and whitespace before `contains`.
//! A generic metadata query does not collapse whitespace, matching SQL `LIKE` on each
//! stored field. Journal checks only `publicationTitle` and `journalAbbreviation`.

use crate::model::{Creator, Item};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchItemSort {
    DateAdded,
    DateModified,
    Title,
    Creator,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchItemDirection {
    Asc,
    Desc,
}

/// Trim, Unicode-lowercase, and collapse internal whitespace to a single space.
pub fn collapse_field_text(value: &str) -> String {
    let mut out = String::new();
    let mut pending_space = false;
    for ch in value.trim().chars() {
        if ch.is_whitespace() {
            pending_space = !out.is_empty();
            continue;
        }
        if pending_space {
            out.push(' ');
            pending_space = false;
        }
        for lower in ch.to_lowercase() {
            out.push(lower);
        }
    }
    out
}

pub fn collapsed_contains(haystack: &str, needle: &str) -> bool {
    let needle = collapse_field_text(needle);
    if needle.is_empty() {
        return false;
    }
    collapse_field_text(haystack).contains(&needle)
}

fn active_filter(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|text| !text.is_empty())
}

pub fn item_matches_field_filters(
    item: &Item,
    title: Option<&str>,
    journal: Option<&str>,
    topic: Option<&str>,
) -> bool {
    if let Some(title) = active_filter(title) {
        if !collapsed_contains(&item.title, title) {
            return false;
        }
    }
    if let Some(journal) = active_filter(journal) {
        let publication = item
            .extra
            .get("publicationTitle")
            .map(String::as_str)
            .unwrap_or("");
        let abbreviation = item
            .extra
            .get("journalAbbreviation")
            .map(String::as_str)
            .unwrap_or("");
        if !collapsed_contains(publication, journal) && !collapsed_contains(abbreviation, journal) {
            return false;
        }
    }
    if let Some(topic) = active_filter(topic) {
        let in_title = collapsed_contains(&item.title, topic);
        let in_abstract = item
            .abstract_note
            .as_deref()
            .is_some_and(|note| collapsed_contains(note, topic));
        let in_tag = item.tags.iter().any(|tag| collapsed_contains(tag, topic));
        if !in_title && !in_abstract && !in_tag {
            return false;
        }
    }
    true
}

/// Case-insensitive substring on metadata fields. Does not read attachment body text.
pub fn item_matches_metadata_query(item: &Item, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    let needle = query.to_lowercase();
    let hit = |value: &str| value.to_lowercase().contains(&needle);
    if hit(&item.title) {
        return true;
    }
    if item.abstract_note.as_deref().is_some_and(hit) {
        return true;
    }
    if item.date.as_deref().is_some_and(hit) {
        return true;
    }
    if item.url.as_deref().is_some_and(hit) {
        return true;
    }
    if item.doi.as_deref().is_some_and(hit) {
        return true;
    }
    if item.extra.values().any(|value| hit(value)) {
        return true;
    }
    if item.tags.iter().any(|tag| hit(tag)) {
        return true;
    }
    item.creators
        .iter()
        .any(|creator| hit(&creator.first_name) || hit(&creator.last_name))
}

/// Matches the snapshot creator filter: trimmed `first last`, case-insensitive substring.
pub fn item_matches_creator_filter(item: &Item, creator: &str) -> bool {
    let needle = creator.to_lowercase();
    if needle.is_empty() {
        return true;
    }
    item.creators.iter().any(|person| {
        format!("{} {}", person.first_name, person.last_name)
            .trim()
            .to_lowercase()
            .contains(&needle)
    })
}

pub fn item_matches_tag_filter(item: &Item, tag: &str) -> bool {
    let needle = tag.to_lowercase();
    item.tags
        .iter()
        .any(|existing| existing.to_lowercase() == needle)
}

pub fn item_matches_year_filter(item: &Item, year: &str) -> bool {
    item.date
        .as_deref()
        .is_some_and(|date| date.starts_with(year))
}

pub fn extra_line_has_citation_key(extra: &str, citekey: &str) -> bool {
    extra.lines().any(|line| {
        let normalized = line.trim().to_lowercase();
        (normalized.starts_with("citation key:") || normalized.starts_with("citationkey:"))
            && line
                .split_once(':')
                .is_some_and(|(_, value)| value.trim() == citekey)
    })
}

pub fn sort_search_items(
    items: &mut [Item],
    sort: Option<SearchItemSort>,
    direction: SearchItemDirection,
) {
    let descending = direction == SearchItemDirection::Desc;
    items.sort_by(|left, right| {
        let primary = match sort {
            Some(SearchItemSort::Title) => {
                left.title.to_lowercase().cmp(&right.title.to_lowercase())
            }
            Some(SearchItemSort::Creator) => creator_sort_key(left).cmp(&creator_sort_key(right)),
            Some(SearchItemSort::DateAdded) => {
                optional_text(&left.date_added).cmp(optional_text(&right.date_added))
            }
            Some(SearchItemSort::DateModified) => {
                optional_text(&left.date_modified).cmp(optional_text(&right.date_modified))
            }
            None => std::cmp::Ordering::Equal,
        };
        let primary = if descending {
            primary.reverse()
        } else {
            primary
        };
        let tie = left.key.cmp(&right.key);
        let tie = if descending { tie.reverse() } else { tie };
        primary.then(tie)
    });
}

fn creator_sort_key(item: &Item) -> String {
    item.creators
        .first()
        .map(|creator| format!("{} {}", creator.first_name, creator.last_name))
        .map(|name| name.trim().to_lowercase())
        .unwrap_or_default()
}

fn optional_text(value: &Option<String>) -> &str {
    value.as_deref().unwrap_or("")
}

pub fn first_creator_name(creators: &[Creator]) -> String {
    creators.first().map(Creator::full_name).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Creator;
    use std::collections::BTreeMap;

    fn item(title: &str) -> Item {
        Item {
            key: "ITEM0001".to_string(),
            item_type: "journalArticle".to_string(),
            title: title.to_string(),
            creators: vec![Creator {
                first_name: "Ashish".to_string(),
                last_name: "Vaswani".to_string(),
                creator_type: "author".to_string(),
            }],
            abstract_note: Some("This abstract mentions Nature".to_string()),
            date: Some("2017".to_string()),
            url: None,
            doi: None,
            tags: vec!["transformer".to_string()],
            collections: Vec::new(),
            date_added: Some("2024-01-01".to_string()),
            date_modified: None,
            extra: BTreeMap::from([
                ("publicationTitle".to_string(), "Nature".to_string()),
                ("journalAbbreviation".to_string(), "Nat.".to_string()),
                ("extra".to_string(), "Citation Key: Smith2024".to_string()),
            ]),
        }
    }

    #[test]
    fn field_filters_collapse_case_and_whitespace_without_using_abstract_as_journal() {
        let paper = item("Deep  Learning");
        assert!(item_matches_field_filters(
            &paper,
            Some("deep learning"),
            None,
            None
        ));
        assert!(!item_matches_field_filters(
            &paper,
            Some("deep x learning"),
            None,
            None
        ));
        assert!(item_matches_field_filters(
            &paper,
            None,
            Some(" nature "),
            None
        ));
        assert!(item_matches_field_filters(&paper, None, Some("nat."), None));
        let mut science = item("Published in Science");
        science
            .extra
            .insert("publicationTitle".to_string(), "Science".to_string());
        science.extra.remove("journalAbbreviation");
        assert!(!item_matches_field_filters(
            &science,
            None,
            Some("Nature"),
            None
        ));
        assert!(item_matches_field_filters(
            &paper,
            None,
            None,
            Some("Transformer")
        ));
        assert!(item_matches_field_filters(
            &paper,
            None,
            None,
            Some("mentions nature")
        ));
        assert!(!item_matches_field_filters(
            &paper,
            None,
            None,
            Some("zzbodyonly")
        ));
        assert!(extra_line_has_citation_key(
            paper.extra.get("extra").map(String::as_str).unwrap_or(""),
            "Smith2024"
        ));
        assert!(!extra_line_has_citation_key(
            "Citation Key: Smith2024",
            "smith2024"
        ));
        assert!(!extra_line_has_citation_key(
            "Citation Key: Smith2024",
            "Smith"
        ));
    }

    #[test]
    fn metadata_query_does_not_collapse_whitespace_or_join_creator_names() {
        let paper = item("Deep  Learning");
        assert!(!item_matches_metadata_query(&paper, "Deep Learning"));
        assert!(item_matches_metadata_query(&paper, "Deep  Learning"));
        assert!(item_matches_metadata_query(&paper, "vaswani"));
        assert!(!item_matches_metadata_query(&paper, "Ashish Vaswani"));
        assert!(item_matches_creator_filter(&paper, "Ashish Vaswani"));
        assert!(item_matches_creator_filter(&paper, "Vaswani"));
        assert!(item_matches_tag_filter(&paper, "Transformer"));
        assert!(!item_matches_tag_filter(&paper, "transform"));
        assert!(item_matches_year_filter(&paper, "2017"));
        assert!(!item_matches_year_filter(&paper, "2018"));
    }
}
