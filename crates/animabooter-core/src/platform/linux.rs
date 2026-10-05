//! Linux backend.
//!
//! * Enumeration via the `udev` crate (subsystem `block`, DEVTYPE=disk).
//! * Removability from sysfs (`removable`) or udev properties
//!   (`ID_DRIVE_FLASH_SD`, `ID_USB_DRIVER`).
//! * Mounted partitions parsed from `/proc/mounts`; we try a lazy unmount
//!   through `udisksctl` and reject with a clear error otherwise (the UI
//!   offers a retry).
//! * Writes go to `/dev/sdX` (or nvme/mmcblk) with `sync_all` before success.

use std::path::Path;
use std::process::Command;

use crate::core::EventSink;
use crate::error::AppError;
use crate::safety::DriveInfo;

pub fn list_drives(unsafe_mode: bool) -> Result<Vec<DriveInfo>, AppError> {
    // udev 0.9 owns its internal context; `Enumerator::new()` builds it for us.
    let mut enumerator = udev::Enumerator::new()
        .map_err(|e| AppError::platform(format!("udev enumerator failed: {e}")))?;
    enumerator
        .match_subsystem("block")
        .map_err(|e| AppError::platform(format!("udev subsystem match failed: {e}")))?;

    let mounts = parse_mounts()?;
    let mut drives = Vec::new();

    for device in enumerator
        .scan_devices()
        .map_err(|e| AppError::platform(format!("udev scan failed: {e}")))?
    {
        // Whole disks only (DEVTYPE=disk), never partitions or loop devices.
        if device.devtype().map(|dt| dt != "disk").unwrap_or(true) {
            continue;
        }
        let Some(node) = device
            .devnode()
            .and_then(|p| p.to_str())
            .map(str::to_string)
        else {
            continue;
        };
        if !is_disk_node(&node) {
            continue;
        }

        let removable_attr = attr(&device, "removable").unwrap_or_default();
        let has_usb_driver = device.property_value("ID_USB_DRIVER").is_some();
        let is_flash_sd = device.property_value("ID_DRIVE_FLASH_SD").is_some();
        let removable = removable_attr == "1" || has_usb_driver || is_flash_sd;

        let bus = if is_flash_sd {
            "sd"
        } else if has_usb_driver {
            "usb"
        } else {
            device
                .property_value("ID_BUS")
                .and_then(|v| v.to_str())
                .unwrap_or("unknown")
        }
        .to_string();

        // Safety filter: internal disks are invisible unless unsafe_mode.
        if !removable && !unsafe_mode {
            continue;
        }

        let size_bytes = attr(&device, "size")
            .and_then(|s| s.trim().parse::<u64>().ok())
            .map(|sectors| sectors * 512) // sysfs reports 512-byte sectors
            .unwrap_or(0);

        let (vendor, model) = read_vendor_model(&device);
        let serial = device
            .property_value("ID_SERIAL_SHORT")
            .or_else(|| device.property_value("ID_SERIAL"))
            .and_then(|v| v.to_str())
            .unwrap_or("")
            .to_string();

        let is_system = mounts
            .iter()
            .any(|(source, mount_point)| mount_point == "/" && is_partition_of(&node, source));

        drives.push(DriveInfo {
            id: node.trim_start_matches("/dev/").to_string(),
            path: node.clone(),
            model,
            vendor,
            size_bytes,
            removable,
            bus,
            serial,
            is_system,
        });
    }

    drives.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(drives)
}

/// Whole-disk node names we accept: /dev/sdX, /dev/nvmeXnY, /dev/mmcblkX.
/// Everything else (loop, ram, zram, dm-*, sr0) is ignored.
fn is_disk_node(node: &str) -> bool {
    if let Some(rest) = node.strip_prefix("/dev/sd") {
        return !rest.is_empty() && rest.chars().all(|c| c.is_ascii_alphabetic());
    }
    if let Some(rest) = node.strip_prefix("/dev/nvme") {
        // nvme0n1 (the controller nvme0 is a chardev and does not appear here)
        return rest.contains('n');
    }
    if let Some(rest) = node.strip_prefix("/dev/mmcblk") {
        return !rest.is_empty();
    }
    false
}

