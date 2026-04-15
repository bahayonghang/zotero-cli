use std::path::PathBuf;

use serde::Serialize;
use thiserror::Error;

pub type ZotResult<T> = Result<T, ZotError>;

#[derive(Debug, Error)]
pub enum ZotError {
    #[error("{message}")]
    InvalidInput {
        code: String,
        message: String,
        hint: Option<String>,
    },

    #[error("I/O error for {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Configuration parse error in {path}: {detail}")]
    ConfigParse { path: PathBuf, detail: String },

    #[error("Database error: {message}")]
    Database {
        code: String,
        message: String,
        hint: Option<String>,
    },

    #[error("Remote API error: {message}")]
    Remote {
        code: String,
        message: String,
        hint: Option<String>,
        status: Option<u16>,
    },

    #[error("PDF error: {message}")]
    Pdf {
        code: String,
        message: String,
        hint: Option<String>,
    },

    #[error("Feature unavailable: {message}")]
    Unsupported {
        code: String,
        message: String,
        hint: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct ErrorPayload {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

impl ZotError {
    pub fn payload(&self) -> ErrorPayload {
        match self {
            ZotError::InvalidInput {
                code,
                message,
                hint,
            }
            | ZotError::Database {
                code,
                message,
                hint,
            }
            | ZotError::Pdf {
                code,
                message,
                hint,
            }
            | ZotError::Unsupported {
                code,
                message,
                hint,
            } => ErrorPayload {
                code: code.clone(),
                message: message.clone(),
                hint: hint.clone(),
            },
            ZotError::Remote {
                code,
                message,
                hint,
                ..
            } => ErrorPayload {
                code: code.clone(),
                message: message.clone(),
                hint: hint.clone(),
            },
            ZotError::Io { path, source } => ErrorPayload {
                code: "io".to_string(),
                message: format!("I/O error for {}: {source}", path.display()),
                hint: None,
            },
            ZotError::ConfigParse { path, detail } => ErrorPayload {
                code: "config-parse".to_string(),
                message: format!("Configuration parse error in {}: {detail}", path.display()),
                hint: None,
            },
        }
    }
}
