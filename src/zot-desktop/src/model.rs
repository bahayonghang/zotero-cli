use std::fmt;

use serde::{Deserialize, Serialize};
use zot_core::{LibraryScope, MergeFieldFill, MergeSkippedField};

pub const BRIDGE_PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BridgeHealth {
    #[serde(default)]
    pub instance_id: String,
    pub plugin_version: String,
    pub zotero_version: String,
    pub protocol_version: u32,
    #[serde(default)]
    pub capabilities: Vec<String>,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BridgePairResult {
    pub token: String,
    #[serde(default)]
    pub instance_id: String,
    pub plugin_version: String,
    pub protocol_version: u32,
}

impl fmt::Debug for BridgePairResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BridgePairResult")
            .field("token", &"(redacted)")
            .field("instance_id", &self.instance_id)
            .field("plugin_version", &self.plugin_version)
            .field("protocol_version", &self.protocol_version)
            .finish()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BridgeCapabilityStatus {
    pub name: String,
    pub available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BridgeLibraryStatus {
    pub library_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group_id: Option<i64>,
    pub editable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BridgeStatus {
    pub paired: bool,
    #[serde(default)]
    pub instance_id: String,
    #[serde(default)]
    pub capabilities: Vec<BridgeCapabilityStatus>,
    #[serde(default)]
    pub libraries: Vec<BridgeLibraryStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BridgeRevokeResult {
    pub revoked: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LocalHttpStatus {
    pub available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zotero_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connector_api_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum BridgeLibraryScope {
    User,
    Group { group_id: i64 },
}

impl From<&LibraryScope> for BridgeLibraryScope {
    fn from(value: &LibraryScope) -> Self {
        match value {
            LibraryScope::User => Self::User,
            LibraryScope::Group { group_id } => Self::Group {
                group_id: *group_id,
            },
        }
    }
}

#[derive(Serialize)]
pub(crate) struct MergePreviewRequest<'a> {
    #[serde(flatten)]
    pub(crate) base: BaseRequest<'a>,
    pub(crate) scope: BridgeLibraryScope,
    pub(crate) keeper_key: &'a str,
    pub(crate) source_keys: &'a [String],
}

#[derive(Clone, Deserialize, PartialEq, Eq)]
pub struct DesktopMergePreview {
    pub keeper_key: String,
    pub source_keys: Vec<String>,
    pub metadata_fields_to_fill: Vec<MergeFieldFill>,
    pub skipped_incompatible_fields: Vec<MergeSkippedField>,
    pub tags_to_add: Vec<String>,
    pub collections_to_add: Vec<String>,
    pub relations_to_add: Vec<String>,
    pub children_to_reparent: usize,
    pub skipped_duplicate_attachments: usize,
    pub confirm_required: bool,
    pub plan_id: String,
    pub expires_at: String,
    pub(crate) plan_token: String,
}

impl fmt::Debug for DesktopMergePreview {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DesktopMergePreview")
            .field("keeper_key", &self.keeper_key)
            .field("source_keys", &self.source_keys)
            .field("metadata_fields_to_fill", &self.metadata_fields_to_fill)
            .field(
                "skipped_incompatible_fields",
                &self.skipped_incompatible_fields,
            )
            .field("tags_to_add", &self.tags_to_add)
            .field("collections_to_add", &self.collections_to_add)
            .field("relations_to_add", &self.relations_to_add)
            .field("children_to_reparent", &self.children_to_reparent)
            .field(
                "skipped_duplicate_attachments",
                &self.skipped_duplicate_attachments,
            )
            .field("confirm_required", &self.confirm_required)
            .field("plan_id", &self.plan_id)
            .field("expires_at", &self.expires_at)
            .field("plan_token", &"(redacted)")
            .finish()
    }
}

impl DesktopMergePreview {
    pub fn take_plan_token(&mut self) -> String {
        std::mem::take(&mut self.plan_token)
    }
}

#[derive(Serialize)]
pub(crate) struct MergeApplyRequest<'a> {
    #[serde(flatten)]
    pub(crate) base: BaseRequest<'a>,
    pub(crate) plan_token: &'a str,
    pub(crate) operation_id: &'a str,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct DesktopMergeApplyResult {
    #[serde(default)]
    pub already_applied: bool,
    pub keeper_key: String,
    pub source_keys_trashed: Vec<String>,
    pub metadata_fields_filled: Vec<String>,
    pub skipped_incompatible_fields: Vec<MergeSkippedField>,
    pub tags_added: Vec<String>,
    pub collections_added: Vec<String>,
    pub relations_to_add: Vec<String>,
    pub children_reparented: usize,
    pub skipped_duplicate_attachments: usize,
}

#[derive(Debug, Serialize)]
pub(crate) struct ClientIdentity<'a> {
    pub(crate) name: &'static str,
    pub(crate) version: &'a str,
}

#[derive(Debug, Serialize)]
pub(crate) struct BaseRequest<'a> {
    pub(crate) protocol_version: u32,
    pub(crate) request_id: String,
    pub(crate) sent_at: String,
    pub(crate) client: ClientIdentity<'a>,
}

#[derive(Serialize)]
pub(crate) struct PairRequest<'a> {
    #[serde(flatten)]
    pub(crate) base: BaseRequest<'a>,
    pub(crate) code: &'a str,
    pub(crate) client_instance_id: &'a str,
}

#[derive(Debug, Deserialize)]
pub(crate) struct BridgeResponseMeta {
    pub(crate) request_id: String,
    pub(crate) protocol_version: u32,
    pub(crate) plugin_version: String,
    pub(crate) zotero_version: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct BridgeResponseError {
    pub(crate) code: String,
    pub(crate) message: String,
    pub(crate) hint: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct BridgeEnvelope<T> {
    pub(crate) ok: bool,
    pub(crate) data: Option<T>,
    pub(crate) error: Option<BridgeResponseError>,
    pub(crate) meta: Option<BridgeResponseMeta>,
}