fn attr(device: &udev::Device, name: &str) -> Option<String> {
    device
        .attribute_value(name)
        .and_then(|v| v.to_str())
        .map(|s| s.trim().to_string())
}

/// vendor/model live under `device/` for sd* disks (sysfs symlink), with a
/// parent fallback for nvme/mmcblk.
fn read_vendor_model(device: &udev::Device) -> (String, String) {
    let vendor = attr(device, "device/vendor")
        .or_else(|| device.parent().as_ref().and_then(|p| attr(p, "vendor")))
        .unwrap_or_default();
    let model = attr(device, "device/model")
        .or_else(|| device.parent().as_ref().and_then(|p| attr(p, "model")))
        .filter(|m| !m.is_empty())
        .unwrap_or_else(|| "Disk".to_string());
    (vendor, model)
}

/// Read /proc/mounts into (source, mount_point) pairs.
pub fn parse_mounts() -> Result<Vec<(String, String)>, AppError> {
    let data = std::fs::read_to_string("/proc/mounts")
        .map_err(|e| AppError::platform(format!("cannot read /proc/mounts: {e}")))?;
    Ok(data
        .lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let source = decode_mount_field(fields.next()?);
            let mount_point = decode_mount_field(fields.next()?);
            Some((source, mount_point))
        })
        .collect())
}

fn decode_mount_field(field: &str) -> String {
    field
        .replace("\\040", " ")
        .replace("\\011", "\t")
        .replace("\\134", "\\")
}

/// True when `source` is a partition OF `disk` (not the disk itself).
/// Handles sdXN, nvmeXnYpZ and mmcblkXpY naming.
pub fn is_partition_of(disk: &str, source: &str) -> bool {
    if source == disk {
        return false;
    }
    let Some(rest) = source.strip_prefix(disk) else {
        return false;
    };
    let rest = rest.strip_prefix('p').unwrap_or(rest);
    !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit())
}

pub use crate::platform::unix_common::open_unix_pair as open_target_pair;

/// Detects whether the source resolves to the selected disk or one of its
/// partitions before any target handle is opened.
pub fn source_matches_drive(image: &Path, drive: &DriveInfo) -> Result<bool, AppError> {
    use std::os::unix::fs::MetadataExt;

    let image = std::fs::canonicalize(image)
        .map_err(|e| AppError::image(format!("cannot resolve image {image:?}: {e}")))?;
    let target_path = Path::new(&drive.path);
    let target = std::fs::canonicalize(target_path)
        .map_err(|e| AppError::platform(format!("cannot resolve target {target_path:?}: {e}")))?;

    let image_dev = std::fs::metadata(&image)
        .map(|metadata| metadata.dev())
        .unwrap_or(0);
    let target_dev = std::fs::metadata(&target)
        .map(|metadata| metadata.dev())
        .unwrap_or(0);
    if image_dev == target_dev {
        return Ok(true);
    }

    let source_text = image.to_string_lossy();
    let target_text = target.to_string_lossy();
    if source_text == target_text
        || is_partition_of(&target_text, &source_text)
        || is_partition_of(&source_text, &target_text)
    {
        return Ok(true);
    }

    let mount_source = parse_mounts()?
        .into_iter()
        .filter_map(|(source, mount_point)| {
            let mount_point = Path::new(&mount_point);
            let mount_point = std::fs::canonicalize(mount_point).ok()?;
            if image.starts_with(&mount_point) {
                Some(source)
            } else {
                None
            }
        })
        .max_by_key(|source| source.len());

    if let Some(mount_source) = mount_source {
        if mount_source == drive.path
            || is_partition_of(&drive.path, &mount_source)
            || is_partition_of(&mount_source, &drive.path)
        {
            return Ok(true);
        }
    }
    Ok(false)
}

