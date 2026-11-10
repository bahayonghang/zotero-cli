use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::future::Future;
use std::pin::Pin;

use anyhow::Result;
use serde_json::{Map, Value, json};
use zot_core::{
    MergeApplyResult, MergeFieldFill, MergeOperation, MergePreview, MergeSkippedField, ZotError,
};
use zot_remote::ZoteroRemote;

use crate::context::AppContext;

const NON_MERGEABLE_ITEM_TYPES: &[&str] = &["attachment", "note", "annotation"];
// `relations` is structural too: it is merged by the dedicated per-predicate
// union in `merge_relations`, never by the plain field fill.
const STRUCTURAL_FIELDS: &[&str] = &[
    "key",
    "version",
    "itemType",
    "dateAdded",
    "dateModified",
    "tags",
    "collections",
    "creators",
    "parentItem",
    "relations",
];
const DC_REPLACES: &str = "dc:replaces";

#[derive(Debug)]
pub(crate) struct MergeExecutionPlan {
    pub(crate) preview: MergePreview,
    pub(crate) merged_keeper: Value,
    pub(crate) children_to_reparent: Vec<Value>,
}

type MergeFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T>> + 'a>>;

#[derive(Debug)]
pub(crate) struct PreparedMerge {
    pub(crate) preview: MergePreview,
    plan: Box<MergeExecutionPlan>,
}

pub(crate) trait MergeWriter {
    fn preview<'a>(
        &'a self,
        keeper_key: &'a str,
        source_keys: &'a [String],
    ) -> MergeFuture<'a, PreparedMerge>;

    fn apply<'a>(
        &'a self,
        prepared: PreparedMerge,
        operation_id: &'a str,
    ) -> MergeFuture<'a, MergeApplyResult>;
}

pub(crate) struct WebMergeWriter {
    remote: ZoteroRemote,
}

impl WebMergeWriter {
    fn new(remote: ZoteroRemote) -> Self {
        Self { remote }
    }
}

impl MergeWriter for WebMergeWriter {
    fn preview<'a>(
        &'a self,
        keeper_key: &'a str,
        source_keys: &'a [String],
    ) -> MergeFuture<'a, PreparedMerge> {
        Box::pin(async move {
            let keeper = self.remote.get_item_flat(keeper_key).await?;
            let keeper_children = self.remote.list_children_flat(keeper_key).await?;
            let mut sources = Vec::new();
            let mut source_children = Vec::new();
            for key in source_keys {
                sources.push(self.remote.get_item_flat(key).await?);
                source_children.extend(self.remote.list_children_flat(key).await?);
            }
            let source_uris = source_keys
                .iter()
                .map(|key| (key.clone(), self.remote.item_uri(key)))
                .collect::<BTreeMap<_, _>>();
            let plan = build_merge_execution_plan(
                keeper,
                sources,
                keeper_children,
                source_children,
                &source_uris,
            )?;
            Ok(PreparedMerge {
                preview: plan.preview.clone(),
                plan: Box::new(plan),
            })
        })
    }

    fn apply<'a>(
        &'a self,
        prepared: PreparedMerge,
        _operation_id: &'a str,
    ) -> MergeFuture<'a, MergeApplyResult> {
        Box::pin(async move {
            let plan = prepared.plan;
            self.remote
                .update_flat_item_value(&plan.merged_keeper)
                .await?;
            for mut child in plan.children_to_reparent {
                child["parentItem"] = Value::String(plan.preview.keeper_key.clone());
                self.remote.update_flat_item_value(&child).await?;
            }
            for key in &plan.preview.source_keys {
                self.remote.set_deleted(key, true).await?;
            }
            Ok(apply_result(&plan.preview))
        })
    }
}

pub(crate) fn selected_merge_writer(ctx: &AppContext) -> Result<Box<dyn MergeWriter>> {
    Ok(Box::new(WebMergeWriter::new(ctx.remote()?)))
}

pub(crate) async fn merge_item_set(
    writer: &dyn MergeWriter,
    keeper_key: &str,
    source_keys: &[String],
    confirm: bool,
) -> Result<MergeOperation> {
    if source_keys.is_empty() {
        return Err(ZotError::InvalidInput {
            code: "item-merge".to_string(),
            message: "At least one source item is required".to_string(),
            hint: None,
        }
        .into());
    }

    let prepared = writer.preview(keeper_key, source_keys).await?;
    if !confirm {
        return Ok(preview_operation(prepared.preview));
    }
    let operation_id = uuid::Uuid::new_v4().to_string();
    Ok(MergeOperation::Applied(
        writer.apply(prepared, &operation_id).await?,
    ))
}

