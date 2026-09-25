//! macOS backend.
//!
//! * Enumeration via `diskutil list -plist` (parsed with the `plist` crate),
//!   enriched per disk with `diskutil info -plist`
//!   (BSDName, Internal, Removable Media, Device / Media Name, Bus Protocol).
//! * Writes ALWAYS target `/dev/rdiskN` — the raw character device, typically
//!   10–20x faster than the buffered `/dev/diskN` equivalent.
//! * Privileges: v0.1 returns a clear "run with sudo / grant Full Disk
//!   Access" error instead of a fragile osascript wrapper (documented in the
//!   README; an elevation helper is planned for v0.2).

use std::path::{Path, PathBuf};
use std::process::Command;

use plist::Value;

use crate::core::EventSink;
use crate::error::AppError;
use crate::safety::DriveInfo;

pub fn list_drives(unsafe_mode: bool) -> Result<Vec<DriveInfo>, AppError> {
    let output = Command::new("diskutil")
        .args(["list", "-plist"])
        .output()
        .map_err(|e| AppError::platform(format!("cannot run diskutil: {e}")))?;
    if !output.status.success() {
        return Err(AppError::platform(format!(
            "diskutil list failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }

    let value = parse_plist(&output.stdout)?;
    let disks = value
        .as_dictionary()
        .and_then(|d| d.get("AllDisksAndPartitions"))
        .and_then(|v| v.as_array())
        .ok_or_else(|| AppError::platform("unexpected diskutil output shape"))?;

    let mut drives = Vec::new();
    for entry in disks {
        let Some(dict) = entry.as_dictionary() else {
            continue;
        };
        // Whole disks only — entries with partition/volume children are
        // containers for their parts.
        if dict.contains_key("Partitions") || dict.contains_key("APFSVolumes") {
            continue;
        }
        let Some(name) = dict.get("DeviceIdentifier").and_then(Value::as_string) else {
            continue;
        };
        let Some(info) = disk_info(name)? else {
            continue;
        };

        let (internal, removable, model, bus, size) = info;
        if (internal || !removable) && !unsafe_mode {
            continue;
        }

        drives.push(DriveInfo {
            id: name.to_string(),
            path: format!("/dev/{name}"),
            model: if model.is_empty() {
                "Disk".to_string()
            } else {
                model
            },
            vendor: String::new(),
            size_bytes: size,
            removable,
            bus: if bus.is_empty() {
                "unknown".to_string()
            } else {
                bus.to_lowercase()
            },
            serial: String::new(),
            is_system: internal,
        });
    }

    drives.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(drives)
}

fn parse_plist(bytes: &[u8]) -> Result<Value, AppError> {
    Value::from_reader(std::io::Cursor::new(bytes))
        .map_err(|e| AppError::platform(format!("cannot parse diskutil plist: {e}")))
}

/// (internal, removable, model, bus, size_bytes)
type DiskInfo = (bool, bool, String, String, u64);

fn disk_info(name: &str) -> Result<Option<DiskInfo>, AppError> {
    let output = Command::new("diskutil")
        .args(["info", "-plist", name])
        .output()
        .map_err(|e| AppError::platform(format!("cannot run diskutil info: {e}")))?;
    if !output.status.success() {
        return Ok(None);
    }
    let value = parse_plist(&output.stdout)?;
    let Some(dict) = value.as_dictionary() else {
        return Ok(None);
    };

    let boolean = |key: &str| dict.get(key).and_then(Value::as_boolean).unwrap_or(false);
    let string = |key: &str| {
        dict.get(key)
            .and_then(Value::as_string)
            .unwrap_or_default()
            .to_string()
    };

    let internal = boolean("Internal");
    let removable =
        boolean("Removable Media") || boolean("RemovableOrExternal") || boolean("Ejectable");
    let model = string("Device / Media Name");
    let bus = string("Bus Protocol");
    let size = dict.get("Disk Size").and_then(plist_u64).unwrap_or(0);

    Ok(Some((internal, removable, model, bus, size)))
}

fn plist_u64(value: &Value) -> Option<u64> {
    match value {
        Value::Integer(i) => i
            .as_unsigned()
            .or_else(|| i.as_signed().map(|v| v.max(0) as u64)),
        Value::Real(f) => Some(*f as u64),
        _ => None,
    }
}

/// /dev/disk2 -> /dev/rdisk2 (raw device: 10–20x faster).
fn raw_device_path(path: &Path) -> PathBuf {
    let text = path.to_string_lossy();
    match text.strip_prefix("/dev/disk") {
        Some(rest) => PathBuf::from(format!("/dev/rdisk{rest}")),
        None => path.to_path_buf(),
    }
}

pub use crate::platform::unix_common::open_unix_pair;

/// Open the RAW device pair (rdiskN). See `raw_device_path` for why.
pub fn open_target_pair(
    path: &Path,
    sink: &dyn EventSink,
) -> Result<
    (
        Box<dyn crate::core::writer::BlockWriter>,
        Box<dyn crate::core::writer::BlockReader>,
    ),
    AppError,
> {
    let raw = raw_device_path(path);
    // IMPORTANT: we always write to /dev/rdiskN (the raw character device).
    // It is typically 10–20x faster than /dev/diskN; the conversion is the
    // whole point of this wrapper.
    open_unix_pair(&raw, sink)
}

pub fn check_flash_allowed(
    drive: &DriveInfo,
    unsafe_mode: bool,
    sink: &dyn EventSink,
) -> Result<(), AppError> {
    crate::safety::assert_flash_allowed(drive, unsafe_mode)?;

    if drive.is_system {
        if !unsafe_mode {
            return Err(AppError::Safety {
                message: format!("{} is an internal disk", drive.path),
                hint: Some(
                    "Internal disks are hidden by default. Unsafe mode exists for special cases — \
                     enable it only if you fully understand the risk."
                        .to_string(),
                ),
            });
        }
        sink.log(format!(
            "warning: unsafe mode is ON — writing to INTERNAL disk {}",
            drive.path
        ));
    }

    // Try a friendly unmount first so the raw write does not fail with EBUSY.
    let unmount = Command::new("diskutil")
        .args(["unmountDisk", &drive.id])
        .output();
    match unmount {
        Ok(out) if out.status.success() => {
            sink.log(format!("safety: unmounted volumes of {}", drive.id));
        }
        _ => {
            sink.log(format!(
                "safety: pre-unmount failed for {} (the write attempt will report the real error)",
                drive.id
            ));
        }
    }
    Ok(())
}

pub fn eject(path: &Path) -> Result<(), AppError> {
    let name = disk_id_from_path(path)?;
    let eject_once = || Command::new("diskutil").args(["ejectDisk", &name]).output();

    let mut output = eject_once();
    if !output.as_ref().is_ok_and(|o| o.status.success()) {
        // Busy: force-unmount the whole disk and retry once.
        let _ = Command::new("diskutil")
            .args(["unmountDisk", "force", &name])
            .output();
        output = eject_once();
    }

    match output {
        Ok(out) if out.status.success() => Ok(()),
        Ok(out) => Err(AppError::Device {
            message: format!(
                "eject failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ),
            hint: Some(
                "Writes were already flushed — if the pipeline reported success, unplugging the \
                 drive is safe."
                    .to_string(),
            ),
        }),
        Err(e) => Err(AppError::device(format!("cannot run diskutil eject: {e}"))),
    }
}

fn disk_id_from_path(path: &Path) -> Result<String, AppError> {
    let text = path.to_string_lossy();
    text.strip_prefix("/dev/disk")
        .map(str::to_string)
        .ok_or_else(|| AppError::device(format!("not a macOS disk path: {text}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_device_path_inserts_r() {
        assert_eq!(
            raw_device_path(Path::new("/dev/disk2")),
            PathBuf::from("/dev/rdisk2")
        );
        assert_eq!(
            raw_device_path(Path::new("/dev/disk42")),
            PathBuf::from("/dev/rdisk42")
        );
        assert_eq!(
            raw_device_path(Path::new("/dev/something")),
            PathBuf::from("/dev/something")
        );
    }

    #[test]
    fn disk_id_extracts_identifier() {
        assert_eq!(disk_id_from_path(Path::new("/dev/disk3")).unwrap(), "3");
        assert!(disk_id_from_path(Path::new("/dev/sdb")).is_err());
    }
}
