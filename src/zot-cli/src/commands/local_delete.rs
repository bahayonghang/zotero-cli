//! Local single-key collection delete. Planning never opens SQLite or requests authorization.
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::Path;

use anyhow::Result;
use serde::Serialize;
use sha2::{Digest, Sha256};
use zot_core::{ErrorPayload, LibraryScope, SecretString, ZotError, ZotResult};
use zot_desktop::{DeleteReceipt, LocalApiClient, LocalCollection, LocalCollectionSnapshot};

use crate::cli::{CollectionDeleteArgs, LocalDeleteArgs};
use crate::context::AppContext;
use crate::output::CommandOutput;

use super::local_credentials;

const EFFECT: &str = "Permanently erases this collection and every descendant collection. Items stay in the library and lose only these memberships.";
const EMPTY_DESCENDANTS_SHA256: &str =
    "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct Descendant {
    key: String,
    name: String,
    parent: Option<String>,
    version: u64,
    deleted: bool,
}

#[derive(Debug, Serialize)]
struct CollectionDeleteReport {
    backend: &'static str,
    operation: &'static str,
    state: &'static str,
    server_id: String,
    library: LibraryScope,
    library_version: u64,
    collection_key: String,
    name: String,
    parent_key: Option<String>,
    version: u64,
    already_trashed: bool,
    descendants: Vec<Descendant>,
    descendant_count: usize,
    descendants_sha256: String,
    effect: &'static str,
    write_accepted: bool,
    verified: bool,
    observed_expected_state: Option<bool>,
    error: Option<ErrorPayload>,
}

pub(crate) fn validate_web_options(args: &LocalDeleteArgs) -> ZotResult<()> {
    if args.confirm
        || args.expected_server_id.is_some()
        || args.expected_version.is_some()
        || args.expected_descendants_sha256.is_some()
    {
        return Err(super::local_write::invalid(
            "local-write-options",
            "Confirmation and expected-state flags require --write-backend local",
        ));
    }
    Ok(())
}

pub(crate) async fn delete(ctx: &AppContext, args: CollectionDeleteArgs) -> Result<CommandOutput> {
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
    let code = exit_code(report.state);
    CommandOutput::new(ctx, report, None, |report| {
        println!(
            "{}: {} ({})",
            report.state, report.name, report.collection_key
        );
        println!("{}", report.effect);
        println!(
            "{}",
            serde_json::to_string_pretty(report).expect("serialize collection delete report")
        );
    })
    .map(|output| output.with_exit_code(code))
}

fn exit_code(state: &str) -> i32 {
    match state {
        "preview" | "applied" => 0,
        _ => 1,
    }
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

fn validate_hash_format(expected: Option<&str>) -> ZotResult<()> {
    let Some(expected) = expected.map(str::trim) else {
        return Err(super::local_write::invalid(
            "local-write-options",
            "Expected descendants SHA-256 must be 64 hexadecimal characters",
        ));
    };
    if expected.len() != 64 || !expected.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(super::local_write::invalid(
            "local-write-options",
            "Expected descendants SHA-256 must be 64 hexadecimal characters",
        ));
    }
    Ok(())
}

fn require_server(expected: Option<&str>, actual: &str) -> ZotResult<()> {
    if expected != Some(actual) {
        return Err(super::local_write::invalid(
            "local-api-conflict",
            "Expected Server ID is missing or changed; preview again",
        ));
    }
    Ok(())
}

fn require_version(expected: Option<u64>, actual: u64) -> ZotResult<()> {
    if expected != Some(actual) {
        return Err(super::local_write::invalid(
            "local-api-conflict",
            "Expected collection version is missing or changed; preview again",
        ));
    }
    Ok(())
}

fn require_descendant_hash(expected: Option<&str>, actual: &str) -> ZotResult<()> {
    validate_hash_format(expected)?;
    let Some(expected) = expected.map(str::trim) else {
        return Err(super::local_write::invalid(
            "local-write-options",
            "Expected descendants SHA-256 must be 64 hexadecimal characters",
        ));
    };
    if !expected.eq_ignore_ascii_case(actual) {
        return Err(super::local_write::invalid(
            "local-delete-descendants",
            "Expected descendant set does not match this preview; preview again",
        ));
    }
    Ok(())
}

