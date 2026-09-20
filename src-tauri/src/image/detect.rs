//! Image type detection by magic bytes (first 64 KiB) + honest size
//! estimation for compressed formats.

use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use crate::error::AppError;

/// How many leading bytes we inspect for magic numbers / ISO9660 PVD.
pub const PREFIX_LEN: usize = 64 * 1024;

/// Supported source formats. `Raw` covers .img/.iso/.bin/.dd pass-through.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageKind {
    Raw,
    Gzip,
    Xz,
    Zstd,
    Bzip2,
}

impl ImageKind {
    pub fn compressed(self) -> bool {
        !matches!(self, ImageKind::Raw)
    }
}

/// Detection outcome for a source image.
#[derive(Debug, Clone)]
pub struct Detection {
    pub kind: ImageKind,
    /// Heuristic Windows-ISO flag (drives the pre-confirmation warning).
    pub is_windows_iso: bool,
}

/// Detect the kind from the leading bytes of an image.
pub fn detect_prefix(prefix: &[u8]) -> Detection {
    let starts = |magic: &[u8]| prefix.len() >= magic.len() && &prefix[..magic.len()] == magic;

    let kind = if starts(&[0x1F, 0x8B]) {
        ImageKind::Gzip
    } else if starts(&[0xFD, 0x37, 0x7A, 0x58, 0x5A, 0x00]) {
        ImageKind::Xz
    } else if starts(&[0x28, 0xB5, 0x2F, 0xFD]) {
        ImageKind::Zstd
    } else if prefix.len() >= 4 && prefix.starts_with(b"BZh") && prefix[3].is_ascii_digit() {
        ImageKind::Bzip2
    } else {
        ImageKind::Raw
    };

    // ISO9660: "CD001" lives at offset 0x8001 inside the Primary Volume
    // Descriptor. Windows detection is a best-effort heuristic over the PVD
    // fields: volume identifier (0x8028), system identifier (0x8008) and
    // publisher (0x8384). We only warn — never block.
    let is_windows_iso = kind == ImageKind::Raw
        && prefix.len() >= 0x8404
        && &prefix[0x8001..0x8006] == b"CD001"
        && (ascii_contains(&prefix[0x8028..0x8048], b"WINDOWS")
            || ascii_contains(&prefix[0x8028..0x8048], b"CCCOMA")
            || ascii_contains(&prefix[0x8008..0x8028], b"WIN32")
            || ascii_contains(&prefix[0x8384..0x8404], b"MICROSOFT"));

    Detection {
        kind,
        is_windows_iso,
    }
}

/// Open the file and detect its kind.
pub fn detect_file(path: &Path) -> Result<Detection, AppError> {
    let mut file = std::fs::File::open(path)
        .map_err(|e| AppError::image(format!("cannot open image {path:?}: {e}")))?;
    let mut prefix = vec![0u8; PREFIX_LEN];
    let mut filled = 0usize;
    while filled < prefix.len() {
        let n = file
            .read(&mut prefix[filled..])
            .map_err(|e| AppError::image(format!("cannot read image header: {e}")))?;
        if n == 0 {
            break;
        }
        filled += n;
    }
    prefix.truncate(filled);
    Ok(detect_prefix(&prefix))
}

/// Case-insensitive ASCII substring search (needle must be uppercase).
fn ascii_contains(hay: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() || hay.len() < needle.len() {
        return false;
    }
    hay.windows(needle.len()).any(|w| {
        w.iter()
            .zip(needle)
            .all(|(a, b)| a.to_ascii_uppercase() == *b)
    })
}

