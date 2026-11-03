//! Local collection creation. Planning never opens SQLite or requests authorization.
use std::path::Path;

use anyhow::Result;
use serde::Serialize;
use serde_json::{Map, Value};
use zot_core::{ErrorPayload, LibraryScope, SecretString, ZotError, ZotResult};
use zot_desktop::{CreateReceipt, LocalApiClient, LocalCollectionSnapshot, LocalObject};

use crate::cli::{CollectionCreateArgs, LocalCreateArgs};
use crate::context::AppContext;
use crate::output::CommandOutput;

use super::local_credentials;

#[derive(Debug, Serialize)]
struct PathEntry {
    key: String,
    name: String,
}

#[derive(Debug, Serialize)]
struct CollectionCreateReport {
    backend: &'static str,
    state: &'static str,
    operation: &'static str,
    server_id: String,
    library: LibraryScope,
    collection_key: String,
    name: String,
    parent_key: Option<String>,
    parent_name: Option<String>,
    path: Vec<PathEntry>,
    expected_library_version: u64,
    before: Value,
    after: Map<String, Value>,
    write_accepted: bool,
    verified: bool,
    observed_expected_state: Option<bool>,
    error: Option<ErrorPayload>,
}

pub(crate) fn validate_web_options(args: &LocalCreateArgs) -> ZotResult<()> {
    if args.confirm || args.expected_server_id.is_some() || args.expected_library_version.is_some()
    {
        return Err(super::local_write::invalid(
            "local-write-options",
            "Confirmation and expected-library-version flags require --write-backend local",
        ));
    }
    Ok(())
}

pub(crate) async fn create(ctx: &AppContext, args: CollectionCreateArgs) -> Result<CommandOutput> {
    validate_name(&args.name)?;
    if let Some(parent) = args.parent.as_deref() {
        validate_object_key(parent)?;
    }
    let client = ctx.local_api()?;
    let report = plan(&client, &ctx.scope, &args).await?;
    let report = execute(
        &client,
        report,
        &args.write,
        &ctx.local_api_credentials_path()?,
        ctx.profile.as_deref(),
    )
    .await?;
    CommandOutput::new(ctx, report, None, |report| {
        println!(
            "{}: {} ({})",
            report.state, report.name, report.collection_key
        );
        println!(
            "{}",
            serde_json::to_string_pretty(report).expect("serialize collection create report")
        );
    })
}

fn validate_name(name: &str) -> ZotResult<()> {
    if name.trim().is_empty() {
        return Err(super::local_write::invalid(
            "local-write-collection",
            "Collection name must not be blank",
        ));
    }
    Ok(())
}

fn validate_object_key(key: &str) -> ZotResult<()> {
    if key.len() != 8
        || !key
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
    {
        return Err(super::local_write::invalid(
            "local-api-key",
            "Expected an eight-character Zotero object key",
        ));
    }
    Ok(())
}

async fn plan(
    client: &LocalApiClient,
    scope: &LibraryScope,
    args: &CollectionCreateArgs,
) -> ZotResult<CollectionCreateReport> {
    let probe = client.probe().await?;
    let server = probe.writable_server_id()?.to_string();
    if args.write.confirm && args.write.expected_server_id.as_deref() != Some(server.as_str()) {
        return Err(super::local_write::invalid(
            "local-api-conflict",
            "Expected Server ID is missing or changed; preview again",
        ));
    }
    let snapshot = client.collection_snapshot(&server, scope).await?;
    if args.write.confirm && args.write.expected_library_version != Some(snapshot.library_version) {
        return Err(super::local_write::invalid(
            "local-api-conflict",
            "Expected local library version is missing or changed; preview again",
        ));
    }
    let parent = args.parent.clone();
    let path = parent_chain(&snapshot, parent.as_deref())?;
    reject_duplicate(&snapshot, &args.name, parent.as_deref())?;
    let parent_name = path.last().map(|entry| entry.name.clone());
    let mut after = Map::new();
    after.insert("name".to_string(), Value::String(args.name.clone()));
    after.insert(
        "parentCollection".to_string(),
        match &parent {
            Some(key) => Value::String(key.clone()),
            None => Value::Bool(false),
        },
    );
    Ok(CollectionCreateReport {
        backend: "local",
        state: "preview",
        operation: "create-collection",
        server_id: server,
        library: scope.clone(),
        collection_key: String::new(),
        name: args.name.clone(),
        parent_key: parent,
        parent_name,
        path,
        expected_library_version: snapshot.library_version,
        before: Value::Null,
        after,
        write_accepted: false,
        verified: false,
        observed_expected_state: None,
        error: None,
    })
}

