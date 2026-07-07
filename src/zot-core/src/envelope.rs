use serde::Serialize;

use crate::error::{ErrorPayload, ZotError};

#[derive(Debug, Clone, Default, Serialize)]
pub struct EnvelopeMeta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_version: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum CliEnvelope<T>
where
    T: Serialize,
{
    Ok {
        ok: bool,
        data: T,
        #[serde(skip_serializing_if = "Option::is_none")]
        meta: Option<EnvelopeMeta>,
    },
    Err {
        ok: bool,
        error: EnvelopeError,
    },
}

/// Alias kept for backwards compatibility; the canonical type is
/// [`ErrorPayload`] in `error.rs`.
pub type EnvelopeError = ErrorPayload;

impl<T> CliEnvelope<T>
where
    T: Serialize,
{
    pub fn ok(data: T) -> Self {
        Self::Ok {
            ok: true,
            data,
            meta: None,
        }
    }

    pub fn ok_with_meta(data: T, meta: EnvelopeMeta) -> Self {
        Self::Ok {
            ok: true,
            data,
            meta: Some(meta),
        }
    }

    pub fn err(error: &ZotError) -> Self {
        Self::Err {
            ok: false,
            error: error.payload(),
        }
    }
}
