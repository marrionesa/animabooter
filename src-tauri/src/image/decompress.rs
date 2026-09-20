//! Streaming decompression wrappers. Nothing here ever buffers the whole
//! image — every decoder is a pull-based `Read` adapter.

use std::io::Read;

use crate::error::AppError;
use crate::image::detect::ImageKind;

/// Wrap a raw reader into the decompressor matching `kind`.
/// `Raw` is pass-through.
pub fn wrap(
    reader: Box<dyn Read + Send>,
    kind: ImageKind,
) -> Result<Box<dyn Read + Send>, AppError> {
    match kind {
        ImageKind::Raw => Ok(reader),
        ImageKind::Gzip => {
            // MultiGzDecoder handles concatenated gzip members (common for
            // images compressed with split streams).
            Ok(Box::new(flate2::read::MultiGzDecoder::new(reader)))
        }
        ImageKind::Xz => Ok(Box::new(xz2::read::XzDecoder::new_multi_decoder(reader))),
        ImageKind::Zstd => zstd::stream::read::Decoder::new(reader)
            .map(|d| Box::new(d) as Box<dyn Read + Send>)
            .map_err(|e| AppError::Compression {
                message: format!("zstd init failed: {e}"),
                hint: Some("The file may be corrupted or not a real zstd stream.".to_string()),
            }),
        ImageKind::Bzip2 => Ok(Box::new(bzip2::read::MultiBzDecoder::new(reader))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn gzip_roundtrip_through_wrapper() {
        let original: Vec<u8> = (0..64_000u32).map(|i| (i % 97) as u8).collect();
        let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        std::io::Write::write_all(&mut enc, &original).unwrap();
        let compressed = enc.finish().unwrap();

        let wrapped = wrap(Box::new(std::io::Cursor::new(compressed)), ImageKind::Gzip).unwrap();
        let mut out = Vec::new();
        let mut wrapped = wrapped;
        wrapped.read_to_end(&mut out).unwrap();
        assert_eq!(out, original);
    }

    #[test]
    fn raw_is_passthrough() {
        let data = b"hello anima".to_vec();
        let wrapped = wrap(Box::new(std::io::Cursor::new(data.clone())), ImageKind::Raw).unwrap();
        let mut out = Vec::new();
        let mut wrapped = wrapped;
        wrapped.read_to_end(&mut out).unwrap();
        assert_eq!(out, data);
    }

    #[test]
    fn zstd_roundtrip_through_wrapper() {
        let original: Vec<u8> = (0..64_000u32).map(|i| (i % 53) as u8).collect();
        let compressed = zstd::stream::encode_all(&original[..], 3).unwrap();

        let wrapped = wrap(Box::new(std::io::Cursor::new(compressed)), ImageKind::Zstd).unwrap();
        let mut out = Vec::new();
        let mut wrapped = wrapped;
        wrapped.read_to_end(&mut out).unwrap();
        assert_eq!(out, original);
    }

    #[test]
    fn xz_roundtrip_through_wrapper() {
        let original: Vec<u8> = (0..64_000u32).map(|i| (i % 31) as u8).collect();
        let mut enc = xz2::write::XzEncoder::new(Vec::new(), 1);
        std::io::Write::write_all(&mut enc, &original).unwrap();
        let compressed = enc.finish().unwrap();

        let wrapped = wrap(Box::new(std::io::Cursor::new(compressed)), ImageKind::Xz).unwrap();
        let mut out = Vec::new();
        let mut wrapped = wrapped;
        wrapped.read_to_end(&mut out).unwrap();
        assert_eq!(out, original);
    }

    #[test]
    fn bzip2_roundtrip_through_wrapper() {
        let original: Vec<u8> = (0..64_000u32).map(|i| (i % 17) as u8).collect();
        let mut enc = bzip2::write::BzEncoder::new(Vec::new(), bzip2::Compression::default());
        std::io::Write::write_all(&mut enc, &original).unwrap();
        let compressed = enc.finish().unwrap();

        let wrapped = wrap(Box::new(std::io::Cursor::new(compressed)), ImageKind::Bzip2).unwrap();
        let mut out = Vec::new();
        let mut wrapped = wrapped;
        wrapped.read_to_end(&mut out).unwrap();
        assert_eq!(out, original);
    }
}
