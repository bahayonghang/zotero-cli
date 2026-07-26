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
        #[serde(skip_serializing_if = "Option::is_none")]
        meta: Option<EnvelopeMeta>,
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
        Self::err_payload(error.payload())
    }

    pub fn err_with_meta(error: &ZotError, meta: EnvelopeMeta) -> Self {
        Self::err_payload_with_meta(error.payload(), meta)
    }

    pub fn err_payload(error: ErrorPayload) -> Self {
        Self::Err {
            ok: false,
            error,
            meta: None,
        }
    }

    pub fn err_payload_with_meta(error: ErrorPayload, meta: EnvelopeMeta) -> Self {
        Self::Err {
            ok: false,
            error,
            meta: Some(meta),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_payload_with_meta_is_additive_and_versioned() {
        let envelope = CliEnvelope::<serde_json::Value>::err_payload_with_meta(
            ErrorPayload {
                code: "runtime-error".to_string(),
                message: "request failed".to_string(),
                hint: None,
            },
            EnvelopeMeta {
                count: None,
                total: None,
                profile: Some("work".to_string()),
                api_version: Some(1),
            },
        );

        let value = serde_json::to_value(envelope).expect("serialize envelope");
        assert_eq!(value["ok"], false);
        assert_eq!(value["error"]["code"], "runtime-error");
        assert_eq!(value["meta"]["profile"], "work");
        assert_eq!(value["meta"]["api_version"], 1);
    }

    #[test]
    fn legacy_error_constructor_omits_meta() {
        let error = ZotError::InvalidInput {
            code: "bad-input".to_string(),
            message: "bad input".to_string(),
            hint: None,
        };
        let value = serde_json::to_value(CliEnvelope::<serde_json::Value>::err(&error))
            .expect("serialize envelope");

        assert!(value.get("meta").is_none());
    }
}
