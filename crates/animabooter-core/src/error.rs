//! Application error type.
//!
//! Every error that crosses the IPC boundary is serialized as
//! `{ message, hint }` so the frontend can always render an actionable hint
//! next to the failure.

use serde::{Serialize, Serializer};

/// Errors surfaced to the UI. `hint` is always user-facing and actionable.
#[derive(Debug, Clone, thiserror::Error)]
pub enum AppError {
    #[error("{message}")]
    Io {
        message: String,
        hint: Option<String>,
    },

    #[error("{message}")]
    Device {
        message: String,
        hint: Option<String>,
    },

    #[error("{message}")]
    Permission {
        message: String,
        hint: Option<String>,
    },

    #[error("{message}")]
    Safety {
        message: String,
        hint: Option<String>,
    },

    #[error("operation cancelled by the user")]
    Cancelled,

    #[error("verification failed: the read-back data does not match the source image")]
    VerifyMismatch { expected: String, actual: String },

    #[error("{message}")]
    Image {
        message: String,
        hint: Option<String>,
    },

    #[error("{message}")]
    Compression {
        message: String,
        hint: Option<String>,
    },

    #[error("{message}")]
    Config {
        message: String,
        hint: Option<String>,
    },

    #[error("another flash operation is already running")]
    Busy,

    #[error("{message}")]
    Platform {
        message: String,
        hint: Option<String>,
    },
}

impl AppError {
    /// Actionable, user-facing hint for this error (if any).
    pub fn hint(&self) -> Option<String> {
        match self {
            AppError::Io { hint, .. }
            | AppError::Device { hint, .. }
            | AppError::Permission { hint, .. }
            | AppError::Safety { hint, .. }
            | AppError::Image { hint, .. }
            | AppError::Compression { hint, .. }
            | AppError::Config { hint, .. }
            | AppError::Platform { hint, .. } => hint.clone(),
            AppError::Cancelled => Some(crate::safety::HINT_UNKNOWN_STATE.to_string()),
            AppError::VerifyMismatch { .. } => Some(crate::safety::HINT_UNKNOWN_STATE.to_string()),
            AppError::Busy => {
                Some("Wait for the current operation to finish or cancel it.".to_string())
            }
        }
    }

    pub fn image(message: impl Into<String>) -> Self {
        AppError::Image {
            message: message.into(),
            hint: None,
        }
    }

    pub fn device(message: impl Into<String>) -> Self {
        AppError::Device {
            message: message.into(),
            hint: None,
        }
    }

    pub fn platform(message: impl Into<String>) -> Self {
        AppError::Platform {
            message: message.into(),
            hint: None,
        }
    }

    pub fn config(message: impl Into<String>) -> Self {
        AppError::Config {
            message: message.into(),
            hint: None,
        }
    }
}

#[derive(Serialize)]
struct SerializedAppError {
    message: String,
    hint: Option<String>,
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        SerializedAppError {
            message: self.to_string(),
            hint: self.hint(),
        }
        .serialize(serializer)
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        let message = e.to_string();
        let hint = match e.kind() {
            std::io::ErrorKind::PermissionDenied => Some(
                "Permission denied. On Linux run with sudo or install the udev rule from the README. \
                 On macOS run with sudo or grant Full Disk Access to the app."
                    .to_string(),
            ),
            _ => None,
        };
        AppError::Io { message, hint }
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        AppError::config(e.to_string())
    }
}