pub(crate) fn build_merge_execution_plan(
    mut keeper: Value,
    sources: Vec<Value>,
    keeper_children: Vec<Value>,
    source_children: Vec<Value>,
    source_uris: &BTreeMap<String, String>,
) -> Result<MergeExecutionPlan> {
    if sources.is_empty() {
        return Err(ZotError::InvalidInput {
            code: "merge-source".to_string(),
            message: "At least one source item is required".to_string(),
            hint: None,
        }
        .into());
    }

    validate_merge_candidate(&keeper, "keeper")?;
    for source in &sources {
        validate_merge_candidate(source, "source")?;
    }

    let keeper_key = item_key(&keeper)?.to_string();
    let source_keys = sources
        .iter()
        .map(item_key)
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();

    let original_tags = tag_set(&keeper);
    let original_collections = collection_set(&keeper);
    let mut metadata_fields_to_fill = Vec::new();
    let mut skipped_incompatible_fields = Vec::new();

    for source in &sources {
        let source_key = item_key(source)?.to_string();
        let source_fields = item_object(source)?;
        for (field, source_value) in source_fields {
            if STRUCTURAL_FIELDS.contains(&field.as_str()) || is_empty(source_value) {
                continue;
            }
            match keeper.get(field).map(is_empty) {
                // The keeper's flat JSON carries every field legal for its
                // item type (unset ones come back as ""), so a missing key
                // means the keeper's type has no such field: filling it
                // would fail the write with 400 Invalid field. Record the
                // dropped value instead of discarding it silently.
                None => skipped_incompatible_fields.push(MergeSkippedField {
                    field: field.clone(),
                    source_key: source_key.clone(),
                }),
                Some(true) => {
                    item_object_mut(&mut keeper)?.insert(field.clone(), source_value.clone());
                    metadata_fields_to_fill.push(MergeFieldFill {
                        field: field.clone(),
                        source_key: source_key.clone(),
                        value: preview_value(source_value),
                    });
                }
                Some(false) => {}
            }
        }
    }

    let mut merged_tags = original_tags.clone();
    for source in &sources {
        merged_tags.extend(tag_set(source));
    }
    let tags_to_add = sorted_difference(&merged_tags, &original_tags);
    item_object_mut(&mut keeper)?.insert(
        "tags".to_string(),
        Value::Array(
            merged_tags
                .iter()
                .map(|tag| json!({ "tag": tag }))
                .collect(),
        ),
    );

    let mut merged_collections = original_collections.clone();
    for source in &sources {
        merged_collections.extend(collection_set(source));
    }
    let collections_to_add = sorted_difference(&merged_collections, &original_collections);
    item_object_mut(&mut keeper)?.insert(
        "collections".to_string(),
        Value::Array(
            merged_collections
                .iter()
                .cloned()
                .map(Value::String)
                .collect(),
        ),
    );

    let (merged_relations, relations_to_add) =
        merge_relations(&keeper, &sources, &source_keys, source_uris)?;
    item_object_mut(&mut keeper)?.insert("relations".to_string(), merged_relations);

    let mut seen_attachment_signatures = keeper_children
        .iter()
        .filter_map(attachment_signature)
        .collect::<HashSet<_>>();
    let mut children_to_reparent = Vec::new();
    let mut skipped_duplicate_attachments = 0usize;
    for child in source_children {
        if let Some(signature) = attachment_signature(&child)
            && !seen_attachment_signatures.insert(signature)
        {
            skipped_duplicate_attachments += 1;
            continue;
        }
        children_to_reparent.push(child);
    }

    Ok(MergeExecutionPlan {
        preview: MergePreview {
            plan_id: None,
            keeper_key,
            source_keys,
            metadata_fields_to_fill,
            skipped_incompatible_fields,
            tags_to_add,
            collections_to_add,
            relations_to_add,
            children_to_reparent: children_to_reparent.len(),
            skipped_duplicate_attachments,
            confirm_required: true,
        },
        merged_keeper: keeper,
        children_to_reparent,
    })
}