async fn plan(
    client: &LocalApiClient,
    scope: &LibraryScope,
    args: &CollectionDeleteArgs,
) -> ZotResult<CollectionDeleteReport> {
    validate_object_key(&args.key)?;
    if args.write.confirm {
        validate_hash_format(args.write.expected_descendants_sha256.as_deref())?;
    }
    let probe = client.probe().await?;
    let server = probe.writable_server_id()?.to_string();
    if args.write.confirm {
        require_server(args.write.expected_server_id.as_deref(), &server)?;
    }
    let snapshot = client.collection_snapshot(&server, scope).await?;
    build_report(&server, scope, snapshot, args)
}

fn build_report(
    server: &str,
    scope: &LibraryScope,
    snapshot: LocalCollectionSnapshot,
    args: &CollectionDeleteArgs,
) -> ZotResult<CollectionDeleteReport> {
    validate_object_key(&args.key)?;
    if args.write.confirm {
        validate_hash_format(args.write.expected_descendants_sha256.as_deref())?;
        require_server(args.write.expected_server_id.as_deref(), server)?;
    }
    let (target, descendants) = descendant_tree(&snapshot, &args.key)?;
    let descendants_sha256 = descendants_sha256(&descendants);
    if args.write.confirm {
        require_version(args.write.expected_version, target.version)?;
        require_descendant_hash(
            args.write.expected_descendants_sha256.as_deref(),
            &descendants_sha256,
        )?;
    }
    Ok(CollectionDeleteReport {
        backend: "local",
        operation: "delete-collection",
        state: "preview",
        server_id: server.to_string(),
        library: scope.clone(),
        library_version: snapshot.library_version,
        collection_key: target.key.clone(),
        name: target.name.clone(),
        parent_key: target.parent.clone(),
        version: target.version,
        already_trashed: target.deleted,
        descendant_count: descendants.len(),
        descendants,
        descendants_sha256,
        effect: EFFECT,
        write_accepted: false,
        verified: false,
        observed_expected_state: None,
        error: None,
    })
}

fn descendant_tree<'a>(
    snapshot: &'a LocalCollectionSnapshot,
    target_key: &str,
) -> ZotResult<(&'a LocalCollection, Vec<Descendant>)> {
    let mut seen = BTreeSet::new();
    for collection in &snapshot.collections {
        if !seen.insert(collection.key.as_str()) {
            return Err(super::local_write::invalid(
                "local-api-protocol",
                "Collection snapshot repeated an object key",
            ));
        }
    }
    let target = snapshot
        .collections
        .iter()
        .find(|collection| collection.key == target_key)
        .ok_or_else(|| {
            super::local_write::invalid(
                "local-write-target",
                "Collection was not found in this library",
            )
        })?;
    let mut children: BTreeMap<&str, Vec<&LocalCollection>> = BTreeMap::new();
    for collection in &snapshot.collections {
        if let Some(parent) = collection.parent.as_deref() {
            children.entry(parent).or_default().push(collection);
        }
    }
    let mut queue = VecDeque::new();
    if let Some(kids) = children.get(target.key.as_str()) {
        queue.extend(kids.iter().copied());
    }
    let mut visited = BTreeSet::new();
    visited.insert(target.key.as_str());
    let mut found = Vec::new();
    while let Some(collection) = queue.pop_front() {
        if !visited.insert(collection.key.as_str()) {
            return Err(super::local_write::invalid(
                "local-api-protocol",
                "Collection descendant tree is cyclic",
            ));
        }
        found.push(collection);
        if let Some(kids) = children.get(collection.key.as_str()) {
            queue.extend(kids.iter().copied());
        }
    }
    found.sort_by(|left, right| left.key.as_bytes().cmp(right.key.as_bytes()));
    let descendants = found
        .into_iter()
        .map(|collection| Descendant {
            key: collection.key.clone(),
            name: collection.name.clone(),
            parent: collection.parent.clone(),
            version: collection.version,
            deleted: collection.deleted,
        })
        .collect();
    Ok((target, descendants))
}

