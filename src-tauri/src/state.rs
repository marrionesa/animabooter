//! App state: cancellation token, busy flag, settings persistence and the
//! Tauri bridge for the tauri-free core `EventSink`.

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::core::cancel_token::CancelToken;
use crate::core::{DonePayload, ErrorPayload, EventSink, Phase, ProgressPayload, VerifyPayload};
use crate::error::AppError;

/// Persisted user settings (settings.json inside the OS config dir).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub unsafe_mode: bool,
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default)]
    pub language: Option<String>,
}

fn default_theme() -> String {
    "midnight".to_string()
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            unsafe_mode: false,
            theme: default_theme(),
            language: None,
        }
    }
}

/// Managed application state.
pub struct AppState {
    pub cancel: CancelToken,
    pub busy: AtomicBool,
    pub settings: std::sync::Mutex<Settings>,
    pub config_dir: PathBuf,
}

impl AppState {
    /// Load state from disk; missing/corrupt settings fall back to defaults.
    pub fn load(app: &AppHandle) -> Self {
        let config_dir = app
            .path()
            .app_config_dir()
            .unwrap_or_else(|_| std::env::temp_dir());
        let path = config_dir.join("settings.json");
        let settings = std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        Self {
            cancel: CancelToken::new(),
            busy: AtomicBool::new(false),
            settings: std::sync::Mutex::new(settings),
            config_dir,
        }
    }

    pub fn settings_path(&self) -> PathBuf {
        self.config_dir.join("settings.json")
    }

    pub fn save_settings(&self, settings: &Settings) -> Result<(), AppError> {
        std::fs::create_dir_all(&self.config_dir)
            .map_err(|e| AppError::config(format!("cannot create config directory: {e}")))?;
        let json = serde_json::to_vec_pretty(settings)?;
        std::fs::write(self.settings_path(), json)
            .map_err(|e| AppError::config(format!("cannot write settings.json: {e}")))?;
        Ok(())
    }

    /// Snapshot of the current settings.
    pub fn snapshot_settings(&self) -> Result<Settings, AppError> {
        Ok(self.lock_settings()?.clone())
    }

    pub fn lock_settings(&self) -> Result<std::sync::MutexGuard<'_, Settings>, AppError> {
        self.settings
            .lock()
            .map_err(|_| AppError::config("settings state lock poisoned"))
    }
}

#[derive(Debug, Clone, serde::Serialize)]
struct LogLine {
    line: String,
}

/// `EventSink` implementation that forwards every pipeline event to the
/// webview through Tauri's emit (`flash://…` channels from the IPC contract).
#[derive(Clone)]
pub struct TauriEventSink(pub AppHandle);

impl EventSink for TauriEventSink {
    fn phase(&self, phase: Phase) {
        let _ = self.0.emit("flash://phase", phase);
    }

    fn progress(&self, payload: &ProgressPayload) {
        let _ = self.0.emit("flash://progress", payload);
    }

    fn verify_progress(&self, payload: &VerifyPayload) {
        let _ = self.0.emit("flash://verify", payload);
    }

    fn log(&self, line: String) {
        let _ = self.0.emit(
            "flash://log",
            LogLine {
                line: format!("{} {}", crate::core::timestamp(), line),
            },
        );
    }

    fn error(&self, payload: &ErrorPayload) {
        let _ = self.0.emit("flash://error", payload);
    }

    fn done(&self, payload: &DonePayload) {
        let _ = self.0.emit("flash://done", payload);
    }
}
