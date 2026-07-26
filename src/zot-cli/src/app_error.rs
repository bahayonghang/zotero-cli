use anyhow::Error;
use zot_core::{ErrorPayload, ZotError};

pub(crate) struct AppError {
    kind: AppErrorKind,
}

enum AppErrorKind {
    CliParse { detail: String },
    Runtime(Error),
}

impl AppError {
    pub(crate) fn cli_parse(detail: String) -> Self {
        Self {
            kind: AppErrorKind::CliParse { detail },
        }
    }

    pub(crate) fn runtime(error: Error) -> Self {
        Self {
            kind: AppErrorKind::Runtime(error),
        }
    }

    pub(crate) fn payload(&self) -> ErrorPayload {
        match &self.kind {
            AppErrorKind::CliParse { .. } => ErrorPayload {
                code: "cli-parse".to_string(),
                message: "Invalid command-line arguments".to_string(),
                hint: Some("Run `zot --help` to see valid commands and options".to_string()),
            },
            AppErrorKind::Runtime(error) => {
                if let Some(error) = error.downcast_ref::<ZotError>() {
                    return error.payload();
                }
                let code = if error.downcast_ref::<serde_json::Error>().is_some() {
                    "json-serialization"
                } else {
                    "runtime-error"
                };
                ErrorPayload {
                    code: code.to_string(),
                    message: error.to_string(),
                    hint: None,
                }
            }
        }
    }

    pub(crate) fn human_message(&self) -> String {
        match &self.kind {
            AppErrorKind::CliParse { .. } => self.payload().message,
            AppErrorKind::Runtime(error) => error
                .downcast_ref::<ZotError>()
                .map_or_else(|| error.to_string(), ToString::to_string),
        }
    }

    pub(crate) fn verbose_diagnostics(&self) -> Vec<String> {
        match &self.kind {
            AppErrorKind::CliParse { detail } => vec![detail.trim().to_string()],
            AppErrorKind::Runtime(error) => error
                .chain()
                .skip(1)
                .enumerate()
                .map(|(index, cause)| format!("{}: {cause}", index + 1))
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use anyhow::anyhow;

    use super::*;

    #[test]
    fn preserves_embedded_zot_error_payload() {
        let source = ZotError::InvalidInput {
            code: "bad-input".to_string(),
            message: "Bad input".to_string(),
            hint: Some("Fix it".to_string()),
        };
        let error = AppError::runtime(anyhow::Error::new(source).context("command failed"));

        let payload = error.payload();
        assert_eq!(payload.code, "bad-input");
        assert_eq!(payload.message, "Bad input");
        assert_eq!(payload.hint.as_deref(), Some("Fix it"));
        assert_eq!(error.human_message(), "Bad input");
    }

    #[test]
    fn classifies_generic_runtime_and_exposes_chain_only_as_diagnostics() {
        let error = AppError::runtime(anyhow!("socket closed").context("request failed"));

        let payload = error.payload();
        assert_eq!(payload.code, "runtime-error");
        assert_eq!(payload.message, "request failed");
        assert_eq!(error.verbose_diagnostics(), ["1: socket closed"]);
    }

    #[test]
    fn classifies_raw_serde_json_error() {
        let source = serde_json::from_str::<serde_json::Value>("{")
            .expect_err("fixture must be invalid JSON");
        let error = AppError::runtime(source.into());

        assert_eq!(error.payload().code, "json-serialization");
    }

    #[test]
    fn normalizes_cli_parse_error_and_keeps_detail_out_of_payload() {
        let error = AppError::cli_parse("error: unknown argument '--wat'\n".to_string());

        let payload = error.payload();
        assert_eq!(payload.code, "cli-parse");
        assert_eq!(payload.message, "Invalid command-line arguments");
        assert!(!payload.message.contains("--wat"));
        assert_eq!(
            error.verbose_diagnostics(),
            ["error: unknown argument '--wat'"]
        );
    }
}
