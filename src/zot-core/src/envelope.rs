use serde::Serialize;

#[derive(Debug, Clone, Default, Serialize)]
pub struct EnvelopeMeta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
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

#[derive(Debug, Clone, Serialize)]
pub struct EnvelopeError {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

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
}
