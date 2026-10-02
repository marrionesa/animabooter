//! Shared flash orchestration used by Tauri and the command-line interface.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::core::cancel_token::CancelToken;
use crate::core::pipeline::{run, PipelineJob, PipelineResult, SourceSpec};
use crate::core::EventSink;
use crate::error::AppError;
use crate::image::detect::detect_file;
use crate::platform;
use crate::safety;

/// Flash an image to a validated device using the shared pipeline.
pub fn run_flash(
    image_path: &str,
    drive_path: &str,
    verify: bool,
    unsafe_mode: bool,
    cancel: CancelToken,
    sink: Arc<dyn EventSink>,
) -> Result<PipelineResult, AppError> {
    let drives = platform::list_drives(unsafe_mode)?;
    let drive = drives
        .into_iter()
        .find(|drive| drive.path.eq_ignore_ascii_case(drive_path))
        .ok_or_else(|| AppError::Device {
            message: format!("drive {drive_path} is no longer available"),
            hint: Some(
                "The drive may have been unplugged. Refresh the list and select it again."
                    .to_string(),
            ),
        })?;

    let image = Path::new(image_path);
    let detection = detect_file(image)?;
    let image_name = image
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| image_path.to_string());
    if detection.is_windows_iso {
        sink.log(
            "warning: Windows ISO detected — install.wim larger than 4 GiB may not boot via dd. \
             Continue at your own risk."
                .to_string(),
        );
    }

    safety::announce_intent(sink.as_ref(), &drive, &image_name);
    platform::check_flash_allowed(&drive, unsafe_mode, sink.as_ref())?;

    let (writer, reader) = platform::open_target_pair(Path::new(&drive.path), sink.as_ref())?;
    let job = PipelineJob {
        source: SourceSpec::File(PathBuf::from(image_path)),
        writer,
        reader,
        verify,
        cancel,
        drive_label: drive.path,
    };
    run(job, sink)
}
