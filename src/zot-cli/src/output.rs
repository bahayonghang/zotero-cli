use zot_core::{CliEnvelope, EnvelopeMeta};

use crate::context::AppContext;
use crate::format::{to_pretty_json, EnvelopeMetaSeed, ENVELOPE_API_VERSION};

/// Successful command output. Handlers return this; the dispatch layer calls
/// [`CommandOutput::emit`] to print it. This is the single place where the
/// json-vs-human decision and envelope meta assembly happen.
pub(crate) struct CommandOutput(Payload);

/// Manual impl because the human variant holds a render closure; tests need
/// `Debug` for `Result::expect_err` on handler return values.
impl std::fmt::Debug for CommandOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.0 {
            Payload::Json(text) => f.debug_tuple("CommandOutput::Json").field(text).finish(),
            Payload::Human(_) => f.write_str("CommandOutput::Human(..)"),
            Payload::Silent => f.write_str("CommandOutput::Silent"),
        }
    }
}

enum Payload {
    /// Fully rendered pretty JSON envelope (constructed only when `ctx.json`).
    Json(String),
    /// Human-readable rendering closure (constructed only when not json;
    /// captures owned data, deferred until `emit`).
    Human(Box<dyn FnOnce() + Send>),
    /// No output (completions, mcp serve, etc. write stdout themselves).
    Silent,
}

impl CommandOutput {
    /// The single json/human decision point and the single meta assembly point.
    ///
    /// `data` is taken by value: the json branch serializes it and drops it,
    /// while the human branch moves it into the deferred render closure. Taking
    /// it by value (rather than `&T` plus a separate `move` closure over the
    /// same binding) is what lets a single call site both serialize and render
    /// without a borrow-vs-move conflict.
    pub(crate) fn new<T, F>(
        ctx: &AppContext,
        data: T,
        seed: Option<EnvelopeMetaSeed>,
        human: F,
    ) -> anyhow::Result<Self>
    where
        T: serde::Serialize + Send + 'static,
        F: FnOnce(&T) + Send + 'static,
    {
        if ctx.json {
            let seed = seed.unwrap_or_default();
            let meta = EnvelopeMeta {
                count: seed.count,
                total: seed.total,
                profile: ctx.profile.clone(),
                api_version: Some(ENVELOPE_API_VERSION),
            };
            Ok(Self(Payload::Json(to_pretty_json(
                &CliEnvelope::ok_with_meta(&data, meta),
            )?)))
        } else {
            Ok(Self(Payload::Human(Box::new(move || human(&data)))))
        }
    }

    pub(crate) fn silent() -> Self {
        Self(Payload::Silent)
    }

    /// The single print action, invoked once by the dispatch layer.
    pub(crate) fn emit(self) {
        match self.0 {
            Payload::Json(text) => println!("{text}"),
            Payload::Human(render) => render(),
            Payload::Silent => {}
        }
    }

    /// Test-only: assert the JSON envelope content directly, without stdout.
    #[cfg(test)]
    pub(crate) fn as_json(&self) -> Option<&str> {
        match &self.0 {
            Payload::Json(text) => Some(text),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use zot_core::{AppConfig, LibraryScope};
    use zot_local::PdfiumBackend;
    use zot_remote::HttpRuntime;

    use super::*;

    fn ctx(json: bool) -> AppContext {
        AppContext {
            json,
            profile: Some("default".to_string()),
            scope: LibraryScope::User,
            config: AppConfig::default(),
            http: Arc::new(HttpRuntime::new().expect("http runtime")),
            pdf: Arc::new(PdfiumBackend),
        }
    }

    #[test]
    fn json_output_is_byte_exact_with_seed() {
        let c = ctx(true);
        let data = serde_json::json!({ "key": "WORK001", "title": "Sample" });
        let seed = Some(EnvelopeMetaSeed {
            count: Some(3),
            total: Some(10),
        });
        let out = CommandOutput::new(&c, data, seed, |_| unreachable!()).expect("build");
        assert_eq!(
            out.as_json(),
            Some(
                "{\n  \"ok\": true,\n  \"data\": {\n    \"key\": \"WORK001\",\n    \"title\": \"Sample\"\n  },\n  \"meta\": {\n    \"count\": 3,\n    \"total\": 10,\n    \"profile\": \"default\",\n    \"api_version\": 1\n  }\n}"
            )
        );
    }

    #[test]
    fn json_output_is_byte_exact_without_seed() {
        let c = ctx(true);
        let data = vec!["a".to_string(), "b".to_string()];
        let out = CommandOutput::new(&c, data, None, |_| unreachable!()).expect("build");
        assert_eq!(
            out.as_json(),
            Some(
                "{\n  \"ok\": true,\n  \"data\": [\n    \"a\",\n    \"b\"\n  ],\n  \"meta\": {\n    \"profile\": \"default\",\n    \"api_version\": 1\n  }\n}"
            )
        );
    }

    #[test]
    fn human_output_has_no_json() {
        let c = ctx(false);
        let data = serde_json::json!({ "key": "WORK001" });
        let out = CommandOutput::new(&c, data, None, |_| {}).expect("build");
        assert_eq!(out.as_json(), None);
    }

    #[test]
    fn json_output_injects_profile_and_api_version() {
        let c = ctx(true);
        let data = serde_json::json!({ "key": "WORK001" });
        let out = CommandOutput::new(&c, data, None, |_| unreachable!()).expect("build");
        let json = out.as_json().expect("json payload");
        assert!(json.contains("\"profile\": \"default\""));
        assert!(json.contains("\"api_version\": 1"));
    }
}