/// HARD safety re-check right before opening the device:
/// * root filesystem on the target => unconditional rejection;
/// * other mounted partitions => attempt `udisksctl unmount`, else reject
///   with the mount list (the frontend offers retry).
pub fn check_flash_allowed(
    drive: &DriveInfo,
    unsafe_mode: bool,
    sink: &dyn EventSink,
) -> Result<(), AppError> {
    crate::safety::assert_flash_allowed(drive, unsafe_mode)?;

    let mounts = parse_mounts()?;
    let mounted: Vec<(String, String)> = mounts
        .into_iter()
        .filter(|(source, _)| is_partition_of(&drive.path, source) || *source == drive.path)
        .collect();

    if mounted.iter().any(|(_, mount_point)| mount_point == "/") {
        return Err(AppError::Safety {
            message: format!(
                "{} contains the root filesystem (/) — flashing it would destroy the running system",
                drive.path
            ),
            hint: Some("This is a hard safety rule that cannot be overridden.".to_string()),
        });
    }

    if !mounted.is_empty() {
        let list: Vec<String> = mounted.iter().map(|(s, m)| format!("{s} → {m}")).collect();
        sink.log(format!(
            "safety: {} has mounted partitions, trying lazy unmount: {}",
            drive.path,
            list.join(", ")
        ));
        for (source, _) in &mounted {
            let output = Command::new("udisksctl")
                .args(["unmount", "-b", source])
                .output();
            match output {
                Ok(out) if out.status.success() => {
                    sink.log(format!("safety: unmounted {source}"));
                }
                _ => {
                    return Err(AppError::Safety {
                        message: format!(
                            "partitions of {} are mounted and could not be unmounted: {}",
                            drive.path,
                            list.join(", ")
                        ),
                        hint: Some(
                            "Close file managers using the drive (or unmount manually), then retry."
                                .to_string(),
                        ),
                    });
                }
            }
        }
    } else {
        sink.log(format!("safety: {} has no mounted partitions", drive.path));
    }
    Ok(())
}

/// Best-effort physical power-off through udisksctl.
pub fn eject(path: &Path) -> Result<(), AppError> {
    let target = path.to_string_lossy().to_string();
    let output = Command::new("udisksctl")
        .args(["power-off", "-b", &target])
        .output();
    match output {
        Ok(out) if out.status.success() => Ok(()),
        Ok(out) => Err(AppError::Device {
            message: format!(
                "eject failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ),
            hint: Some(
                "Writes were already flushed — if the pipeline reported success, unplugging the \
                 drive is safe. Otherwise re-flash before using it."
                    .to_string(),
            ),
        }),
        Err(e) => Err(AppError::Device {
            message: format!("udisksctl is not available: {e}"),
            hint: Some(
                "Writes were already flushed — if the pipeline reported success, unplugging the \
                 drive is safe. Install udisks2 for one-click ejection."
                    .to_string(),
            ),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::is_partition_of;

    #[test]
    fn sd_partitions() {
        assert!(is_partition_of("/dev/sdb", "/dev/sdb1"));
        assert!(is_partition_of("/dev/sdb", "/dev/sdb12"));
        assert!(!is_partition_of("/dev/sdb", "/dev/sdb"));
        assert!(!is_partition_of("/dev/sdb", "/dev/sda1"));
        // sdaa is a DIFFERENT disk, not a partition of sda.
        assert!(!is_partition_of("/dev/sda", "/dev/sdaa"));
    }

    #[test]
    fn nvme_and_mmc_partitions() {
        assert!(is_partition_of("/dev/nvme0n1", "/dev/nvme0n1p2"));
        assert!(!is_partition_of("/dev/nvme0n1", "/dev/nvme0n1"));
        assert!(is_partition_of("/dev/mmcblk0", "/dev/mmcblk0p1"));
        assert!(!is_partition_of("/dev/nvme1n1", "/dev/nvme0n1p1"));
    }
}
