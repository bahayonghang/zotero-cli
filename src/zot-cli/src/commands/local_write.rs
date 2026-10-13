//! Single-item local mutations. Planning never opens SQLite or requests authorization.
use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Result;
use serde::Serialize;
use serde_json::{Map, Value, json};
use zot_core::{ErrorPayload, LibraryScope, SecretString, ZotError, ZotResult};
use zot_desktop::{LocalApiClient, LocalObject, PatchReceipt};

use crate::cli::{ItemUpdateArgs, LocalWriteArgs};
use crate::context::AppContext;
use crate::output::CommandOutput;

use super::local_credentials;

#[derive(Debug, Serialize)]
struct MutationReport {
    backend: &'static str,
    state: &'static str,
    operation: &'static str,
    server_id: String,
    library: LibraryScope,
    item_key: String,
    title: String,
    expected_version: u64,
    before: Map<String, Value>,
    after: Map<String, Value>,
    skipped_fields: Vec<String>,
    collections: BTreeMap<String, String>,
    write_accepted: bool,
    verified: bool,
    observed_expected_state: Option<bool>,
    error: Option<ErrorPayload>,
}

impl MutationReport {
    fn new(
        server: String,
        scope: &LibraryScope,
        item: &LocalObject,
        operation: &'static str,
    ) -> Self {
        Self {
            backend: "local",
            state: "preview",
            operation,
            server_id: server,
            library: scope.clone(),
            item_key: item.key.clone(),
            title: item
                .data
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            expected_version: item.version,
            before: Map::new(),
            after: Map::new(),
            skipped_fields: Vec::new(),
            collections: BTreeMap::new(),
            write_accepted: false,
            verified: false,
            observed_expected_state: None,
            error: None,
        }
    }

    fn change(&mut self, item: &LocalObject, field: &str, value: Value) {
        let before = item.data.get(field).cloned().unwrap_or(Value::Null);
        if before != value {
            self.before.insert(field.to_string(), before);
            self.after.insert(field.to_string(), value);
        }
    }
}

pub(super) fn invalid(code: &str, message: &str) -> ZotError {
    ZotError::InvalidInput {
        code: code.to_string(),
        message: message.to_string(),
        hint: None,
    }
}

pub(crate) fn validate_web_options(args: &LocalWriteArgs) -> ZotResult<()> {
    if args.confirm || args.expected_server_id.is_some() || args.expected_version.is_some() {
        return Err(invalid(
            "local-write-options",
            "Confirmation and expected-state flags require --write-backend local",
        ));
    }
    Ok(())
}

async fn read_target(
    client: &LocalApiClient,
    scope: &LibraryScope,
    key: &str,
    args: &LocalWriteArgs,
) -> ZotResult<(String, LocalObject)> {
    let probe = client.probe().await?;
    let server = probe.writable_server_id()?.to_string();
    if args.confirm && args.expected_server_id.as_deref() != Some(&server) {
        return Err(invalid(
            "local-api-conflict",
            "Expected Server ID is missing or changed; preview again",
        ));
    }
    let item = client.item(&server, scope, key).await?;
    if args.confirm && args.expected_version != Some(item.version) {
        return Err(invalid(
            "local-api-conflict",
            "Expected local item version is missing or changed; preview again",
        ));
    }
    let item_type = item
        .data
        .get("itemType")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("local-api-protocol", "Item has no itemType"))?;
    if matches!(item_type, "note" | "attachment" | "annotation")
        || item
            .data
            .get("parentItem")
            .is_some_and(|v| v.as_str().is_some_and(|s| !s.is_empty()))
        || deleted(&item.data)
    {
        return Err(invalid(
            "local-write-target",
            "Local content operations require a non-trashed top-level bibliographic item",
        ));
    }
    Ok((server, item))
}

fn deleted(data: &Map<String, Value>) -> bool {
    data.get("deleted")
        .is_some_and(|v| v == &json!(1) || v == &json!(true))
}

pub(crate) async fn update(ctx: &AppContext, args: ItemUpdateArgs) -> Result<CommandOutput> {
    let fields = parse_fields(&args)?;
    let creators = args
        .creators_file
        .as_deref()
        .map(read_creators)
        .transpose()?;
    if fields.is_empty() && creators.is_none() {
        return Err(invalid("local-write-empty", "Provide a field or --creators-file").into());
    }
    let client = ctx.local_api()?;
    let (server, item) = read_target(&client, &ctx.scope, &args.key, &args.write).await?;
    let item_type = item.data["itemType"]
        .as_str()
        .ok_or_else(|| invalid("local-api-protocol", "Missing itemType"))?;
    let allowed_fields = client.fields(&server, item_type).await?;
    let creator_types = if creators.is_some() {
        client.creator_types(&server, item_type).await?
    } else {
        Vec::new()
    };
    let mut report = MutationReport::new(server, &ctx.scope, &item, "update");
    plan_fields(
        &mut report,
        &item,
        fields,
        creators,
        &allowed_fields,
        &creator_types,
        args.overwrite,
    )?;
    complete(ctx, &client, report, &args.write).await
}