/// Best-effort estimate of the uncompressed payload size.
///
/// We only return an estimate when we can do it HONESTLY:
/// * `Raw`: the file size is the payload (exact).
/// * `Gzip`: the trailing ISIZE field holds `size mod 2^32`. If the result
///   is smaller than the compressed size it is almost certainly a >4 GiB
///   wrap-around, so we report unknown instead of a wrong total.
/// * `xz` / `zstd` / `bzip2`: streaming index parsing is not worth the risk
///   in v0.1 — the UI shows live byte counters instead of percentages.
pub fn estimate_uncompressed_size(
    path: &Path,
    kind: ImageKind,
    compressed_size: Option<u64>,
) -> Option<u64> {
    match kind {
        ImageKind::Raw => compressed_size,
        ImageKind::Gzip => {
            let mut file = std::fs::File::open(path).ok()?;
            file.seek(SeekFrom::End(-4)).ok()?;
            let mut tail = [0u8; 4];
            file.read_exact(&mut tail).ok()?;
            let isize_field = u32::from_le_bytes(tail) as u64;
            let compressed = compressed_size.unwrap_or(0);
            if isize_field == u32::MAX as u64 || isize_field < compressed {
                None
            } else {
                Some(isize_field)
            }
        }
        ImageKind::Xz | ImageKind::Zstd | ImageKind::Bzip2 => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn detects_gzip_magic() {
        let bytes = [0x1F, 0x8B, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x03];
        assert_eq!(detect_prefix(&bytes).kind, ImageKind::Gzip);
    }

    #[test]
    fn detects_xz_magic() {
        let bytes = [0xFD, 0x37, 0x7A, 0x58, 0x5A, 0x00, 0x00, 0x04];
        assert_eq!(detect_prefix(&bytes).kind, ImageKind::Xz);
    }

    #[test]
    fn detects_zstd_magic() {
        let bytes = [0x28, 0xB5, 0x2F, 0xFD, 0x24, 0x00];
        assert_eq!(detect_prefix(&bytes).kind, ImageKind::Zstd);
    }

    #[test]
    fn detects_bzip2_magic() {
        let bytes = *b"BZh9";
        assert_eq!(detect_prefix(&bytes).kind, ImageKind::Bzip2);
    }

    #[test]
    fn detects_raw_and_short_buffers() {
        assert_eq!(detect_prefix(b"\0\0\0\0").kind, ImageKind::Raw);
        assert_eq!(detect_prefix(&[]).kind, ImageKind::Raw);
        assert_eq!(detect_prefix(&[0x1F]).kind, ImageKind::Raw);
    }

    #[test]
    fn detects_iso9660_and_windows_flag() {
        // Build a buffer with a PVD: CD001 at 0x8001, volume id at 0x8028.
        let mut buf = vec![0u8; 0x8404];
        buf[0x8001..0x8006].copy_from_slice(b"CD001");
        let vol = b"LINUX_DVD";
        buf[0x8028..0x8028 + vol.len()].copy_from_slice(vol);
        let det = detect_prefix(&buf);
        assert_eq!(det.kind, ImageKind::Raw);
        assert!(!det.is_windows_iso);

        // A Windows-style volume id flips the warning flag.
        let win = b"CCCOMA_X64FRE_EN-US_DV9";
        buf[0x8028..0x8028 + win.len()].copy_from_slice(win);
        assert!(detect_prefix(&buf).is_windows_iso);

        // Publisher-based detection ("MICROSOFT" at 0x8384).
        let mut buf2 = vec![0u8; 0x8404];
        buf2[0x8001..0x8006].copy_from_slice(b"CD001");
        let pub_field = b"MICROSOFT CORPORATION";
        buf2[0x8384..0x8384 + pub_field.len()].copy_from_slice(pub_field);
        assert!(detect_prefix(&buf2).is_windows_iso);
    }

    #[test]
    fn ascii_contains_is_case_insensitive() {
        assert!(ascii_contains(b"microsoft corporation", b"MICROSOFT"));
        assert!(!ascii_contains(b"gnu/linux", b"WINDOWS"));
        assert!(!ascii_contains(b"short", b"longer-needle"));
    }

    #[test]
    fn gzip_estimate_rejects_wrap_around() {
        // Real file on disk: build a tiny gzip stream, then patch ISIZE.
        let dir = std::env::temp_dir();
        let path = dir.join(format!("animabooter-test-{}.gz", std::process::id()));
        {
            let mut enc = flate2::write::GzEncoder::new(
                std::fs::File::create(&path).unwrap(),
                flate2::Compression::default(),
            );
            enc.write_all(&vec![0u8; 1024]).unwrap();
        }
        let compressed = std::fs::metadata(&path).unwrap().len();
        // Honest case: 1024 uncompressed bytes > 0… well, ISIZE=1024 >= compressed? yes.
        assert_eq!(
            estimate_uncompressed_size(&path, ImageKind::Gzip, Some(compressed)),
            Some(1024)
        );
        // Wrap-around suspicion: claim the compressed file is HUGE (bigger than ISIZE).
        assert_eq!(
            estimate_uncompressed_size(&path, ImageKind::Gzip, Some(compressed * 1000)),
            None
        );
        let _ = std::fs::remove_file(&path);
    }
}