fn parent_chain(
    snapshot: &LocalCollectionSnapshot,
    parent: Option<&str>,
) -> ZotResult<Vec<PathEntry>> {
    let Some(mut cursor) = parent else {
        return Ok(Vec::new());
    };
    let mut chain = Vec::new();
    let mut seen = Vec::new();
    loop {
        if seen.iter().any(|key: &String| key == cursor) {
            return Err(super::local_write::invalid(
                "local-api-protocol",
                "Collection parent path is cyclic",
            ));
        }
        seen.push(cursor.to_string());
        let Some(collection) = snapshot.collections.iter().find(|item| item.key == cursor) else {
            return Err(super::local_write::invalid(
                "local-write-target",
                "Parent collection was not found in this library",
            ));
        };
        if collection.deleted {
            return Err(super::local_write::invalid(
                "local-write-target",
                "Parent collection is deleted",
            ));
        }
        chain.push(PathEntry {
            key: collection.key.clone(),
            name: collection.name.clone(),
        });
        match collection.parent.as_deref() {
            Some(parent) => cursor = parent,
            None => break,
        }
    }
    chain.reverse();
    Ok(chain)
}

fn reject_duplicate(
    snapshot: &LocalCollectionSnapshot,
    name: &str,
    parent: Option<&str>,
) -> ZotResult<()> {
    let mut keys = snapshot
        .collections
        .iter()
        .filter(|collection| collection.name == name && collection.parent.as_deref() == parent)
        .map(|collection| collection.key.clone())
        .collect::<Vec<_>>();
    if keys.is_empty() {
        return Ok(());
    }
    keys.sort();
    Err(ZotError::InvalidInput {
        code: "local-collection-duplicate".to_string(),
        message: format!(
            "Collection `{name}` already exists under the same parent: {}",
            keys.join(", ")
        ),
        hint: Some(
            "Choose another name or use an existing key; this command does not reuse or rename matches"
                .to_string(),
        ),
    })
}

async fn execute(
    client: &LocalApiClient,
    mut report: CollectionCreateReport,
    args: &LocalCreateArgs,
    credential_path: &Path,
    profile: Option<&str>,
) -> ZotResult<CollectionCreateReport> {
    if !args.confirm {
        return Ok(report);
    }
    if args.expected_server_id.as_deref() != Some(report.server_id.as_str())
        || args.expected_library_version != Some(report.expected_library_version)
    {
        return Err(super::local_write::invalid(
            "local-api-conflict",
            "Preview identity or library version changed; preview again",
        ));
    }
    let server = report.server_id.clone();
    let mut key = match local_credentials::load(credential_path, profile, &server)? {
        Some(key) => key,
        None => authorize(client, &server, credential_path, profile).await?,
    };
    let mut receipt = client
        .create_collection(
            &server,
            &report.library,
            report.expected_library_version,
            &report.name,
            report.parent_key.as_deref(),
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
        local_credentials::save(credential_path, profile, &server, None)?;
        key = authorize(client, &server, credential_path, profile).await?;
        receipt = client
            .create_collection(
                &server,
                &report.library,
                report.expected_library_version,
                &report.name,
                report.parent_key.as_deref(),
                &key,
            )
            .await;
    }
    let receipt = receipt?;
    let CreateReceipt::Accepted { key: created } = receipt else {
        report.state = "unknown";
        report.error = Some(
            super::local_write::invalid(
                "local-api-unknown",
                "Collection create submission outcome is unknown; do not retry or claim a same-name collection",
            )
            .payload(),
        );
        return Ok(report);
    };
    report.collection_key = created.clone();
    report.write_accepted = true;
    match client.collection(&server, &report.library, &created).await {
        Ok(collection) => {
            let matches = collection_matches(&collection, &report);
            report.observed_expected_state = Some(matches);
            report.verified = matches;
            report.state = if matches {
                "applied"
            } else {
                "verification-failed"
            };
        }
        Err(error) => {
            report.error = Some(error.payload());
            report.observed_expected_state = Some(false);
            report.state = "verification-failed";
        }
    }
    Ok(report)
}

fn collection_matches(collection: &LocalObject, report: &CollectionCreateReport) -> bool {
    collection.key == report.collection_key
        && collection.data.get("name").and_then(Value::as_str) == Some(report.name.as_str())
        && parent_value(collection.data.get("parentCollection")) == report.parent_key.as_deref()
}

fn parent_value(value: Option<&Value>) -> Option<&str> {
    match value {
        None | Some(Value::Null) | Some(Value::Bool(false)) => None,
        Some(Value::String(parent)) if parent.is_empty() => None,
        Some(Value::String(parent)) => Some(parent.as_str()),
        Some(_) => Some("\u{0}"),
    }
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

#[cfg(test)]
#[path = "local_create_tests.rs"]
mod tests;