pub(crate) fn preview_operation(preview: MergePreview) -> MergeOperation {
    MergeOperation::Preview(preview)
}

#[cfg(test)]
pub(crate) fn applied_operation(preview: &MergePreview) -> MergeOperation {
    MergeOperation::Applied(apply_result(preview))
}

fn apply_result(preview: &MergePreview) -> MergeApplyResult {
    MergeApplyResult {
        already_applied: false,
        keeper_key: preview.keeper_key.clone(),
        source_keys_trashed: preview.source_keys.clone(),
        metadata_fields_filled: preview
            .metadata_fields_to_fill
            .iter()
            .map(|entry| entry.field.clone())
            .collect(),
        skipped_incompatible_fields: preview.skipped_incompatible_fields.clone(),
        tags_added: preview.tags_to_add.clone(),
        collections_added: preview.collections_to_add.clone(),
        relations_to_add: preview.relations_to_add.clone(),
        children_reparented: preview.children_to_reparent,
        skipped_duplicate_attachments: preview.skipped_duplicate_attachments,
    }
}

/// Union the `relations` maps of keeper and sources per predicate (values
/// normalized to sorted, deduplicated arrays), then point `dc:replaces` at
/// every merged source's canonical URI. Zotero's own merge writes the same
/// relation and the word-processor plugins rely on it to redirect citations
/// of trashed sources to the keeper. Returns the merged relations value plus
/// the `dc:replaces` URIs that are new relative to the keeper's existing
/// relations (so re-running a merge stays idempotent).
fn merge_relations(
    keeper: &Value,
    sources: &[Value],
    source_keys: &[String],
    source_uris: &BTreeMap<String, String>,
) -> Result<(Value, Vec<String>)> {
    let mut merged: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    collect_relations(keeper, &mut merged);
    for source in sources {
        collect_relations(source, &mut merged);
    }

    let existing_replaces = merged.get(DC_REPLACES).cloned().unwrap_or_default();
    let mut replaced_uris = BTreeSet::new();
    for key in source_keys {
        let uri = source_uris.get(key).ok_or_else(|| ZotError::InvalidInput {
            code: "item-merge".to_string(),
            message: format!("Missing canonical URI for source item '{key}'"),
            hint: None,
        })?;
        replaced_uris.insert(uri.clone());
    }
    let relations_to_add = sorted_difference(&replaced_uris, &existing_replaces);
    merged
        .entry(DC_REPLACES.to_string())
        .or_default()
        .extend(replaced_uris);

    let relations = merged
        .into_iter()
        .filter(|(_, values)| !values.is_empty())
        .map(|(predicate, values)| {
            (
                predicate,
                Value::Array(values.into_iter().map(Value::String).collect()),
            )
        })
        .collect::<Map<String, Value>>();
    Ok((Value::Object(relations), relations_to_add))
}

/// Accumulate one item's `relations` into `merged`, accepting both the
/// single-string and the array form Zotero uses for relation values.
fn collect_relations(item: &Value, merged: &mut BTreeMap<String, BTreeSet<String>>) {
    let Some(relations) = item.get("relations").and_then(Value::as_object) else {
        return;
    };
    for (predicate, value) in relations {
        let values = merged.entry(predicate.clone()).or_default();
        match value {
            Value::String(uri) if !uri.is_empty() => {
                values.insert(uri.clone());
            }
            Value::Array(entries) => {
                for uri in entries.iter().filter_map(Value::as_str) {
                    if !uri.is_empty() {
                        values.insert(uri.to_string());
                    }
                }
            }
            _ => {}
        }
    }
}

fn validate_merge_candidate(item: &Value, role: &str) -> Result<()> {
    let item_type = item
        .get("itemType")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if NON_MERGEABLE_ITEM_TYPES.contains(&item_type) {
        return Err(ZotError::InvalidInput {
            code: "item-merge".to_string(),
            message: format!(
                "Cannot use {role} item '{}' of type '{}' in merge",
                item_key(item).unwrap_or(""),
                item_type
            ),
            hint: Some("Only top-level bibliographic items can be merged".to_string()),
        }
        .into());
    }
    Ok(())
}

fn item_key(item: &Value) -> Result<&str> {
    item.get("key").and_then(Value::as_str).ok_or_else(|| {
        ZotError::InvalidInput {
            code: "item-merge".to_string(),
            message: "Merge payload is missing item key".to_string(),
            hint: None,
        }
        .into()
    })
}