fn parse_fields(args: &ItemUpdateArgs) -> ZotResult<BTreeMap<String, String>> {
    let mut fields = BTreeMap::new();
    let pairs = args
        .title
        .iter()
        .map(|s| ("title", s.as_str()))
        .chain(args.date.iter().map(|s| ("date", s.as_str())))
        .map(|(k, v)| Ok((k, v)))
        .chain(args.fields.iter().map(|field| {
            field
                .split_once('=')
                .ok_or_else(|| invalid("local-write-field", "Each --field must be NAME=VALUE"))
        }));
    for pair in pairs {
        let (key, value) = pair?;
        if key.is_empty() || value.trim().is_empty() {
            return Err(invalid(
                "local-write-field",
                "Field name and value must be nonempty",
            ));
        }
        if let Some(previous) = fields.insert(key.to_string(), value.to_string()) {
            if previous != value {
                return Err(invalid(
                    "local-write-field",
                    "Conflicting values for the same field",
                ));
            }
        }
    }
    Ok(fields)
}

fn read_creators(path: &Path) -> ZotResult<Value> {
    let bytes = std::fs::read(path).map_err(|source| ZotError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_slice(&bytes).map_err(|_| {
        invalid(
            "local-write-creators",
            "Creators file must contain a JSON array",
        )
    })
}

fn plan_fields(
    report: &mut MutationReport,
    item: &LocalObject,
    fields: BTreeMap<String, String>,
    creators: Option<Value>,
    allowed: &[String],
    creator_types: &[String],
    overwrite: bool,
) -> ZotResult<()> {
    const RESERVED: &[&str] = &[
        "key",
        "version",
        "library",
        "itemType",
        "parentItem",
        "creators",
        "tags",
        "collections",
        "relations",
        "deleted",
        "dateAdded",
        "dateModified",
    ];
    for (field, value) in fields {
        if RESERVED.contains(&field.as_str()) || !allowed.contains(&field) {
            return Err(invalid(
                "local-write-field",
                &format!("Field '{field}' cannot be edited for this item type"),
            ));
        }
        let current = item.data.get(&field);
        if !overwrite
            && current.is_some_and(|v| v.as_str().is_none_or(|s| !s.trim().is_empty()))
            && current != Some(&Value::String(value.clone()))
        {
            report.skipped_fields.push(field);
        } else {
            report.change(item, &field, Value::String(value));
        }
    }
    if let Some(creators) = creators {
        validate_creators(&creators, creator_types)?;
        let current = item
            .data
            .get("creators")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid("local-api-protocol", "Item creators must be an array"))?;
        if !overwrite && !current.is_empty() && item.data.get("creators") != Some(&creators) {
            report.skipped_fields.push("creators".to_string());
        } else {
            report.change(item, "creators", creators);
        }
    }
    Ok(())
}

fn validate_creators(creators: &Value, types: &[String]) -> ZotResult<()> {
    let error = || {
        invalid(
            "local-write-creators",
            "Use a nonempty array of valid creators: creatorType with name OR firstName/lastName",
        )
    };
    let entries = creators
        .as_array()
        .filter(|v| !v.is_empty())
        .ok_or_else(error)?;
    for entry in entries {
        let object = entry.as_object().ok_or_else(error)?;
        if object
            .keys()
            .any(|k| !["name", "firstName", "lastName", "creatorType"].contains(&k.as_str()))
            || object.values().any(|v| !v.is_string())
            || !entry
                .get("creatorType")
                .and_then(Value::as_str)
                .is_some_and(|t| types.iter().any(|v| v == t))
        {
            return Err(error());
        }
        let nonempty = |field| {
            entry
                .get(field)
                .and_then(Value::as_str)
                .is_some_and(|s| !s.trim().is_empty())
        };
        if object.contains_key("name") {
            if !nonempty("name")
                || object.contains_key("firstName")
                || object.contains_key("lastName")
            {
                return Err(error());
            }
        } else if !object.contains_key("firstName")
            || !object.contains_key("lastName")
            || (!nonempty("firstName") && !nonempty("lastName"))
        {
            return Err(error());
        }
    }
    Ok(())
}

pub(crate) async fn membership(
    ctx: &AppContext,
    key: &str,
    add: Option<&str>,
    remove: Option<&str>,
    args: &LocalWriteArgs,
) -> Result<CommandOutput> {
    let client = ctx.local_api()?;
    let (server, item) = read_target(&client, &ctx.scope, key, args).await?;
    let operation = match (add, remove) {
        (Some(_), Some(_)) => "move",
        (Some(_), None) => "add",
        _ => "remove",
    };
    let mut report = MutationReport::new(server, &ctx.scope, &item, operation);
    for collection_key in remove.into_iter().chain(add) {
        if report.collections.contains_key(collection_key) {
            continue;
        }
        let collection = client
            .collection(&report.server_id, &ctx.scope, collection_key)
            .await?;
        if deleted(&collection.data) {
            return Err(invalid("local-write-target", "Collection is deleted").into());
        }
        report.collections.insert(
            collection_key.to_string(),
            collection
                .data
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
        );
    }
    let next = membership_patch(&item, add, remove)?;
    report.change(&item, "collections", json!(next));
    complete(ctx, &client, report, args).await
}