fn descendants_sha256(descendants: &[Descendant]) -> String {
    if descendants.is_empty() {
        return EMPTY_DESCENDANTS_SHA256.to_string();
    }
    let mut payload = String::new();
    for (index, descendant) in descendants.iter().enumerate() {
        if index > 0 {
            payload.push('\n');
        }
        payload.push_str(&descendant.key);
    }
    sha256_hex(payload.as_bytes())
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

async fn execute(
    client: &LocalApiClient,
    mut report: CollectionDeleteReport,
    args: &LocalDeleteArgs,
    credential_path: &Path,
    profile: Option<&str>,
) -> ZotResult<CollectionDeleteReport> {
    if !args.confirm {
        return Ok(report);
    }
    require_server(args.expected_server_id.as_deref(), &report.server_id)?;
    require_version(args.expected_version, report.version)?;
    require_descendant_hash(
        args.expected_descendants_sha256.as_deref(),
        &report.descendants_sha256,
    )?;
    let server = report.server_id.clone();
    let mut key = match local_credentials::load(credential_path, profile, &server)? {
        Some(key) => key,
        None => authorize(client, &server, credential_path, profile).await?,
    };
    // A new child does not bump the parent object version, so the descendant
    // key set is read again immediately before DELETE. The 401 retry authorizes
    // again and must re-read too.
    let mut version = confirm_live_descendants(client, &report, args).await?;
    let mut receipt = client
        .delete_collection(
            &server,
            &report.library,
            &report.collection_key,
            version,
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
        version = confirm_live_descendants(client, &report, args).await?;
        receipt = client
            .delete_collection(
                &server,
                &report.library,
                &report.collection_key,
                version,
                &key,
            )
            .await;
    }
    let receipt = receipt?;
    match receipt {
        DeleteReceipt::Accepted => verify_absent(client, &mut report).await,
        DeleteReceipt::Unknown => {
            report.state = "unknown";
            report.error = Some(
                super::local_write::invalid(
                    "local-api-unknown",
                    "Collection delete submission outcome is unknown; do not retry or treat a later 404 as this delete",
                )
                .payload(),
            );
        }
        DeleteReceipt::Absent => {
            report.state = "absent";
            report.error = Some(
                super::local_write::invalid(
                    "local-delete-absent",
                    "The collection was already absent when DELETE ran; this call did not delete it",
                )
                .payload(),
            );
        }
    }
    Ok(report)
}

async fn confirm_live_descendants(
    client: &LocalApiClient,
    report: &CollectionDeleteReport,
    args: &LocalDeleteArgs,
) -> ZotResult<u64> {
    let snapshot = client
        .collection_snapshot(&report.server_id, &report.library)
        .await?;
    let (target, descendants) = descendant_tree(&snapshot, &report.collection_key)?;
    let descendants_sha256 = descendants_sha256(&descendants);
    require_version(args.expected_version, target.version)?;
    require_descendant_hash(
        args.expected_descendants_sha256.as_deref(),
        &descendants_sha256,
    )?;
    Ok(target.version)
}

async fn verify_absent(client: &LocalApiClient, report: &mut CollectionDeleteReport) {
    report.write_accepted = true;
    match client
        .collection(&report.server_id, &report.library, &report.collection_key)
        .await
    {
        Err(error) if error.payload().code == "local-api-not-found" => {
            report.state = "applied";
            report.verified = true;
            report.observed_expected_state = Some(true);
        }
        Ok(_) => {
            report.state = "verification-failed";
            report.verified = false;
            report.observed_expected_state = Some(false);
        }
        Err(error) => {
            report.error = Some(error.payload());
            report.state = "verification-failed";
            report.verified = false;
            report.observed_expected_state = Some(false);
        }
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
#[path = "local_delete_tests.rs"]
mod tests;
