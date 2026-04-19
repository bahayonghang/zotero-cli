use std::collections::{BTreeSet, HashSet};

use anyhow::Result;
use serde_json::{Map, Value, json};
use zot_core::{MergeApplyResult, MergeFieldFill, MergeOperation, MergePreview, ZotError};
use zot_remote::ZoteroRemote;

const NON_MERGEABLE_ITEM_TYPES: &[&str] = &["attachment", "note", "annotation"];
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
];

#[derive(Debug)]
pub(crate) struct MergeExecutionPlan {
    pub(crate) preview: MergePreview,
    pub(crate) merged_keeper: Value,
    pub(crate) children_to_reparent: Vec<Value>,
}

pub(crate) async fn merge_item_set(
    remote: &ZoteroRemote,
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

    let keeper = remote.get_item_flat(keeper_key).await?;
    let keeper_children = remote.list_children_flat(keeper_key).await?;
    let mut sources = Vec::new();
    let mut source_children = Vec::new();
    for key in source_keys {
        sources.push(remote.get_item_flat(key).await?);
        source_children.extend(remote.list_children_flat(key).await?);
    }

    let plan = build_merge_execution_plan(keeper, sources, keeper_children, source_children)?;
    if !confirm {
        return Ok(preview_operation(plan.preview));
    }

    remote.update_flat_item_value(&plan.merged_keeper).await?;
    for mut child in plan.children_to_reparent {
        child["parentItem"] = Value::String(plan.preview.keeper_key.clone());
        remote.update_flat_item_value(&child).await?;
    }
    for key in &plan.preview.source_keys {
        remote.set_deleted(key, true).await?;
    }

    Ok(applied_operation(&plan.preview))
}

pub(crate) fn build_merge_execution_plan(
    mut keeper: Value,
    sources: Vec<Value>,
    keeper_children: Vec<Value>,
    source_children: Vec<Value>,
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

    for source in &sources {
        let source_key = item_key(source)?.to_string();
        let source_fields = item_object(source)?;
        for (field, source_value) in source_fields {
            if STRUCTURAL_FIELDS.contains(&field.as_str()) || is_empty(source_value) {
                continue;
            }
            let target_value = keeper.get(field);
            if target_value.is_none_or(is_empty) {
                item_object_mut(&mut keeper)?.insert(field.clone(), source_value.clone());
                metadata_fields_to_fill.push(MergeFieldFill {
                    field: field.clone(),
                    source_key: source_key.clone(),
                    value: preview_value(source_value),
                });
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
            keeper_key,
            source_keys,
            metadata_fields_to_fill,
            tags_to_add,
            collections_to_add,
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

pub(crate) fn applied_operation(preview: &MergePreview) -> MergeOperation {
    MergeOperation::Applied(MergeApplyResult {
        keeper_key: preview.keeper_key.clone(),
        source_keys_trashed: preview.source_keys.clone(),
        metadata_fields_filled: preview
            .metadata_fields_to_fill
            .iter()
            .map(|entry| entry.field.clone())
            .collect(),
        tags_added: preview.tags_to_add.clone(),
        collections_added: preview.collections_to_add.clone(),
        children_reparented: preview.children_to_reparent,
        skipped_duplicate_attachments: preview.skipped_duplicate_attachments,
    })
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
    use serde_json::json;

    use super::{applied_operation, build_merge_execution_plan};
    use zot_core::{MergeOperation, ZotError};

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

        let plan = build_merge_execution_plan(keeper, vec![source], vec![], vec![])
            .expect("build merge plan");

        assert_eq!(plan.preview.metadata_fields_to_fill.len(), 2);
        assert_eq!(plan.preview.tags_to_add, vec!["priority"]);
        assert_eq!(plan.preview.collections_to_add, vec!["COLL002"]);
        assert_eq!(plan.merged_keeper["DOI"], "10.1000/test");
        assert_eq!(plan.merged_keeper["abstractNote"], "Merged abstract");

        let applied = applied_operation(&plan.preview);
        match applied {
            MergeOperation::Applied(result) => {
                assert_eq!(
                    result.metadata_fields_filled,
                    vec!["DOI".to_string(), "abstractNote".to_string()]
                );
                assert_eq!(result.tags_added, vec!["priority"]);
            }
            other => panic!("unexpected merge operation: {other:?}"),
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

        let plan =
            build_merge_execution_plan(keeper, vec![source], keeper_children, source_children)
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

        let err = build_merge_execution_plan(keeper, vec![source], vec![], vec![])
            .expect_err("attachment keeper should fail");
        let err = err.downcast_ref::<ZotError>().expect("zot error");
        match err {
            ZotError::InvalidInput { code, .. } => assert_eq!(code, "item-merge"),
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
