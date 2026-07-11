mod client;
mod model;

pub use client::{BRIDGE_BASE_URL, DesktopClient, ensure_matching_instance_id};
pub use model::{
    BRIDGE_PROTOCOL_VERSION, BridgeCapabilityStatus, BridgeHealth, BridgeLibraryScope,
    BridgeLibraryStatus, BridgePairResult, BridgeRevokeResult, BridgeStatus,
    DesktopMergeApplyResult, DesktopMergePreview, LocalHttpStatus,
};
