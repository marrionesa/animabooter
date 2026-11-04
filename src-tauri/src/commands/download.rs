//! Download IPC commands: static catalog, streaming download with official
//! hash verification, and cancellation.
//!
//! The busy-flag is shared with `flash` on purpose (one heavy operation at a
//! time keeps the wizard in a single well-defined state), but the cancel
//! token is only armed when a download is actually the active operation, so
//! `cancel_download` can never kill a running flash.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::core::timestamp;
use crate::download::downloader::{self, DownloadEvents, DownloadProgress};
use crate::download::{OsCatalogItem, OsEntry, CATALOG};
use crate::error::AppError;
use crate::state::AppState;

/// True only while `download_iso` owns the busy flag.
static DOWNLOAD_ACTIVE: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, Serialize)]
struct StatusState {
    state: String,
}

#[derive(Debug, Clone, Serialize)]
struct LogLine {
    line: String,
}

/// Bridges downloader events into the webview (`download://…` channels).
struct TauriDownloadSink(AppHandle);

impl DownloadEvents for TauriDownloadSink {
    fn status(&self, state: &str) {
        let _ = self.0.emit(
            "download://status",
            StatusState {
                state: state.to_string(),
            },
        );
    }

    fn progress(&self, p: &DownloadProgress) {
        let _ = self.0.emit("download://progress", p);
    }

    fn log(&self, line: String) {
        let _ = self.0.emit(
            "download://log",
            LogLine {
                line: format!("{} {}", timestamp(), line),
            },
        );
    }
}

/// Static, honest catalog of officially resolvable OS releases.
#[tauri::command]
pub async fn list_os_catalog() -> Result<Vec<OsCatalogItem>, AppError> {
    Ok(CATALOG
        .iter()
        .map(|e| OsCatalogItem {
            id: e.id,
            name: e.name,
            icon: e.icon,
            size_bytes: e.size_bytes,
            website: e.website,
        })
        .collect())
}

/// Download (or cache-verify) the newest official ISO for `os_id`.
/// Progress flows through the `download://…` events; the command resolves
/// when the ISO is cached, verified and ready to flash.
#[tauri::command]
pub async fn download_iso(
    app: AppHandle,
    state: State<'_, AppState>,
    os_id: String,
) -> Result<downloader::DownloadOutcome, AppError> {
    let entry: &OsEntry = OsEntry::by_id(&os_id)
        .ok_or_else(|| AppError::config(format!("unknown OS id: {os_id}")))?;

    // One heavy operation at a time (shared with flash).
    if state.busy.swap(true, Ordering::SeqCst) {
        return Err(AppError::Busy);
    }
    state.cancel.reset();
    DOWNLOAD_ACTIVE.store(true, Ordering::SeqCst);

    let cache_dir = app
        .path()
        .app_cache_dir()
        .unwrap_or_else(|_| std::env::temp_dir().join("animabooter"));
    let cancel = state.cancel.clone();
    let sink: Arc<dyn DownloadEvents> = Arc::new(TauriDownloadSink(app.clone()));

    let result = downloader::run_download(entry, &cache_dir, cancel, sink).await;

    DOWNLOAD_ACTIVE.store(false, Ordering::SeqCst);
    state.busy.store(false, Ordering::SeqCst);
    result
}

/// Cancel the running download; a no-op when nothing is downloading, so it
/// can never cancel a flash that happens to own the busy flag.
#[tauri::command]
pub async fn cancel_download(state: State<'_, AppState>) -> Result<(), AppError> {
    if DOWNLOAD_ACTIVE.load(Ordering::SeqCst) {
        state.cancel.cancel();
    }
    Ok(())
}
