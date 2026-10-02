//! Tauri-free AnimaBooter engine shared by the desktop application and CLI.

pub mod core;
pub mod error;
pub mod flash;
pub mod image;
pub mod platform;
pub mod safety;

pub use core::cancel_token::CancelToken;
pub use core::pipeline::PipelineResult;
pub use core::{DonePayload, ErrorPayload, EventSink, Phase, ProgressPayload, VerifyPayload};
pub use error::AppError;
pub use platform::{eject, list_drives};
pub use safety::DriveInfo;
