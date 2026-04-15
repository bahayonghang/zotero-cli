use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Creator {
    pub first_name: String,
    pub last_name: String,
    pub creator_type: String,
}

impl Creator {
    pub fn full_name(&self) -> String {
        match (self.first_name.is_empty(), self.last_name.is_empty()) {
            (true, true) => String::new(),
            (true, false) => self.last_name.clone(),
            (false, true) => self.first_name.clone(),
            (false, false) => format!("{} {}", self.first_name, self.last_name),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Item {
    pub key: String,
    pub item_type: String,
    pub title: String,
    pub creators: Vec<Creator>,
    pub abstract_note: Option<String>,
    pub date: Option<String>,
    pub url: Option<String>,
    pub doi: Option<String>,
    pub tags: Vec<String>,
    pub collections: Vec<String>,
    pub date_added: Option<String>,
    pub date_modified: Option<String>,
    pub extra: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Note {
    pub key: String,
    pub parent_key: String,
    pub content: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Attachment {
    pub key: String,
    pub parent_key: String,
    pub filename: String,
    pub content_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Collection {
    pub key: String,
    pub name: String,
    pub parent_key: Option<String>,
    pub children: Vec<Collection>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SearchResult {
    pub items: Vec<Item>,
    pub total: usize,
    pub query: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DuplicateGroup {
    pub match_type: String,
    pub score: f32,
    pub items: Vec<Item>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LibraryStats {
    pub total_items: usize,
    pub by_type: std::collections::BTreeMap<String, usize>,
    pub top_tags: std::collections::BTreeMap<String, usize>,
    pub collections: std::collections::BTreeMap<String, usize>,
    pub pdf_attachments: usize,
    pub notes: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkspaceItem {
    pub key: String,
    pub title: String,
    pub added: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Workspace {
    pub name: String,
    pub created: String,
    pub description: String,
    pub items: Vec<WorkspaceItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QueryChunk {
    pub item_key: String,
    pub source: String,
    pub score: f32,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AnnotationSnippet {
    pub annotation_type: String,
    pub page: usize,
    pub content: String,
    pub quote: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PublicationMatch {
    pub key: String,
    pub preprint_id: String,
    pub source: String,
    pub title: String,
    pub published: bool,
    pub venue: Option<String>,
    pub journal: Option<String>,
    pub doi: Option<String>,
    pub date: Option<String>,
}
