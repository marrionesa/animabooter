//! Command layer: thin, typed IPC endpoints. Heavy work happens in `core`
//! and `platform`; commands own the busy-flag, cancellation and safety flow.

pub mod cancel_flash;
pub mod eject;
pub mod flash;
pub mod list_drives;

use std::path::Path;

use serde::Serialize;
use tauri::State;

use crate::error::AppError;
use crate::image::detect::{detect_file, estimate_uncompressed_size};
use crate::state::{AppState, Settings};

// Small utility commands live here to keep the file tree exactly as
// specified by the project layout.

#[derive(Debug, Serialize)]
pub struct DetectionInfo {
    /// "raw" | "gzip" | "xz" | "zstd" | "bzip2"
    pub kind: String,
    pub compressed: bool,
    pub is_windows_iso: bool,
    pub file_size: u64,
    /// Honest estimate of the decompressed size (null when unknown).
    pub estimated_size: Option<u64>,
}

/// Detect image type/size BEFORE the user confirms anything (drives the
/// file chip badges and the Windows-ISO warning).
#[tauri::command]
pub async fn detect_image(image_path: String) -> Result<DetectionInfo, AppError> {
    let path = Path::new(&image_path);
    let detection = detect_file(path)?;
    let file_size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    let estimated_size = estimate_uncompressed_size(path, detection.kind, Some(file_size));
    Ok(DetectionInfo {
        kind: format!("{:?}", detection.kind).to_lowercase(),
        compressed: detection.kind.compressed(),
        is_windows_iso: detection.is_windows_iso,
        file_size,
        estimated_size,
    })
}

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<Settings, AppError> {
    state.snapshot_settings()
}

#[tauri::command]
pub async fn set_settings(state: State<'_, AppState>, settings: Settings) -> Result<(), AppError> {
    if !matches!(
        settings.theme.as_str(),
        "midnight" | "catppuccin-mocha" | "tokyo-night"
    ) {
        return Err(AppError::config(format!(
            "unknown theme: {}",
            settings.theme
        )));
    }
    *state.lock_settings()? = settings.clone();
    state.save_settings(&settings)
}

/// Windows only: relaunch the app elevated via PowerShell `runas`.
#[tauri::command]
pub async fn restart_as_admin() -> Result<(), AppError> {
    #[cfg(target_os = "windows")]
    let result = crate::platform::windows::restart_as_admin();
    #[cfg(not(target_os = "windows"))]
    let result: Result<(), AppError> = Err(AppError::platform(
        "elevated restart is only needed on Windows",
    ));
    result
}
