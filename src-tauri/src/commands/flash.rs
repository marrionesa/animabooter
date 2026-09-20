//! `flash` — the orchestrating command.
//!
//! Flow: busy-flag → refresh drive info → detect image → HARD safety
//! re-check → announce intent → open device pair → run the 3-stage parallel
//! pipeline → return the honest measured result.

use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::Arc;

use tauri::State;

use crate::core::pipeline::{run, PipelineJob, PipelineResult, SourceSpec};
use crate::error::AppError;
use crate::image::detect::detect_file;
use crate::platform;
use crate::safety;
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

    // One flash at a time.
    if state.busy.swap(true, Ordering::SeqCst) {
        return Err(AppError::Busy);
    }
    state.cancel.reset();

    let sink: Arc<dyn crate::core::EventSink> = Arc::new(TauriEventSink(app.clone()));
    let unsafe_mode = state.lock_settings()?.unsafe_mode;
    let cancel = state.cancel.clone();

    let result = tokio::task::spawn_blocking(move || {
        run_flash_job(
            &image_path,
            &drive_path,
            verify,
            unsafe_mode,
            cancel,
            Arc::clone(&sink),
        )
    })
    .await
    .unwrap_or_else(|e| {
        Err(AppError::platform(format!("flash task crashed: {e}")))
    });

    state.busy.store(false, Ordering::SeqCst);
    result
}

fn run_flash_job(
    image_path: &str,
    drive_path: &str,
    verify: bool,
    unsafe_mode: bool,
    cancel: crate::core::cancel_token::CancelToken,
    sink: Arc<dyn crate::core::EventSink>,
) -> Result<PipelineResult, AppError> {
    // 1. Refresh drive identity (it may have changed since the last scan).
    let drives = platform::list_drives(unsafe_mode)?;
    let drive = drives
        .into_iter()
        .find(|d| d.path.eq_ignore_ascii_case(drive_path))
        .ok_or_else(|| {
            AppError::Device {
                message: format!("drive {drive_path} is no longer available"),
                hint: Some(
                    "The drive may have been unplugged. Refresh the list and select it again."
                        .to_string(),
                ),
            }
        })?;

    // 2. Detect the image (kind + honest size estimate + Windows-ISO flag).
    let image = Path::new(image_path);
    let detection = detect_file(image)?;
    let image_name = image
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| image_path.to_string());
    if detection.is_windows_iso {
        sink.log(
            "warning: Windows ISO detected — install.wim larger than 4 GiB may not boot via dd. \
             Continue at your own risk."
                .to_string(),
        );
    }

    // 3. HARD safety re-check BEFORE opening the device (per platform).
    safety::announce_intent(sink.as_ref(), &drive, &image_name);
    platform::check_flash_allowed(&drive, unsafe_mode, sink.as_ref())?;

    // 4. Open one write handle + one read-back handle.
    let (writer, reader) = platform::open_target_pair(Path::new(&drive.path), sink.as_ref())?;

    // 5. Run the parallel pipeline.
    let job = PipelineJob {
        source: SourceSpec::File(PathBuf::from(image_path)),
        writer,
        reader,
        verify,
        cancel,
        drive_label: drive.path.clone(),
    };
    run(job, sink)
}
