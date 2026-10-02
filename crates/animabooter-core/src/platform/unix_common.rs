//! Shared file-based device access for Unix platforms (Linux + macOS).
//!
//! Writes go through buffered I/O with an explicit `sync_all()` at the end
//! (O_DIRECT is intentionally OFF for the MVP — the honest flush is what
//! guarantees durability, and page-cache writes are faster on USB sticks
//! with tiny internal caches).

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

use crate::core::writer::{BlockReader, BlockWriter};
use crate::error::AppError;

pub struct UnixBlockWriter(File);
pub struct UnixBlockReader(File);

impl BlockWriter for UnixBlockWriter {
    fn write_at(&mut self, offset: u64, data: &[u8]) -> std::io::Result<()> {
        // `&File` implements Write/Seek so we never need &mut on the field.
        let mut f = &self.0;
        f.seek(SeekFrom::Start(offset))?;
        f.write_all(data)
    }

    fn sync(&mut self) -> std::io::Result<()> {
        self.0.sync_all()
    }
}

impl BlockReader for UnixBlockReader {
    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> std::io::Result<()> {
        let mut f = &self.0;
        f.seek(SeekFrom::Start(offset))?;
        f.read_exact(buf)
    }
}

type OpenedPair = Result<(Box<dyn BlockWriter>, Box<dyn BlockReader>), AppError>;

/// Open one write handle and one separate read-back handle.
/// `sink` receives an audit line about the destructive open.
pub fn open_unix_pair(path: &Path, sink: &dyn crate::core::EventSink) -> OpenedPair {
    sink.log(format!(
        "device: opening {path:?} for exclusive destructive write"
    ));
    let writer = OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .map_err(|e| open_error(e, path))?;
    let reader = File::open(path).map_err(|e| open_error(e, path))?;
    Ok((
        Box::new(UnixBlockWriter(writer)),
        Box::new(UnixBlockReader(reader)),
    ))
}

fn open_error(e: std::io::Error, path: &Path) -> AppError {
    let message = format!("cannot open device {path:?}: {e}");
    match e.kind() {
        std::io::ErrorKind::PermissionDenied => AppError::Permission {
            message,
            hint: Some(
                "Write access to raw devices requires elevated privileges. On Linux run with sudo \
                 or install the udev rule shipped in the README (60-animabooter.rules). On macOS \
                 run with sudo or grant Full Disk Access."
                    .to_string(),
            ),
        },
        std::io::ErrorKind::NotFound => AppError::Device {
            message,
            hint: Some("The drive was unplugged or its device node changed. Refresh the drive list and retry.".to_string()),
        },
        _ => AppError::Device {
            message,
            hint: Some("Make sure the device path exists and no other process holds it.".to_string()),
        },
    }
}
