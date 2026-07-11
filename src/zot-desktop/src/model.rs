use std::fmt;

use serde::{Deserialize, Serialize};

pub const BRIDGE_PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BridgeHealth {
    pub plugin_version: String,
    pub zotero_version: String,
    pub protocol_version: u32,
    #[serde(default)]
    pub capabilities: Vec<String>,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BridgePairResult {
    pub token: String,
    pub plugin_version: String,
    pub protocol_version: u32,
}

impl fmt::Debug for BridgePairResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BridgePairResult")
            .field("token", &"(redacted)")
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