fn item_object(item: &Value) -> Result<&Map<String, Value>> {
    item.as_object().ok_or_else(|| {
        ZotError::InvalidInput {
            code: "item-merge".to_string(),
            message: "Merge payload is not a JSON object".to_string(),
            hint: None,
        }
        .into()
    })
}

fn item_object_mut(item: &mut Value) -> Result<&mut Map<String, Value>> {
    item.as_object_mut().ok_or_else(|| {
        ZotError::InvalidInput {
            code: "item-merge".to_string(),
            message: "Merge payload is not a JSON object".to_string(),
            hint: None,
        }
        .into()
    })
}

fn is_empty(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::String(text) => text.trim().is_empty(),
        Value::Array(values) => values.is_empty(),
        Value::Object(values) => values.is_empty(),
        _ => false,
    }
}

fn tag_set(item: &Value) -> BTreeSet<String> {
    item.get("tags")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.get("tag").and_then(Value::as_str))
        .map(ToOwned::to_owned)
        .collect()
}

fn collection_set(item: &Value) -> BTreeSet<String> {
    item.get("collections")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(ToOwned::to_owned)
        .collect()
}

fn sorted_difference(all: &BTreeSet<String>, baseline: &BTreeSet<String>) -> Vec<String> {
    all.iter()
        .filter(|entry| !baseline.contains(*entry))
        .cloned()
        .collect()
}

fn preview_value(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        _ => value.to_string(),
    }
}

