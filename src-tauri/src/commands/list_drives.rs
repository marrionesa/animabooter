//! `list_drives` — enumeration with the safety filter applied server-side.

use tauri::State;

use crate::error::AppError;
use crate::safety::DriveInfo;
use crate::state::AppState;

#[tauri::command]
pub async fn list_drives(state: State<'_, AppState>) -> Result<Vec<DriveInfo>, AppError> {
    let unsafe_mode = state.lock_settings()?.unsafe_mode;
    // Enumeration talks to the OS (udev/diskutil/Win32): keep it off the
    // async executor.
    let drives = tokio::task::spawn_blocking(move || crate::platform::list_drives(unsafe_mode))
        .await
        .map_err(|e| AppError::platform(format!("enumeration task failed: {e}")))??;
    Ok(drives)
}
