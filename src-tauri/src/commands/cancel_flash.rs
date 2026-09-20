//! `cancel_flash` — cooperative cancellation for the running pipeline.

use tauri::State;

use crate::error::AppError;
use crate::state::AppState;

#[tauri::command]
pub async fn cancel_flash(state: State<'_, AppState>) -> Result<(), AppError> {
    state.cancel.cancel();
    Ok(())
}