fn attachment_signature(item: &Value) -> Option<(String, String, String, String)> {
    (item.get("itemType").and_then(Value::as_str) == Some("attachment")).then(|| {
        (
            item.get("contentType")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            item.get("filename")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            item.get("md5")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            item.get("url")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
        )
    })
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::BTreeMap;

    use serde_json::json;
    use tiny_http::{Response, Server};

    use super::{
        MergeFuture, MergeWriter, PreparedMerge, WebMergeWriter, applied_operation,
        build_merge_execution_plan, merge_item_set,
    };
    use zot_core::{LibraryScope, MergeApplyResult, MergeOperation, ZotError};
    use zot_remote::{HttpRuntime, ZoteroRemote};

    fn uri_map(entries: &[(&str, &str)]) -> BTreeMap<String, String> {
        entries
            .iter()
            .map(|(key, uri)| ((*key).to_string(), (*uri).to_string()))
            .collect()
    }

    struct WebRequest {
        method: String,
        path: String,
        body: String,
        precondition: Option<String>,
    }

    fn spawn_web_merge_server(
        responses: Vec<(u16, String)>,
    ) -> (String, std::thread::JoinHandle<Vec<WebRequest>>) {
        let server = Server::http(("127.0.0.1", 0)).expect("bind web merge server");
        let port = server.server_addr().to_ip().expect("server IP").port();
        let base = format!("http://127.0.0.1:{port}");
        let handle = std::thread::spawn(move || {
            responses
                .into_iter()
                .map(|(status, body)| {
                    let mut request = server.recv().expect("receive web merge request");
                    let mut request_body = String::new();
                    request
                        .as_reader()
                        .read_to_string(&mut request_body)
                        .expect("read request body");
                    let captured = WebRequest {
                        method: request.method().as_str().to_string(),
                        path: request.url().to_string(),
                        body: request_body,
                        precondition: request
                            .headers()
                            .iter()
                            .find(|header| header.field.equiv("if-unmodified-since-version"))
                            .map(|header| header.value.to_string()),
                    };
                    request
                        .respond(Response::from_string(body).with_status_code(status))
                        .expect("respond to web merge request");
                    captured
                })
                .collect()
        });
        (base, handle)
    }

    #[tokio::test]
    async fn web_writer_preserves_existing_request_sequence_and_payloads() {
        let keeper = json!({
            "key": "KEEP0001",
            "version": 7,
            "data": {
                "itemType": "journalArticle",
                "title": "Keeper",
                "DOI": "",
                "tags": [],
                "collections": [],
                "relations": {}
            }
        })
        .to_string();
        let source = json!({
            "key": "DUPE0001",
            "version": 8,
            "data": {
                "itemType": "preprint",
                "title": "Source",
                "DOI": "10.1000/test",
                "tags": [],
                "collections": [],
                "relations": {}
            }
        })
        .to_string();
        let (base, handle) = spawn_web_merge_server(vec![
            (200, keeper),
            (200, "[]".to_string()),
            (200, source.clone()),
            (200, "[]".to_string()),
            (204, String::new()),
            (200, source),
            (204, String::new()),
        ]);
        let remote = ZoteroRemote::with_base_url_for_tests(
            &HttpRuntime::default(),
            "123",
            "api-key",
            LibraryScope::User,
            base,
        )
        .expect("web client");
        let writer = WebMergeWriter::new(remote);

        let operation = merge_item_set(&writer, "KEEP0001", &["DUPE0001".to_string()], true)
            .await
            .expect("web merge");
        let MergeOperation::Applied(result) = operation else {
            panic!("expected applied merge");
        };
        assert_eq!(result.metadata_fields_filled, vec!["DOI"]);

        let requests = handle.join().expect("join web merge server");
        assert_eq!(
            requests
                .iter()
                .map(|request| (request.method.as_str(), request.path.as_str()))
                .collect::<Vec<_>>(),
            vec![
                ("GET", "/users/123/items/KEEP0001"),
                ("GET", "/users/123/items/KEEP0001/children"),
                ("GET", "/users/123/items/DUPE0001"),
                ("GET", "/users/123/items/DUPE0001/children"),
                ("PUT", "/users/123/items/KEEP0001"),
                ("GET", "/users/123/items/DUPE0001"),
                ("PATCH", "/users/123/items/DUPE0001"),
            ]
        );
        assert_eq!(requests[4].precondition.as_deref(), Some("7"));
        let keeper_payload: serde_json::Value =
            serde_json::from_str(&requests[4].body).expect("keeper payload");
        assert_eq!(keeper_payload["DOI"], "10.1000/test");
        assert_eq!(
            keeper_payload["relations"]["dc:replaces"],
            json!(["http://zotero.org/users/123/items/DUPE0001"])
        );
        assert_eq!(requests[6].precondition.as_deref(), Some("8"));
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&requests[6].body).expect("trash payload"),
            json!({ "deleted": 1 })
        );
    }

    struct FakeWriter {
        preview_calls: RefCell<Vec<(String, Vec<String>)>>,
        apply_calls: RefCell<Vec<String>>,
    }

    impl FakeWriter {
        fn new() -> Self {
            Self {
                preview_calls: RefCell::new(Vec::new()),
                apply_calls: RefCell::new(Vec::new()),
            }
        }
    }

    impl MergeWriter for FakeWriter {
        fn preview<'a>(
            &'a self,
            keeper_key: &'a str,
            source_keys: &'a [String],
        ) -> MergeFuture<'a, PreparedMerge> {
            Box::pin(async move {
                self.preview_calls
                    .borrow_mut()
                    .push((keeper_key.to_string(), source_keys.to_vec()));
                let keeper = json!({
                    "key": keeper_key,
                    "itemType": "journalArticle",
                    "title": "Keeper",
                    "tags": [],
                    "collections": [],
                    "relations": {},
                });
                let sources = source_keys
                    .iter()
                    .map(|key| {
                        json!({
                            "key": key,
                            "itemType": "journalArticle",
                            "title": "Source",
                            "tags": [],
                            "collections": [],
                            "relations": {},
                        })
                    })
                    .collect::<Vec<_>>();
                let uris = source_keys
                    .iter()
                    .map(|key| {
                        (
                            key.clone(),
                            format!("http://zotero.org/users/1/items/{key}"),
                        )
                    })
                    .collect();
                let plan = build_merge_execution_plan(keeper, sources, vec![], vec![], &uris)?;
                Ok(PreparedMerge {
                    preview: plan.preview.clone(),
                    plan: Box::new(plan),
                })
            })
        }

        fn apply<'a>(
            &'a self,
            prepared: PreparedMerge,
            operation_id: &'a str,
        ) -> MergeFuture<'a, MergeApplyResult> {
            Box::pin(async move {
                self.apply_calls.borrow_mut().push(operation_id.to_string());
                Ok(super::apply_result(&prepared.preview))
            })
        }
    }

    #[tokio::test]
    async fn writer_contract_validates_sources_and_separates_preview_from_apply() {
        let writer = FakeWriter::new();
        let empty = merge_item_set(&writer, "KEEP001", &[], false)
            .await
            .expect_err("empty source list");
        assert_eq!(
            empty
                .downcast_ref::<ZotError>()
                .expect("zot error")
                .payload()
                .code,
            "item-merge"
        );
        assert!(writer.preview_calls.borrow().is_empty());

        let sources = vec!["DUPE001".to_string(), "DUPE002".to_string()];
        let preview = merge_item_set(&writer, "KEEP001", &sources, false)
            .await
            .expect("preview");
        let MergeOperation::Preview(preview) = preview else {
            panic!("expected preview");
        };
        assert_eq!(preview.source_keys, sources);
        assert!(writer.apply_calls.borrow().is_empty());

        let applied = merge_item_set(&writer, "KEEP001", &["DUPE003".to_string()], true)
            .await
            .expect("apply");
        let MergeOperation::Applied(applied) = applied else {
            panic!("expected apply");
        };
        assert_eq!(applied.source_keys_trashed, vec!["DUPE003"]);
        assert_eq!(writer.apply_calls.borrow().len(), 1);
    }

    #[test]
    fn merge_plan_fills_empty_metadata_and_sets_apply_result() {
        let keeper = json!({
            "key": "KEEP001",
            "version": 7,
            "itemType": "journalArticle",
            "title": "Existing Title",
            "DOI": "",
            "abstractNote": null,
            "tags": [{"tag": "reading"}],
            "collections": ["COLL001"],
        });
        let source = json!({
            "key": "DUPE001",
            "version": 8,
            "itemType": "journalArticle",
            "title": "Source Title",
            "DOI": "10.1000/test",
            "abstractNote": "Merged abstract",
            "tags": [{"tag": "priority"}],
            "collections": ["COLL002"],
        });
        let uris = uri_map(&[("DUPE001", "http://zotero.org/users/123/items/DUPE001")]);

        let plan = build_merge_execution_plan(keeper, vec![source], vec![], vec![], &uris)
            .expect("build merge plan");

        assert_eq!(plan.preview.metadata_fields_to_fill.len(), 2);
        assert!(plan.preview.skipped_incompatible_fields.is_empty());
        assert_eq!(plan.preview.tags_to_add, vec!["priority"]);
        assert_eq!(plan.preview.collections_to_add, vec!["COLL002"]);
        assert_eq!(plan.merged_keeper["DOI"], "10.1000/test");
        assert_eq!(plan.merged_keeper["abstractNote"], "Merged abstract");
        assert_eq!(
            plan.merged_keeper["relations"]["dc:replaces"],
            json!(["http://zotero.org/users/123/items/DUPE001"])
        );

        let applied = applied_operation(&plan.preview);
        match applied {
            MergeOperation::Applied(result) => {
                assert_eq!(
                    result.metadata_fields_filled,
                    vec!["DOI".to_string(), "abstractNote".to_string()]
                );
                assert_eq!(result.tags_added, vec!["priority"]);
                assert!(result.skipped_incompatible_fields.is_empty());
                assert_eq!(
                    result.relations_to_add,
                    vec!["http://zotero.org/users/123/items/DUPE001".to_string()]
                );
            }
            other => panic!("unexpected merge operation: {other:?}"),
        }
    }

    #[test]
    fn merge_plan_skips_source_fields_missing_on_keeper_type() {
        // Cross-type merge: the conferencePaper keeper's flat JSON has no
        // `repository`/`archiveID` keys (illegal for its type) while its own
        // legal-but-unset fields are present as "" — the Web API response
        // shape. Type-specific source fields must never land in the payload
        // (the write would 400) and the dropped values must stay visible.
        let keeper = json!({
            "key": "CONF0001",
            "version": 12,
            "itemType": "conferencePaper",
            "title": "Attention Is All You Need",
            "DOI": "",
            "url": "",
            "proceedingsTitle": "",
            "tags": [],
            "collections": [],
            "relations": {},
        });
        let source = json!({
            "key": "PREP0001",
            "version": 4,
            "itemType": "preprint",
            "title": "Attention Is All You Need",
            "DOI": "10.48550/arXiv.1706.03762",
            "url": "https://arxiv.org/abs/1706.03762",
            "repository": "arXiv",
            "archiveID": "arXiv:1706.03762",
            "tags": [],
            "collections": [],
            "relations": {},
        });
        let uris = uri_map(&[("PREP0001", "http://zotero.org/users/123/items/PREP0001")]);

        let plan = build_merge_execution_plan(keeper, vec![source], vec![], vec![], &uris)
            .expect("build merge plan");

        assert_eq!(plan.merged_keeper["DOI"], "10.48550/arXiv.1706.03762");
        assert_eq!(
            plan.merged_keeper["url"],
            "https://arxiv.org/abs/1706.03762"
        );
        assert!(plan.merged_keeper.get("repository").is_none());
        assert!(plan.merged_keeper.get("archiveID").is_none());

        let mut skipped = plan
            .preview
            .skipped_incompatible_fields
            .iter()
            .map(|entry| (entry.field.as_str(), entry.source_key.as_str()))
            .collect::<Vec<_>>();
        skipped.sort_unstable();
        assert_eq!(
            skipped,
            vec![("archiveID", "PREP0001"), ("repository", "PREP0001")]
        );

        let mut filled = plan
            .preview
            .metadata_fields_to_fill
            .iter()
            .map(|entry| entry.field.as_str())
            .collect::<Vec<_>>();
        filled.sort_unstable();
        assert_eq!(filled, vec!["DOI", "url"]);
    }

    #[test]
    fn merge_plan_unions_relations_and_adds_dc_replaces() {
        let keeper = json!({
            "key": "KEEP001",
            "version": 7,
            "itemType": "journalArticle",
            "title": "Existing Title",
            "tags": [],
            "collections": [],
            "relations": { "dc:relation": "http://zotero.org/users/123/items/AAAA1111" },
        });
        let source_one = json!({
            "key": "DUPE001",
            "version": 8,
            "itemType": "journalArticle",
            "title": "Source Title",
            "tags": [],
            "collections": [],
            "relations": {
                "dc:relation": [
                    "http://zotero.org/users/123/items/AAAA1111",
                    "http://zotero.org/users/123/items/BBBB2222",
                ],
                "owl:sameAs": "http://zotero.org/groups/456/items/CCCC3333",
            },
        });
        let source_two = json!({
            "key": "DUPE002",
            "version": 9,
            "itemType": "journalArticle",
            "title": "Source Title Two",
            "tags": [],
            "collections": [],
        });
        let uris = uri_map(&[
            ("DUPE001", "http://zotero.org/users/123/items/DUPE001"),
            ("DUPE002", "http://zotero.org/users/123/items/DUPE002"),
        ]);

        let plan =
            build_merge_execution_plan(keeper, vec![source_one, source_two], vec![], vec![], &uris)
                .expect("build merge plan");

        let relations = &plan.merged_keeper["relations"];
        // Source relations are unioned per predicate; single-string values
        // are normalized to arrays and duplicates collapse.
        assert_eq!(
            relations["dc:relation"],
            json!([
                "http://zotero.org/users/123/items/AAAA1111",
                "http://zotero.org/users/123/items/BBBB2222",
            ])
        );
        assert_eq!(
            relations["owl:sameAs"],
            json!(["http://zotero.org/groups/456/items/CCCC3333"])
        );
        // Every merged source is recorded as dc:replaces so word-processor
        // citations redirect to the keeper.
        assert_eq!(
            relations["dc:replaces"],
            json!([
                "http://zotero.org/users/123/items/DUPE001",
                "http://zotero.org/users/123/items/DUPE002",
            ])
        );
        assert_eq!(
            plan.preview.relations_to_add,
            vec![
                "http://zotero.org/users/123/items/DUPE001".to_string(),
                "http://zotero.org/users/123/items/DUPE002".to_string(),
            ]
        );
    }

    #[test]
    fn merge_plan_dc_replaces_dedupes_and_reports_only_new_uris() {
        // Re-running a merge (e.g. after a partial failure) must not duplicate
        // dc:replaces entries; relations_to_add only lists genuinely new URIs.
        // Group-scope URIs double as the group-prefix example.
        let keeper = json!({
            "key": "KEEP001",
            "version": 7,
            "itemType": "journalArticle",
            "title": "Existing Title",
            "tags": [],
            "collections": [],
            "relations": {
                "dc:replaces": ["http://zotero.org/groups/9001/items/DUPE001"],
            },
        });
        let source_one = json!({
            "key": "DUPE001",
            "version": 8,
            "itemType": "journalArticle",
            "title": "Source Title",
            "tags": [],
            "collections": [],
        });
        let source_two = json!({
            "key": "DUPE002",
            "version": 9,
            "itemType": "journalArticle",
            "title": "Source Title Two",
            "tags": [],
            "collections": [],
        });
        let uris = uri_map(&[
            ("DUPE001", "http://zotero.org/groups/9001/items/DUPE001"),
            ("DUPE002", "http://zotero.org/groups/9001/items/DUPE002"),
        ]);

        let plan =
            build_merge_execution_plan(keeper, vec![source_one, source_two], vec![], vec![], &uris)
                .expect("build merge plan");

        assert_eq!(
            plan.merged_keeper["relations"]["dc:replaces"],
            json!([
                "http://zotero.org/groups/9001/items/DUPE001",
                "http://zotero.org/groups/9001/items/DUPE002",
            ])
        );
        assert_eq!(
            plan.preview.relations_to_add,
            vec!["http://zotero.org/groups/9001/items/DUPE002".to_string()]
        );
    }

    #[test]
    fn merge_plan_requires_uri_for_every_source() {
        // A source without a canonical URI would silently lose its citation
        // redirect, so the plan builder refuses to continue.
        let keeper = json!({
            "key": "KEEP001",
            "version": 7,
            "itemType": "journalArticle",
            "title": "Existing Title",
            "tags": [],
            "collections": [],
        });
        let source = json!({
            "key": "DUPE001",
            "version": 8,
            "itemType": "journalArticle",
            "title": "Source Title",
            "tags": [],
            "collections": [],
        });

        let err =
            build_merge_execution_plan(keeper, vec![source], vec![], vec![], &BTreeMap::new())
                .expect_err("missing source URI should fail");
        let err = err.downcast_ref::<ZotError>().expect("zot error");
        match err {
            ZotError::InvalidInput { code, .. } => assert_eq!(code, "item-merge"),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn merge_plan_skips_duplicate_attachments() {
        let keeper = json!({
            "key": "KEEP001",
            "version": 7,
            "itemType": "journalArticle",
            "title": "Existing Title",
            "tags": [],
            "collections": [],
        });
        let source = json!({
            "key": "DUPE001",
            "version": 8,
            "itemType": "journalArticle",
            "title": "Source Title",
            "tags": [],
            "collections": [],
        });
        let keeper_children = vec![json!({
            "key": "ATCH001",
            "version": 3,
            "itemType": "attachment",
            "contentType": "application/pdf",
            "filename": "paper.pdf",
            "md5": "abc",
            "url": "https://example.com/paper.pdf",
        })];
        let source_children = vec![
            json!({
                "key": "ATCH002",
                "version": 4,
                "itemType": "attachment",
                "contentType": "application/pdf",
                "filename": "paper.pdf",
                "md5": "abc",
                "url": "https://example.com/paper.pdf",
            }),
            json!({
                "key": "NOTE001",
                "version": 5,
                "itemType": "note",
                "note": "keep me",
            }),
        ];
        let uris = uri_map(&[("DUPE001", "http://zotero.org/users/123/items/DUPE001")]);

        let plan = build_merge_execution_plan(
            keeper,
            vec![source],
            keeper_children,
            source_children,
            &uris,
        )
        .expect("build merge plan");

        assert_eq!(plan.preview.skipped_duplicate_attachments, 1);
        assert_eq!(plan.preview.children_to_reparent, 1);
        assert_eq!(plan.children_to_reparent.len(), 1);
        assert_eq!(plan.children_to_reparent[0]["key"], "NOTE001");
    }

    #[test]
    fn merge_plan_rejects_attachment_as_merge_candidate() {
        let keeper = json!({
            "key": "KEEP001",
            "version": 7,
            "itemType": "attachment",
            "title": "Attachment",
            "tags": [],
            "collections": [],
        });
        let source = json!({
            "key": "DUPE001",
            "version": 8,
            "itemType": "journalArticle",
            "title": "Source Title",
            "tags": [],
            "collections": [],
        });
        let uris = uri_map(&[("DUPE001", "http://zotero.org/users/123/items/DUPE001")]);

        let err = build_merge_execution_plan(keeper, vec![source], vec![], vec![], &uris)
            .expect_err("attachment keeper should fail");
        let err = err.downcast_ref::<ZotError>().expect("zot error");
        match err {
            ZotError::InvalidInput { code, .. } => assert_eq!(code, "item-merge"),
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
