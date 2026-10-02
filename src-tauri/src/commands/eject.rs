//! `eject` — best-effort physical ejection after a successful flash.

use std::path::Path;
use std::sync::Arc;

use tauri::State;

use crate::state::{AppState, TauriEventSink};
use animabooter_core::core::EventSink;
use animabooter_core::error::AppError;

#[tauri::command]
pub async fn eject(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    drive_path: String,
) -> Result<(), AppError> {
    let _ = state; // reserved for future "recently flashed" bookkeeping
    let sink = Arc::new(TauriEventSink(app));
    let sink_for_log = Arc::clone(&sink);
    sink_for_log.log(format!("eject: requesting safe removal of {drive_path}"));
    tokio::task::spawn_blocking(move || platform_eject(&drive_path))
        .await
        .map_err(|e| AppError::platform(format!("eject task failed: {e}")))?
}

fn platform_eject(drive_path: &str) -> Result<(), AppError> {
    animabooter_core::platform::eject(Path::new(drive_path))
}
