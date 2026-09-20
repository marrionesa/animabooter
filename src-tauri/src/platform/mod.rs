//! Platform backends.
//!
//! Each module implements the same surface:
//! * `list_drives(unsafe_mode)` — enumeration with safety filtering.
//! * `open_target_pair(path)` — one writer handle + one read-back handle.
//! * `check_flash_allowed(drive, unsafe_mode, sink)` — HARD safety re-checks
//!   right before the device is opened.
//! * `eject(path)` — best-effort physical ejection.
//!
//! The Unix writer/reader used by Linux and macOS is shared in `unix_common`.

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub mod unix_common;

#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "linux")]
pub use linux::{check_flash_allowed, eject, list_drives, open_target_pair};
#[cfg(target_os = "macos")]
pub use macos::{check_flash_allowed, eject, list_drives, open_target_pair};
#[cfg(target_os = "windows")]
pub use windows::{check_flash_allowed, eject, list_drives, open_target_pair, restart_as_admin};
