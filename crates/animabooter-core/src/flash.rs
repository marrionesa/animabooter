//! Shared flash orchestration used by Tauri and the command-line interface.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::core::cancel_token::CancelToken;
use crate::core::pipeline::{run, PipelineJob, PipelineResult, SourceSpec};
use crate::core::EventSink;
use crate::error::AppError;
use crate::image::detect::{detect_file, estimate_uncompressed_size};
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
    let image_size = std::fs::metadata(image)
        .map_err(|error| AppError::image(format!("cannot inspect image {image:?}: {error}")))?
        .len();
    let estimated_payload = estimate_uncompressed_size(image, detection.kind, Some(image_size));
    ensure_capacity(estimated_payload, drive.size_bytes, image, &drive.path)?;

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

fn ensure_capacity(
    payload_size: Option<u64>,
    drive_size: u64,
    image_path: &Path,
    drive_path: &str,
) -> Result<(), AppError> {
    if let Some(payload_size) = payload_size {
        if payload_size > drive_size {
            return Err(AppError::Safety {
                message: format!(
                    "image {image_path:?} ({payload_size} bytes) is larger than {drive_path} ({drive_size} bytes)"
                ),
                hint: Some("Choose a larger target device or a smaller image.".to_string()),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::ensure_capacity;
    use std::path::Path;

    #[test]
    fn rejects_known_payload_larger_than_drive() {
        let result = ensure_capacity(Some(2_048), 1_024, Path::new("image.iso"), "/dev/sdb");
        assert!(result.is_err());
    }

    #[test]
    fn accepts_payload_that_fits() {
        let result = ensure_capacity(Some(1_024), 1_024, Path::new("image.iso"), "/dev/sdb");
        assert!(result.is_ok());
    }

    #[test]
    fn allows_unknown_compressed_size() {
        let result = ensure_capacity(None, 1_024, Path::new("image.xz"), "/dev/sdb");
        assert!(result.is_ok());
    }
}
