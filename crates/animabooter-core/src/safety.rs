//! Destructive-safety rules and the shared `DriveInfo` domain type.
//!
//! Safety here is a *feature*, not an afterthought:
//! * Only removable drives are listed by default; internal disks appear only
//!   with `unsafe_mode` enabled (double confirmation in the UI).
//! * `flash` re-verifies the target BEFORE opening the device (platform hard
//!   checks live in `platform::check_flash_allowed`).
//! * Every destructive operation announces its intent in the live log first.

use serde::{Deserialize, Serialize};

use crate::core::EventSink;
use crate::error::AppError;

/// Shared drive descriptor across platforms.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriveInfo {
    pub id: String,
    pub path: String,
    pub model: String,
    pub vendor: String,
    pub size_bytes: u64,
    pub removable: bool,
    pub bus: String,
    pub serial: String,
    pub is_system: bool,
}

/// Hint shown whenever the device may be left in an inconsistent state.
pub const HINT_UNKNOWN_STATE: &str = "Device is in an unknown state. Re-flash before using.";

/// Shared policy gate. Platform-specific hard checks (root filesystem on
/// Linux, internal flag on macOS, %SystemRoot% volume on Windows) live in
/// each platform module and run in addition to this.
pub fn assert_flash_allowed(drive: &DriveInfo, unsafe_mode: bool) -> Result<(), AppError> {
    if !drive.removable && !unsafe_mode {
        return Err(AppError::Safety {
            message: format!("refusing to touch non-removable drive {}", drive.path),
            hint: Some(
                "Internal disks are only visible with unsafe mode enabled. \
                 Enable it in the header if you really know what you are doing."
                    .to_string(),
            ),
        });
    }
    Ok(())
}

/// Announce the destructive intent BEFORE any destructive operation runs.
/// Every destructive path must call this first — it is the audit trail.
pub fn announce_intent(sink: &dyn EventSink, drive: &DriveInfo, image_name: &str) {
    sink.log(format!(
        "safety: about to write '{}' to {} ({} {})",
        image_name,
        drive.path,
        drive.vendor.trim(),
        drive.model.trim()
    ));
    sink.log(format!(
        "safety: ALL DATA on {} will be destroyed",
        drive.path
    ));
}