fn membership_patch(
    item: &LocalObject,
    add: Option<&str>,
    remove: Option<&str>,
) -> ZotResult<Vec<String>> {
    let current = item
        .data
        .get("collections")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("local-api-protocol", "Item collections must be an array"))?;
    let mut next = current
        .iter()
        .map(|v| {
            v.as_str()
                .map(ToOwned::to_owned)
                .ok_or_else(|| invalid("local-api-protocol", "Invalid collection membership"))
        })
        .collect::<ZotResult<Vec<_>>>()?;
    if add == remove {
        return Ok(next);
    }
    if let (Some(to), Some(from)) = (add, remove) {
        if !next.iter().any(|v| v == from) && !next.iter().any(|v| v == to) {
            return Err(invalid(
                "local-write-source-missing",
                "Item is not in the source collection",
            ));
        }
    }
    if let Some(remove) = remove {
        next.retain(|v| v != remove);
    }
    if let Some(add) = add {
        if !next.iter().any(|v| v == add) {
            next.push(add.to_string());
        }
    }
    Ok(next)
}

async fn complete(
    ctx: &AppContext,
    client: &LocalApiClient,
    report: MutationReport,
    args: &LocalWriteArgs,
) -> Result<CommandOutput> {
    let report = execute(
        client,
        report,
        args,
        &ctx.local_api_credentials_path()?,
        ctx.profile.as_deref(),
    )
    .await?;
    CommandOutput::new(ctx, report, None, |r| {
        println!("{}: {} ({})", r.state, r.title, r.item_key);
        println!(
            "{}",
            serde_json::to_string_pretty(r).expect("serialize local mutation report")
        );
    })
}

async fn authorize(
    client: &LocalApiClient,
    server: &str,
    path: &Path,
    profile: Option<&str>,
) -> ZotResult<SecretString> {
    let authorization = client.authorize(server).await?;
    if authorization.remember {
        local_credentials::save(path, profile, server, Some(authorization.key.clone()))?;
    }
    Ok(authorization.key)
}

async fn execute(
    client: &LocalApiClient,
    mut report: MutationReport,
    args: &LocalWriteArgs,
    credential_path: &Path,
    profile: Option<&str>,
) -> ZotResult<MutationReport> {
    if report.after.is_empty() {
        report.state = "unchanged";
        return Ok(report);
    }
    if !args.confirm {
        return Ok(report);
    }
    let server = &report.server_id;
    if args.expected_server_id.as_deref() != Some(server)
        || args.expected_version != Some(report.expected_version)
    {
        return Err(invalid(
            "local-api-conflict",
            "Preview identity or version changed; preview again",
        ));
    }
    let mut key = match local_credentials::load(credential_path, profile, server)? {
        Some(key) => key,
        None => authorize(client, server, credential_path, profile).await?,
    };
    let mut receipt = client
        .patch_item(
            server,
            &report.library,
            &report.item_key,
            report.expected_version,
            &report.after,
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
        local_credentials::save(credential_path, profile, server, None)?;
        key = authorize(client, server, credential_path, profile).await?;
        receipt = client
            .patch_item(
                server,
                &report.library,
                &report.item_key,
                report.expected_version,
                &report.after,
                &key,
            )
            .await;
    }
    let receipt = receipt?;
    report.write_accepted = receipt == PatchReceipt::Accepted;
    match client.item(server, &report.library, &report.item_key).await {
        Ok(item) => {
            let matches = matches_patch(&item, &report.after);
            report.observed_expected_state = Some(matches);
            report.verified = matches && report.write_accepted;
            report.state = if report.verified {
                "applied"
            } else if report.write_accepted {
                "verification-failed"
            } else {
                "unknown"
            };
        }
        Err(error) => {
            report.error = Some(error.payload());
            report.state = if report.write_accepted {
                "verification-failed"
            } else {
                "unknown"
            };
        }
    }
    Ok(report)
}

fn matches_patch(item: &LocalObject, patch: &Map<String, Value>) -> bool {
    patch.iter().all(|(field, value)| {
        if field == "collections" {
            match (
                item.data.get(field).and_then(Value::as_array),
                value.as_array(),
            ) {
                (Some(actual), Some(expected)) => {
                    actual.len() == expected.len()
                        && expected.iter().all(|key| actual.contains(key))
                }
                _ => false,
            }
        } else {
            item.data.get(field) == Some(value)
        }
    })
}

#[cfg(test)]
#[path = "local_write_tests.rs"]
mod tests;
