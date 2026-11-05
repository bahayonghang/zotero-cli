//! Explicit local organize plans. Requests are untrusted data, not instructions.
use std::collections::BTreeSet;
use std::io::Write;
use std::path::Path;

use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use tempfile::NamedTempFile;
use zot_core::{ErrorPayload, LibraryScope, SecretString, ZotError, ZotResult};
use zot_desktop::{
    LocalApiClient, LocalCollection, LocalCollectionSnapshot, LocalObject, PatchReceipt,
};

use crate::cli::{
    LibraryOrganizeApplyArgs, LibraryOrganizeCommand, LibraryOrganizePlanArgs, WriteBackendArg,
};
use crate::context::AppContext;
use crate::output::CommandOutput;

use super::local_credentials;
use super::local_write::{
    local_patch_matches, next_memberships, plan_tags, require_bibliographic_item, tags_match,
};

const SCHEMA_VERSION: u64 = 1;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct OrganizeRequest {
    schema_version: u64,
    items: Vec<ItemRequest>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct ItemRequest {
    item_key: String,
    #[serde(default)]
    add_tags: Vec<String>,
    #[serde(default)]
    remove_tags: Vec<String>,
    #[serde(default)]
    add_collections: Vec<String>,
    #[serde(default)]
    remove_collections: Vec<String>,
    #[serde(default)]
    move_collection: Option<MoveRequest>,
    #[serde(default)]
    reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct MoveRequest {
    from: String,
    to: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct CollectionIdentity {
    key: String,
    name: String,
    version: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PlanItem {
    item_key: String,
    title: String,
    expected_version: u64,
    original_request: ItemRequest,
    changed: bool,
    before: Map<String, Value>,
    after: Map<String, Value>,
    collections: Vec<CollectionIdentity>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct OrganizePlan {
    schema_version: u64,
    backend: String,
    server_id: String,
    library: String,
    profile: Option<String>,
    max_affected: usize,
    item_count: usize,
    action_count: usize,
    changed_count: usize,
    unchanged_count: usize,
    items: Vec<PlanItem>,
}

#[derive(Debug, Clone, Serialize)]
struct PlanSummary {
    plan_path: String,
    plan_sha256: String,
    server_id: String,
    library: String,
    max_affected: usize,
    item_count: usize,
    action_count: usize,
    changed_count: usize,
    unchanged_count: usize,
}

#[derive(Debug, Clone, Serialize)]
struct ReportItem {
    item_key: String,
    status: String,
    before: Map<String, Value>,
    after: Map<String, Value>,
    expected_version: u64,
    observed_version: Option<u64>,
    write_accepted: bool,
    verified: bool,
    observed_expected_state: Option<bool>,
    error: Option<ErrorPayload>,
}

#[derive(Debug, Clone, Serialize)]
struct OrganizeReport {
    schema_version: u64,
    plan_sha256: String,
    server_id: String,
    library: String,
    state: String,
    started_at: String,
    updated_at: String,
    item_count: usize,
    applied_count: usize,
    unchanged_count: usize,
    failed_count: usize,
    unknown_count: usize,
    not_attempted_count: usize,
    items: Vec<ReportItem>,
    error: Option<ErrorPayload>,
}

pub(crate) async fn handle(
    ctx: &AppContext,
    command: LibraryOrganizeCommand,
) -> Result<CommandOutput> {
    match command {
        LibraryOrganizeCommand::Plan(args) => plan_command(ctx, args).await,
        LibraryOrganizeCommand::Apply(args) => apply_command(ctx, args).await,
    }
}

async fn plan_command(ctx: &AppContext, args: LibraryOrganizePlanArgs) -> Result<CommandOutput> {
    require_local(args.write_backend)?;
    if args.max_affected == 0 {
        return Err(invalid(
            "local-organize-max-affected",
            "--max-affected must be greater than zero",
        )
        .into());
    }
    if args.output.exists() {
        return Err(invalid(
            "local-organize-file",
            "Plan output already exists and will not be overwritten",
        )
        .into());
    }
    let request = read_request(&args.file)?;
    let client = ctx.local_api()?;
    let plan = build_plan(&client, ctx, &request, args.max_affected).await?;
    write_json(&args.output, &plan, false)?;
    let bytes = std::fs::read(&args.output).map_err(|source| ZotError::Io {
        path: args.output.clone(),
        source,
    })?;
    let summary = PlanSummary {
        plan_path: args.output.display().to_string(),
        plan_sha256: sha256_hex(&bytes),
        server_id: plan.server_id,
        library: plan.library,
        max_affected: plan.max_affected,
        item_count: plan.item_count,
        action_count: plan.action_count,
        changed_count: plan.changed_count,
        unchanged_count: plan.unchanged_count,
    };
    CommandOutput::new(ctx, summary, None, |summary| {
        println!(
            "plan: {} changed, sha256 {}",
            summary.changed_count, summary.plan_sha256
        );
    })
}

async fn apply_command(ctx: &AppContext, args: LibraryOrganizeApplyArgs) -> Result<CommandOutput> {
    require_local(args.write_backend)?;
    if !args.confirm {
        return Err(invalid(
            "local-organize-confirm",
            "Review the plan, then pass --confirm and its SHA-256",
        )
        .into());
    }
    let Some(expected_hash) = args.expected_plan_sha256.as_deref() else {
        return Err(invalid(
            "local-organize-hash",
            "Pass --expected-plan-sha256 from the plan command",
        )
        .into());
    };
    if args.max_affected == 0 {
        return Err(invalid(
            "local-organize-max-affected",
            "--max-affected must be greater than zero",
        )
        .into());
    }
    if args.report.exists() {
        return Err(invalid(
            "local-organize-file",
            "Report already exists and will not be overwritten",
        )
        .into());
    }
    let bytes = std::fs::read(&args.plan).map_err(|source| ZotError::Io {
        path: args.plan.clone(),
        source,
    })?;
    let actual_hash = sha256_hex(&bytes);
    if !actual_hash.eq_ignore_ascii_case(expected_hash.trim()) {
        return Err(invalid(
            "local-organize-hash",
            "Plan SHA-256 does not match the reviewed file bytes",
        )
        .into());
    }
    let plan: OrganizePlan = serde_json::from_slice(&bytes).map_err(|error| {
        invalid(
            "local-organize-schema",
            &format!("Plan schema is invalid: {error}"),
        )
    })?;
    validate_plan_shape(&plan, ctx, args.max_affected)?;
    let client = ctx.local_api()?;
    preflight(&client, ctx, &plan).await?;
    let mut report = initial_report(&plan, &actual_hash);
    write_json(&args.report, &report, false)?;
    let report = execute_plan(
        &client,
        ctx,
        &plan,
        &mut report,
        &args.report,
        &ctx.local_api_credentials_path()?,
    )
    .await?;
    let code = exit_code(&report.state);
    CommandOutput::new(ctx, report, None, |report| {
        println!("organize: {}", report.state);
    })
    .map(|output| output.with_exit_code(code))
}

fn require_local(backend: WriteBackendArg) -> ZotResult<()> {
    if backend != WriteBackendArg::Local {
        return Err(invalid(
            "local-organize-backend",
            "library organize only accepts --write-backend local",
        ));
    }
    Ok(())
}

fn read_request(path: &Path) -> ZotResult<OrganizeRequest> {
    let bytes = std::fs::read(path).map_err(|source| ZotError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let request: OrganizeRequest = serde_json::from_slice(&bytes).map_err(|error| {
        invalid(
            "local-organize-schema",
            &format!("Request schema is invalid: {error}"),
        )
    })?;
    validate_request(&request)?;
    Ok(request)
}

fn validate_request(request: &OrganizeRequest) -> ZotResult<()> {
    if request.schema_version != SCHEMA_VERSION {
        return Err(invalid(
            "local-organize-schema",
            "Only organize request schema_version 1 is supported",
        ));
    }
    if request.items.is_empty() {
        return Err(invalid(
            "local-organize-schema",
            "Organize request must contain at least one item",
        ));
    }
    let mut seen = BTreeSet::new();
    for item in &request.items {
        validate_key(&item.item_key)?;
        if !seen.insert(item.item_key.clone()) {
            return Err(invalid(
                "local-organize-conflict",
                &format!("Duplicate item_key {}", item.item_key),
            ));
        }
        validate_item_request(item)?;
    }
    Ok(())
}

fn validate_item_request(item: &ItemRequest) -> ZotResult<()> {
    let membership = !item.add_collections.is_empty()
        || !item.remove_collections.is_empty()
        || item.move_collection.is_some();
    if item.move_collection.is_some()
        && (!item.add_collections.is_empty() || !item.remove_collections.is_empty())
    {
        return Err(invalid(
            "local-organize-conflict",
            "move_collection cannot be combined with other membership actions",
        ));
    }
    if item.add_tags.is_empty() && item.remove_tags.is_empty() && !membership {
        return Err(invalid(
            "local-organize-schema",
            "Each item needs at least one tag or collection action",
        ));
    }
    for name in item.add_tags.iter().chain(&item.remove_tags) {
        if name.trim().is_empty() {
            return Err(invalid("local-write-tag", "Tag names must not be blank"));
        }
    }
    if item
        .add_tags
        .iter()
        .any(|name| item.remove_tags.iter().any(|other| other == name))
    {
        return Err(invalid(
            "local-organize-conflict",
            "The same tag cannot be added and removed",
        ));
    }
    for key in item.add_collections.iter().chain(&item.remove_collections) {
        validate_key(key)?;
    }
    if item
        .add_collections
        .iter()
        .any(|key| item.remove_collections.iter().any(|other| other == key))
    {
        return Err(invalid(
            "local-organize-conflict",
            "The same collection cannot be added and removed",
        ));
    }
    if let Some(move_collection) = &item.move_collection {
        validate_key(&move_collection.from)?;
        validate_key(&move_collection.to)?;
        if move_collection.from == move_collection.to {
            return Err(invalid(
                "local-organize-conflict",
                "move_collection from and to must differ",
            ));
        }
    }
    Ok(())
}

fn validate_key(key: &str) -> ZotResult<()> {
    if key.len() != 8
        || !key
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
    {
        return Err(invalid(
            "local-api-key",
            "Expected an eight-character Zotero object key",
        ));
    }
    Ok(())
}

fn validate_plan_shape(
    plan: &OrganizePlan,
    ctx: &AppContext,
    max_affected: usize,
) -> ZotResult<()> {
    if plan.schema_version != SCHEMA_VERSION || plan.backend != "local" {
        return Err(invalid(
            "local-organize-schema",
            "Plan schema or backend is not supported",
        ));
    }
    if plan.max_affected != max_affected {
        return Err(invalid(
            "local-organize-max-affected",
            "plan and apply must use the same --max-affected and be reviewed again",
        ));
    }
    if plan.changed_count > plan.max_affected {
        return Err(invalid(
            "local-organize-max-affected",
            "Plan changes more items than --max-affected",
        ));
    }
    if library_label(&ctx.scope) != plan.library || ctx.profile != plan.profile {
        return Err(invalid(
            "local-organize-scope",
            "Plan library or profile does not match this command",
        ));
    }
    if plan.items.len() != plan.item_count {
        return Err(invalid(
            "local-organize-schema",
            "Plan item count does not match its items",
        ));
    }
    validate_request(&OrganizeRequest {
        schema_version: plan.schema_version,
        items: plan
            .items
            .iter()
            .map(|item| item.original_request.clone())
            .collect(),
    })?;
    let changed = plan.items.iter().filter(|item| item.changed).count();
    let actions: usize = plan
        .items
        .iter()
        .map(|item| action_count_of(&item.original_request))
        .sum();
    if changed != plan.changed_count
        || plan.unchanged_count != plan.items.len() - changed
        || actions != plan.action_count
    {
        return Err(invalid(
            "local-organize-schema",
            "Plan summary does not match its items",
        ));
    }
    for item in &plan.items {
        if item.item_key != item.original_request.item_key {
            return Err(invalid(
                "local-organize-schema",
                "Plan item and request identities differ",
            ));
        }
        // Recompute from the reviewed before state, never trust an arbitrary patch.
        let before = LocalObject {
            key: item.item_key.clone(),
            version: item.expected_version,
            data: item.before.clone(),
        };
        let snapshot = LocalCollectionSnapshot {
            library_version: 0,
            collections: item
                .collections
                .iter()
                .map(|collection| LocalCollection {
                    key: collection.key.clone(),
                    name: collection.name.clone(),
                    version: collection.version,
                    parent: None,
                    deleted: false,
                })
                .collect(),
        };
        let recomputed = plan_item(&before, &item.original_request, &snapshot)?;
        if !maps_match(&recomputed.before, &item.before)
            || !maps_match(&recomputed.after, &item.after)
            || recomputed.changed != item.changed
            || recomputed.collections != item.collections
        {
            return Err(invalid(
                "local-organize-schema",
                "Plan fields or change flags do not match original_request",
            ));
        }
    }
    Ok(())
}

async fn build_plan(
    client: &LocalApiClient,
    ctx: &AppContext,
    request: &OrganizeRequest,
    max_affected: usize,
) -> ZotResult<OrganizePlan> {
    validate_request(request)?;
    let probe = client.probe().await?;
    let server = probe.writable_server_id()?.to_string();
    let snapshot = client.collection_snapshot(&server, &ctx.scope).await?;
    let mut items = Vec::new();
    let mut action_count = 0usize;
    for request_item in &request.items {
        let item = client
            .item(&server, &ctx.scope, &request_item.item_key)
            .await?;
        require_bibliographic_item(&item)?;
        let planned = plan_item(&item, request_item, &snapshot)?;
        action_count += action_count_of(request_item);
        items.push(planned);
    }
    let changed_count = items.iter().filter(|item| item.changed).count();
    if changed_count > max_affected {
        return Err(invalid(
            "local-organize-max-affected",
            &format!(
                "Organize plan changes {changed_count} items, exceeding --max-affected {max_affected}"
            ),
        ));
    }
    Ok(OrganizePlan {
        schema_version: SCHEMA_VERSION,
        backend: "local".to_string(),
        server_id: server,
        library: library_label(&ctx.scope),
        profile: ctx.profile.clone(),
        max_affected,
        item_count: items.len(),
        action_count,
        changed_count,
        unchanged_count: items.len() - changed_count,
        items,
    })
}

fn plan_item(
    item: &LocalObject,
    request: &ItemRequest,
    snapshot: &LocalCollectionSnapshot,
) -> ZotResult<PlanItem> {
    let mut before = Map::new();
    let mut after = Map::new();
    let mut collections = Vec::new();
    let mut changed = false;
    if !request.add_tags.is_empty() || !request.remove_tags.is_empty() {
        let removed = plan_tags(item.data.get("tags"), &request.remove_tags, true)?;
        let next = plan_tags(Some(&Value::Array(removed)), &request.add_tags, false)?;
        let current = item.data.get("tags").cloned().unwrap_or_else(|| json!([]));
        let next_value = Value::Array(next);
        before.insert("tags".to_string(), current.clone());
        after.insert("tags".to_string(), next_value.clone());
        if !tags_match(item.data.get("tags"), &next_value) {
            changed = true;
        }
    }
    if has_membership(request) {
        let current = collection_keys(item)?;
        let next = next_collections(&current, request)?;
        for key in referenced_collections(request) {
            collections.push(collection_identity(snapshot, &key)?);
        }
        before.insert("collections".to_string(), json!(current));
        after.insert("collections".to_string(), json!(next));
        if !same_keys(&current, &next) {
            changed = true;
        }
    }
    Ok(PlanItem {
        item_key: item.key.clone(),
        title: item
            .data
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        expected_version: item.version,
        original_request: request.clone(),
        changed,
        before,
        after,
        collections,
    })
}

fn next_collections(current: &[String], request: &ItemRequest) -> ZotResult<Vec<String>> {
    let mut next = current.to_vec();
    for key in &request.remove_collections {
        next = next_memberships(&next, None, Some(key))?;
    }
    for key in &request.add_collections {
        next = next_memberships(&next, Some(key), None)?;
    }
    if let Some(move_collection) = &request.move_collection {
        next = next_memberships(
            &next,
            Some(&move_collection.to),
            Some(&move_collection.from),
        )?;
    }
    Ok(next)
}

fn referenced_collections(request: &ItemRequest) -> Vec<String> {
    let mut keys = request
        .add_collections
        .iter()
        .chain(&request.remove_collections)
        .cloned()
        .collect::<Vec<_>>();
    if let Some(move_collection) = &request.move_collection {
        keys.push(move_collection.from.clone());
        keys.push(move_collection.to.clone());
    }
    keys.sort();
    keys.dedup();
    keys
}

fn collection_identity(
    snapshot: &LocalCollectionSnapshot,
    key: &str,
) -> ZotResult<CollectionIdentity> {
    let Some(collection) = snapshot.collections.iter().find(|item| item.key == key) else {
        return Err(invalid(
            "local-write-target",
            &format!("Collection {key} was not found in this library"),
        ));
    };
    if collection.deleted {
        return Err(invalid(
            "local-write-target",
            &format!("Collection {key} is deleted"),
        ));
    }
    Ok(CollectionIdentity {
        key: collection.key.clone(),
        name: collection.name.clone(),
        version: collection.version,
    })
}

fn collection_keys(item: &LocalObject) -> ZotResult<Vec<String>> {
    let Some(values) = item.data.get("collections") else {
        return Ok(Vec::new());
    };
    let values = values
        .as_array()
        .ok_or_else(|| invalid("local-api-protocol", "Item collections must be an array"))?;
    values
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(ToOwned::to_owned)
                .ok_or_else(|| invalid("local-api-protocol", "Invalid collection membership"))
        })
        .collect()
}

fn has_membership(request: &ItemRequest) -> bool {
    !request.add_collections.is_empty()
        || !request.remove_collections.is_empty()
        || request.move_collection.is_some()
}

fn action_count_of(request: &ItemRequest) -> usize {
    request.add_tags.len()
        + request.remove_tags.len()
        + request.add_collections.len()
        + request.remove_collections.len()
        + usize::from(request.move_collection.is_some())
}

fn same_keys(left: &[String], right: &[String]) -> bool {
    let mut left = left.iter().collect::<Vec<_>>();
    let mut right = right.iter().collect::<Vec<_>>();
    left.sort();
    right.sort();
    left == right
}

async fn preflight(
    client: &LocalApiClient,
    ctx: &AppContext,
    plan: &OrganizePlan,
) -> ZotResult<()> {
    let probe = client.probe().await?;
    let server = probe.writable_server_id()?.to_string();
    if server != plan.server_id {
        return Err(invalid(
            "local-api-conflict",
            "Zotero instance changed; preview again",
        ));
    }
    let snapshot = client.collection_snapshot(&server, &ctx.scope).await?;
    for item in &plan.items {
        let live = client.item(&server, &ctx.scope, &item.item_key).await?;
        require_bibliographic_item(&live)?;
        if live.version != item.expected_version {
            return Err(invalid(
                "local-api-conflict",
                &format!("Item {} version changed; preview again", item.item_key),
            ));
        }
        let recomputed = plan_item(&live, &item.original_request, &snapshot)?;
        if !maps_match(&recomputed.before, &item.before)
            || !maps_match(&recomputed.after, &item.after)
            || recomputed.changed != item.changed
            || recomputed.title != item.title
        {
            return Err(invalid(
                "local-organize-schema",
                "Plan after does not match a fresh calculation from original_request",
            ));
        }
        for collection in &item.collections {
            let live = collection_identity(&snapshot, &collection.key)?;
            if live.version != collection.version || live.name != collection.name {
                return Err(invalid(
                    "local-api-conflict",
                    &format!("Collection {} changed; preview again", collection.key),
                ));
            }
        }
    }
    Ok(())
}

fn maps_match(left: &Map<String, Value>, right: &Map<String, Value>) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.iter().all(|(field, value)| match right.get(field) {
        Some(other) if field == "tags" => tags_match(Some(value), other),
        Some(other) if field == "collections" => same_json_keys(value, other),
        Some(other) => other == value,
        None => false,
    })
}

fn same_json_keys(left: &Value, right: &Value) -> bool {
    match (left.as_array(), right.as_array()) {
        (Some(left), Some(right)) => {
            let left = left
                .iter()
                .map(|value| value.as_str().map(ToOwned::to_owned))
                .collect::<Option<Vec<_>>>();
            let right = right
                .iter()
                .map(|value| value.as_str().map(ToOwned::to_owned))
                .collect::<Option<Vec<_>>>();
            matches!((left, right), (Some(left), Some(right)) if same_keys(&left, &right))
        }
        _ => false,
    }
}

fn initial_report(plan: &OrganizePlan, hash: &str) -> OrganizeReport {
    let now = Utc::now().to_rfc3339();
    OrganizeReport {
        schema_version: SCHEMA_VERSION,
        plan_sha256: hash.to_string(),
        server_id: plan.server_id.clone(),
        library: plan.library.clone(),
        state: "in-progress".to_string(),
        started_at: now.clone(),
        updated_at: now,
        item_count: plan.items.len(),
        applied_count: 0,
        unchanged_count: 0,
        failed_count: 0,
        unknown_count: 0,
        not_attempted_count: plan.items.len(),
        items: plan
            .items
            .iter()
            .map(|item| ReportItem {
                item_key: item.item_key.clone(),
                status: "not-attempted".to_string(),
                before: item.before.clone(),
                after: item.after.clone(),
                expected_version: item.expected_version,
                observed_version: None,
                write_accepted: false,
                verified: false,
                observed_expected_state: None,
                error: None,
            })
            .collect(),
        error: None,
    }
}

async fn execute_plan(
    client: &LocalApiClient,
    ctx: &AppContext,
    plan: &OrganizePlan,
    report: &mut OrganizeReport,
    report_path: &Path,
    credential_path: &Path,
) -> ZotResult<OrganizeReport> {
    let mut authorization: Option<SecretString> = None;
    for index in 0..plan.items.len() {
        report.items[index].status = "in-flight".to_string();
        if let Err(error) = persist_report(report, report_path) {
            return Ok(stop_for_report_error(report, index, error, false));
        }
        let outcome = submit_item(
            client,
            ctx,
            &plan.server_id,
            &plan.items[index],
            credential_path,
            &mut authorization,
        )
        .await;
        apply_outcome(&mut report.items[index], outcome);
        let stop = matches!(
            report.items[index].status.as_str(),
            "failed" | "conflict" | "unknown" | "verification-failed"
        );
        if stop {
            mark_remaining_not_attempted(report, index + 1);
        }
        if let Err(error) = persist_report(report, report_path) {
            return Ok(stop_for_report_error(report, index, error, true));
        }
        if stop {
            break;
        }
    }
    finish_report(report);
    if let Err(error) = persist_report(report, report_path) {
        return Ok(stop_for_report_error(
            report,
            plan.items.len() - 1,
            error,
            true,
        ));
    }
    Ok(report.clone())
}

async fn submit_item(
    client: &LocalApiClient,
    ctx: &AppContext,
    server: &str,
    item: &PlanItem,
    credential_path: &Path,
    authorization: &mut Option<SecretString>,
) -> ZotResult<SubmitOutcome> {
    let live = client.item(server, &ctx.scope, &item.item_key).await?;
    require_bibliographic_item(&live)?;
    if live.version != item.expected_version {
        return Ok(SubmitOutcome::Conflict);
    }
    check_collections(client, &ctx.scope, server, item).await?;
    if !item.changed {
        return Ok(SubmitOutcome::Unchanged {
            version: live.version,
        });
    }
    let key = match authorization.clone() {
        Some(key) => key,
        None => {
            let key = load_or_authorize(client, ctx, server, credential_path).await?;
            *authorization = Some(key.clone());
            key
        }
    };
    // Authorization can wait on a dialog; collection versions are not guarded by
    // the item's conditional PATCH, so recheck them after authorization too.
    check_collections(client, &ctx.scope, server, item).await?;
    let mut receipt = client
        .patch_item(
            server,
            &ctx.scope,
            &item.item_key,
            item.expected_version,
            &item.after,
            &key,
        )
        .await;
    if matches!(
        &receipt,
        Err(ZotError::LocalApi {
            status: Some(401),
            ..
        })
    ) {
        local_credentials::save(credential_path, ctx.profile.as_deref(), server, None)?;
        let key = authorize(client, ctx, server, credential_path).await?;
        *authorization = Some(key.clone());
        check_collections(client, &ctx.scope, server, item).await?;
        receipt = client
            .patch_item(
                server,
                &ctx.scope,
                &item.item_key,
                item.expected_version,
                &item.after,
                &key,
            )
            .await;
    }
    let receipt = receipt?;
    let accepted = receipt == PatchReceipt::Accepted;
    match client.item(server, &ctx.scope, &item.item_key).await {
        Ok(readback) => Ok(SubmitOutcome::Readback {
            accepted,
            version: readback.version,
            matches: local_patch_matches(&readback, &item.after),
        }),
        Err(error) => Ok(SubmitOutcome::ReadbackFailed { accepted, error }),
    }
}

async fn check_collections(
    client: &LocalApiClient,
    scope: &LibraryScope,
    server: &str,
    item: &PlanItem,
) -> ZotResult<()> {
    for expected in &item.collections {
        let live = client.collection(server, scope, &expected.key).await?;
        if live.version != expected.version
            || live.data.get("name").and_then(Value::as_str) != Some(expected.name.as_str())
            || live
                .data
                .get("deleted")
                .is_some_and(|value| value == &json!(1) || value == &json!(true))
        {
            return Err(invalid(
                "local-api-conflict",
                "Collection changed before submission; preview again",
            ));
        }
    }
    Ok(())
}

enum SubmitOutcome {
    Conflict,
    Unchanged {
        version: u64,
    },
    Readback {
        accepted: bool,
        version: u64,
        matches: bool,
    },
    ReadbackFailed {
        accepted: bool,
        error: ZotError,
    },
}

fn apply_outcome(item: &mut ReportItem, outcome: ZotResult<SubmitOutcome>) {
    match outcome {
        Ok(SubmitOutcome::Unchanged { version }) => {
            item.status = "unchanged".to_string();
            item.observed_version = Some(version);
        }
        Err(error) => {
            let conflict = error.payload().code == "local-api-conflict";
            item.status = if conflict {
                "conflict".to_string()
            } else {
                "failed".to_string()
            };
            item.error = Some(error.payload());
        }
        Ok(SubmitOutcome::Conflict) => {
            item.status = "conflict".to_string();
            item.error = Some(
                invalid(
                    "local-api-conflict",
                    "Item version changed before submission; preview again",
                )
                .payload(),
            );
        }
        Ok(SubmitOutcome::Readback {
            accepted,
            version,
            matches,
        }) => {
            item.write_accepted = accepted;
            item.observed_version = Some(version);
            item.observed_expected_state = Some(matches);
            item.verified = accepted && matches;
            item.status = if item.verified {
                "applied".to_string()
            } else if accepted {
                "verification-failed".to_string()
            } else {
                "unknown".to_string()
            };
        }
        Ok(SubmitOutcome::ReadbackFailed { accepted, error }) => {
            item.write_accepted = accepted;
            item.observed_expected_state = Some(false);
            item.error = Some(error.payload());
            item.status = if accepted {
                "verification-failed".to_string()
            } else {
                "unknown".to_string()
            };
        }
    }
}

fn stop_for_report_error(
    report: &mut OrganizeReport,
    index: usize,
    error: ZotError,
    submitted: bool,
) -> OrganizeReport {
    if !submitted {
        report.items[index].status = "not-attempted".to_string();
        report.items[index].error = Some(error.payload());
    }
    mark_remaining_not_attempted(report, index + 1);
    report.error = Some(
        invalid(
            "local-organize-file",
            "Execution report could not be saved; stdout retains known outcomes, disk may retain in-flight; do not resume automatically",
        )
        .payload(),
    );
    finish_report(report);
    report.clone()
}

fn mark_remaining_not_attempted(report: &mut OrganizeReport, from: usize) {
    for item in report.items.iter_mut().skip(from) {
        if item.status == "pending" || item.status == "in-flight" {
            item.status = "not-attempted".to_string();
        }
    }
}

fn finish_report(report: &mut OrganizeReport) {
    report.applied_count = count(report, "applied");
    report.unchanged_count = count(report, "unchanged");
    report.failed_count = report
        .items
        .iter()
        .filter(|item| matches!(item.status.as_str(), "failed" | "conflict"))
        .count();
    report.unknown_count = report
        .items
        .iter()
        .filter(|item| {
            matches!(
                item.status.as_str(),
                "unknown" | "verification-failed" | "in-flight"
            )
        })
        .count();
    report.not_attempted_count = count(report, "not-attempted");
    report.state = aggregate_state(report).to_string();
    report.updated_at = Utc::now().to_rfc3339();
}

fn count(report: &OrganizeReport, status: &str) -> usize {
    report
        .items
        .iter()
        .filter(|item| item.status == status)
        .count()
}

fn aggregate_state(report: &OrganizeReport) -> &'static str {
    let applied = report.items.iter().any(|item| item.status == "applied");
    let unknown = report.items.iter().any(|item| {
        matches!(
            item.status.as_str(),
            "unknown" | "verification-failed" | "in-flight"
        )
    });
    let clean = report
        .items
        .iter()
        .all(|item| matches!(item.status.as_str(), "applied" | "unchanged"));
    if clean && report.error.is_none() {
        if applied { "applied" } else { "unchanged" }
    } else if applied {
        "partial"
    } else if unknown {
        "unknown"
    } else {
        "failed"
    }
}

fn exit_code(state: &str) -> i32 {
    match state {
        "applied" | "unchanged" => 0,
        _ => 1,
    }
}

fn persist_report(report: &mut OrganizeReport, path: &Path) -> ZotResult<()> {
    let running = report.state == "in-progress";
    finish_report(report);
    if running {
        report.state = "in-progress".to_string();
    }
    write_json(path, report, true)
}

fn write_json(path: &Path, value: &impl Serialize, clobber: bool) -> ZotResult<()> {
    if !clobber && path.exists() {
        return Err(invalid(
            "local-organize-file",
            "Output file already exists and will not be overwritten",
        ));
    }
    let dir = path
        .parent()
        .filter(|dir| !dir.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut bytes = serde_json::to_vec_pretty(value)
        .map_err(|_| invalid("json-serialization", "Could not serialize organize output"))?;
    bytes.push(b'\n');
    let mut temp = NamedTempFile::new_in(dir).map_err(|source| ZotError::Io {
        path: dir.to_path_buf(),
        source,
    })?;
    temp.write_all(&bytes).map_err(|source| ZotError::Io {
        path: temp.path().to_path_buf(),
        source,
    })?;
    temp.as_file().sync_all().map_err(|source| ZotError::Io {
        path: temp.path().to_path_buf(),
        source,
    })?;
    if clobber {
        temp.persist(path).map_err(|error| ZotError::Io {
            path: path.to_path_buf(),
            source: error.error,
        })?;
    } else {
        temp.persist_noclobber(path).map_err(|error| ZotError::Io {
            path: path.to_path_buf(),
            source: error.error,
        })?;
    }
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn library_label(scope: &LibraryScope) -> String {
    match scope {
        LibraryScope::User => "user".to_string(),
        LibraryScope::Group { group_id } => format!("group:{group_id}"),
    }
}

async fn load_or_authorize(
    client: &LocalApiClient,
    ctx: &AppContext,
    server: &str,
    credential_path: &Path,
) -> ZotResult<SecretString> {
    match local_credentials::load(credential_path, ctx.profile.as_deref(), server)? {
        Some(key) => Ok(key),
        None => authorize(client, ctx, server, credential_path).await,
    }
}

async fn authorize(
    client: &LocalApiClient,
    ctx: &AppContext,
    server: &str,
    credential_path: &Path,
) -> ZotResult<SecretString> {
    let authorization = client.authorize(server).await?;
    if authorization.remember {
        local_credentials::save(
            credential_path,
            ctx.profile.as_deref(),
            server,
            Some(authorization.key.clone()),
        )?;
    }
    Ok(authorization.key)
}

fn invalid(code: &str, message: &str) -> ZotError {
    ZotError::InvalidInput {
        code: code.to_string(),
        message: message.to_string(),
        hint: None,
    }
}

#[cfg(test)]
#[path = "organize_tests.rs"]
mod tests;
