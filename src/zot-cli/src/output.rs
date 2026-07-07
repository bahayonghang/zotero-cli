use zot_core::{CliEnvelope, EnvelopeMeta};

use crate::context::AppContext;
use crate::format::{to_pretty_json, EnvelopeMetaSeed, ENVELOPE_API_VERSION};

/// Successful command output. Handlers return this; the dispatch layer calls
/// [`CommandOutput::emit`] to print it. This is the single place where the
/// json-vs-human decision and envelope meta assembly happen.
pub(crate) struct CommandOutput(Payload);

enum Payload {
    /// Fully rendered pretty JSON envelope (constructed only when `ctx.json`).
    Json(String),
    /// Human-readable rendering closure (constructed only when not json;
    /// captures owned data, deferred until `emit`).
    Human(Box<dyn FnOnce() + Send>),
    /// Mode-independent verbatim text (e.g. non-json workspace/item export).
    /// Constructed starting in the workspace-export migration batch.
    #[allow(dead_code)]
    Raw(String),
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

    #[allow(dead_code)]
    pub(crate) fn raw(text: String) -> Self {
        Self(Payload::Raw(text))
    }

    pub(crate) fn silent() -> Self {
        Self(Payload::Silent)
    }

    /// The single print action, invoked once by the dispatch layer.
    pub(crate) fn emit(self) {
        match self.0 {
            Payload::Json(text) | Payload::Raw(text) => println!("{text}"),
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
    use zot_remote::HttpRuntime;

    use super::*;
    use crate::format::print_enveloped;

    fn ctx(json: bool) -> AppContext {
        AppContext {
            json,
            profile: Some("default".to_string()),
            scope: LibraryScope::User,
            config: AppConfig::default(),
            http: Arc::new(HttpRuntime::new().expect("http runtime")),
        }
    }

    /// Capture what `print_enveloped` writes to stdout by rebuilding the exact
    /// same serialization path (it is a thin wrapper over `to_pretty_json`).
    fn expected_enveloped<T: serde::Serialize>(
        c: &AppContext,
        data: &T,
        seed: Option<EnvelopeMetaSeed>,
    ) -> String {
        let seed = seed.unwrap_or_default();
        let meta = zot_core::EnvelopeMeta {
            count: seed.count,
            total: seed.total,
            profile: c.profile.clone(),
            api_version: Some(ENVELOPE_API_VERSION),
        };
        to_pretty_json(&zot_core::CliEnvelope::ok_with_meta(data, meta)).expect("serialize")
    }

    #[test]
    fn json_output_matches_print_enveloped_with_seed() {
        let c = ctx(true);
        let data = serde_json::json!({ "key": "WORK001", "title": "Sample" });
        let seed = Some(EnvelopeMetaSeed {
            count: Some(3),
            total: Some(10),
        });
        let expected = expected_enveloped(&c, &data, seed.clone());
        let out = CommandOutput::new(&c, data, seed, |_| unreachable!()).expect("build");
        assert_eq!(out.as_json(), Some(expected.as_str()));
    }

    #[test]
    fn json_output_matches_print_enveloped_without_seed() {
        let c = ctx(true);
        let data = vec!["a".to_string(), "b".to_string()];
        let expected = expected_enveloped(&c, &data, None);
        let out = CommandOutput::new(&c, data, None, |_| unreachable!()).expect("build");
        assert_eq!(out.as_json(), Some(expected.as_str()));
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

    #[test]
    fn print_enveloped_still_reachable() {
        // Keep a reference so the migration-era helper stays wired until batch 6.
        let c = ctx(true);
        print_enveloped(&c, serde_json::json!({"ok": 1}), None).expect("print");
    }
}
