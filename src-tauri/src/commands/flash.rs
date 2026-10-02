//! Tauri adapter for the shared AnimaBooter flash engine.

use std::sync::atomic::Ordering;
use std::sync::Arc;

use tauri::State;

use animabooter_core::{AppError, EventSink, PipelineResult};

use crate::state::{AppState, TauriEventSink};

#[tauri::command]
pub async fn flash(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    image_path: String,
    drive_path: String,
    verify: Option<bool>,
) -> Result<PipelineResult, AppError> {
    let verify = verify.unwrap_or(true);

    if state.busy.swap(true, Ordering::SeqCst) {
        return Err(AppError::Busy);
    }
    state.cancel.reset();

    let sink: Arc<dyn EventSink> = Arc::new(TauriEventSink(app));
    let unsafe_mode = state.lock_settings()?.unsafe_mode;
    let cancel = state.cancel.clone();

    let result = tokio::task::spawn_blocking(move || {
        animabooter_core::flash::run_flash(
            &image_path,
            &drive_path,
            verify,
            unsafe_mode,
            cancel,
            sink,
        )
    })
    .await
    .unwrap_or_else(|e| Err(AppError::platform(format!("flash task crashed: {e}"))));

    state.busy.store(false, Ordering::SeqCst);
    result
}
