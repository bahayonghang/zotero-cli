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
pub struct SavedSearchCondition {
    pub condition: String,
    pub operator: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SavedSearch {
    pub key: String,
    pub version: i64,
    pub name: String,
    pub conditions: Vec<SavedSearchCondition>,
    pub library_type: Option<String>,
    pub library_id: Option<i64>,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TagSummary {
    pub name: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChildNote {
    pub key: String,
    pub parent_key: Option<String>,
    pub content: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChildAttachment {
    pub key: String,
    pub parent_key: Option<String>,
    pub filename: String,
    pub content_type: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChildAnnotation {
    pub key: String,
    pub parent_key: Option<String>,
    pub annotation_type: String,
    pub text: String,
    pub comment: String,
    pub color: Option<String>,
    pub page_label: Option<String>,
    pub tags: Vec<String>,
}

/// One child of an item: note, attachment, or annotation.
///
/// Serialised with a `kind` discriminator + flattened payload, so consumers
/// can match on `kind` instead of hunting through a dozen nullable fields:
/// ```json
/// { "kind": "note", "key": "...", "parent_key": "...", "content": "...", "tags": [] }
/// { "kind": "attachment", "key": "...", "filename": "...", "content_type": "...", "tags": [] }
/// { "kind": "annotation", "key": "...", "annotation_type": "highlight", "text": "...", ... }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ChildItem {
    Note(ChildNote),
    Attachment(ChildAttachment),
    Annotation(ChildAnnotation),
}

impl ChildItem {
    pub fn key(&self) -> &str {
        match self {
            Self::Note(inner) => &inner.key,
            Self::Attachment(inner) => &inner.key,
            Self::Annotation(inner) => &inner.key,
        }
    }

    pub fn parent_key(&self) -> Option<&str> {
        match self {
            Self::Note(inner) => inner.parent_key.as_deref(),
            Self::Attachment(inner) => inner.parent_key.as_deref(),
            Self::Annotation(inner) => inner.parent_key.as_deref(),
        }
    }

    pub fn kind_label(&self) -> &'static str {
        match self {
            Self::Note(_) => "note",
            Self::Attachment(_) => "attachment",
            Self::Annotation(_) => "annotation",
        }
    }

    pub fn tags(&self) -> &[String] {
        match self {
            Self::Note(inner) => &inner.tags,
            Self::Attachment(inner) => &inner.tags,
            Self::Annotation(inner) => &inner.tags,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NoteSearchResult {
    pub key: String,
    pub parent_key: Option<String>,
    pub parent_title: Option<String>,
    pub title: Option<String>,
    pub content: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AnnotationRecord {
    pub key: String,
    pub parent_key: Option<String>,
    pub parent_title: Option<String>,
    pub attachment_key: Option<String>,
    pub attachment_title: Option<String>,
    pub annotation_type: String,
    pub text: String,
    pub comment: String,
    pub color: Option<String>,
    pub page_label: Option<String>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CitationKeyMatch {
    pub citekey: String,
    pub source: String,
    pub item: Item,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LibraryInfo {
    pub library_id: i64,
    pub library_type: String,
    pub editable: bool,
    pub files_editable: bool,
    pub group_id: Option<i64>,
    pub group_name: Option<String>,
    pub group_description: Option<String>,
    pub feed_name: Option<String>,
    pub feed_url: Option<String>,
    pub item_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FeedInfo {
    pub library_id: i64,
    pub name: String,
    pub url: String,
    pub last_check: Option<String>,
    pub last_update: Option<String>,
    pub last_check_error: Option<String>,
    pub refresh_interval: Option<i64>,
    pub item_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PdfOutlineEntry {
    pub level: usize,
    pub title: String,
    pub page: Option<usize>,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SemanticHit {
    pub item: Item,
    pub score: f32,
    pub source: String,
    pub matched_chunk: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SemanticIndexStatus {
    pub exists: bool,
    pub path: String,
    pub indexed_items: usize,
    pub indexed_chunks: usize,
    pub chunks_with_embeddings: usize,
    pub last_indexed_at: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SciteTally {
    pub supporting: u32,
    pub contrasting: u32,
    pub mentioning: u32,
    pub total: u32,
    pub citing_publications: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EditorialNotice {
    pub notice_type: String,
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SciteItemReport {
    pub doi: String,
    pub title: String,
    pub tally: Option<SciteTally>,
    pub notices: Vec<EditorialNotice>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RetractionCheckResult {
    pub item: Item,
    pub notices: Vec<EditorialNotice>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MergeFieldFill {
    pub field: String,
    pub source_key: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MergePreview {
    pub keeper_key: String,
    pub source_keys: Vec<String>,
    pub metadata_fields_to_fill: Vec<MergeFieldFill>,
    pub tags_to_add: Vec<String>,
    pub collections_to_add: Vec<String>,
    pub children_to_reparent: usize,
    pub skipped_duplicate_attachments: usize,
    pub confirm_required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MergeApplyResult {
    pub keeper_key: String,
    pub source_keys_trashed: Vec<String>,
    pub metadata_fields_filled: Vec<String>,
    pub tags_added: Vec<String>,
    pub collections_added: Vec<String>,
    pub children_reparented: usize,
    pub skipped_duplicate_attachments: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "kebab-case")]
pub enum MergeOperation {
    Preview(MergePreview),
    Applied(MergeApplyResult),
}

/// The kind of local relationship an edge in the knowledge graph represents.
///
/// A single [`GraphEdge`] may carry several of these when two papers are
/// connected in more than one way (e.g. shared collection *and* an explicit
/// Zotero relation).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "kebab-case")]
pub enum EdgeRelation {
    Coauthor,
    Tag,
    Collection,
    Related,
}

/// One paper node in the knowledge graph.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GraphNode {
    pub key: String,
    pub title: String,
    pub item_type: String,
    pub year: Option<String>,
    pub first_author: Option<String>,
    pub tags: Vec<String>,
    pub doi: Option<String>,
    pub url: Option<String>,
    pub degree: usize,
    pub weighted_degree: i64,
    pub community: usize,
}

/// A weighted, possibly multi-relation edge between two paper nodes.
///
/// `source` < `target` (lexicographic on key) so each unordered pair appears
/// exactly once.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GraphEdge {
    pub source: String,
    pub target: String,
    pub weight: i64,
    pub relations: Vec<EdgeRelation>,
}

/// A node ranked by a scalar metric (degree, weighted degree, ...).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RankedNode {
    pub key: String,
    pub title: String,
    pub score: i64,
}

/// One detected community plus its most representative tags.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommunitySummary {
    pub id: usize,
    pub size: usize,
    pub top_tags: Vec<String>,
}

/// Structural analysis of a [`KnowledgeGraph`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GraphMetrics {
    pub node_count: usize,
    pub edge_count: usize,
    pub connected_components: usize,
    pub top_by_degree: Vec<RankedNode>,
    pub top_by_weighted_degree: Vec<RankedNode>,
    pub communities: Vec<CommunitySummary>,
    pub top_authors: Vec<TagSummary>,
    pub top_tags: Vec<TagSummary>,
}

/// A local relationship knowledge graph for a library or collection scope.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KnowledgeGraph {
    pub scope: String,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub metrics: GraphMetrics,
}

/// Which relation types contribute edges when building the graph.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GraphRelationToggles {
    pub coauthor: bool,
    pub tag: bool,
    pub collection: bool,
    pub related: bool,
}

impl Default for GraphRelationToggles {
    fn default() -> Self {
        Self {
            coauthor: true,
            tag: true,
            collection: true,
            related: true,
        }
    }
}

/// Tunable knobs for knowledge-graph construction.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GraphOptions {
    /// Restrict the graph to a single collection key; `None` = whole library.
    pub collection: Option<String>,
    /// Minimum shared tags before a `tag` edge is emitted (default 2).
    pub min_shared_tags: usize,
    /// Skip author/tag/collection groups larger than this to avoid hairballs
    /// (default 50).
    pub max_group_size: usize,
    /// Length of each Top-N ranking in the metrics (default 20).
    pub top_n: usize,
    pub relations: GraphRelationToggles,
}

impl Default for GraphOptions {
    fn default() -> Self {
        Self {
            collection: None,
            min_shared_tags: 2,
            max_group_size: 50,
            top_n: 20,
            relations: GraphRelationToggles::default(),
        }
    }
}
