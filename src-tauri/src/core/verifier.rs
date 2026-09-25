//! Verification stage: re-reads confirmed write spans and hashes them.
//!
//! Runs concurrently with the writer (it consumes write spans as they are
//! confirmed), so the only extra cost of verification is the device
//! re-read itself — the source hash was computed for free by the reader
//! during writing.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use blake3::Hasher;

use crate::core::progress::ProgressEmitter;
use crate::core::writer::{BlockReader, WriteSpan};
use crate::core::{EventSink, VerifyPayload, BLOCK_SIZE};
use crate::error::AppError;
use crate::safety::HINT_UNKNOWN_STATE;

/// Result of a successful verification pass.
pub struct VerifiedOutput {
    pub blake3_hex: String,
    /// Bytes actually re-read and hashed (asserted in tests).
    #[cfg_attr(not(test), allow(dead_code))]
    pub checked_bytes: u64,
}

/// Consume write spans, re-read each span from the device and hash it.
///
/// * `spans` — channel fed by the writer; closes when the writer finishes.
/// * `total_written` — shared counter updated by the writer while it runs.
/// * `throttle` — minimum interval between `flash://verify` events.
pub fn run_verify(
    mut reader: Box<dyn BlockReader>,
    spans: Receiver<WriteSpan>,
    total_written: std::sync::Arc<AtomicU64>,
    cancel: crate::core::cancel_token::CancelToken,
    sink: &dyn EventSink,
    throttle: Duration,
) -> Result<VerifiedOutput, AppError> {
    let mut hasher = Hasher::new();
    let mut emitter = ProgressEmitter::new(throttle);
    let mut checked: u64 = 0;
    let mut buffer = vec![0u8; BLOCK_SIZE];

    // Channel closed and fully drained: the writer is done.
    while let Ok(span) = spans.recv() {
        if cancel.is_cancelled() {
            drain_spans(spans);
            return Err(AppError::Cancelled);
        }

        let mut done_in_span: u32 = 0;
        while done_in_span < span.len {
            if cancel.is_cancelled() {
                drain_spans(spans);
                return Err(AppError::Cancelled);
            }
            let chunk = usize::try_from(span.len - done_in_span)
                .unwrap_or(0)
                .min(buffer.len());
            if chunk == 0 {
                return Err(AppError::device("verification chunk computation failed"));
            }
            let slice = &mut buffer[..chunk];
            reader
                .read_at(span.offset + u64::from(done_in_span), slice)
                .map_err(|e| AppError::Device {
                    message: format!("verify read failed at offset {}: {e}", span.offset),
                    hint: Some(HINT_UNKNOWN_STATE.to_string()),
                })?;
            hasher.update(slice);
            done_in_span += chunk as u32;
            checked += chunk as u64;

            let now = Instant::now();
            if emitter.should_emit(now) {
                let total = total_written.load(Ordering::SeqCst).max(checked);
                let percent = if total > 0 {
                    (checked as f64 / total as f64) * 100.0
                } else {
                    0.0
                };
                sink.verify_progress(&VerifyPayload {
                    checked,
                    total,
                    percent,
                });
            }
        }
    }

    if cancel.is_cancelled() {
        return Err(AppError::Cancelled);
    }

    // Always emit the final 100% verification state.
    let total = total_written.load(Ordering::SeqCst).max(checked);
    sink.verify_progress(&VerifyPayload {
        checked,
        total,
        percent: 100.0,
    });

    sink.log(format!(
        "verify: read-back finished ({} bytes hashed), comparing with source hash",
        checked
    ));

    Ok(VerifiedOutput {
        blake3_hex: hasher.finalize().to_hex().to_string(),
        checked_bytes: checked,
    })
}

/// Drain the span channel so a cancelled pipeline never deadlocks.
fn drain_spans(spans: Receiver<WriteSpan>) {
    for _ in spans {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::writer::MemoryDevice;

    fn dev_with(data: &[u8]) -> MemoryDevice {
        let dev = MemoryDevice::new();
        let mut w = dev.writer();
        use crate::core::writer::BlockWriter;
        w.write_at(0, data).unwrap();
        dev
    }

    #[test]
    fn identical_stream_verifies() {
        let data: Vec<u8> = (0..8192u32).map(|i| (i % 251) as u8).collect();
        let dev = dev_with(&data);
        let reader: Box<dyn BlockReader> = Box::new(dev.reader());
        // Feed spans through a helper thread so the verifier has work to do.
        let (tx, rx) = std::sync::mpsc::sync_channel::<WriteSpan>(8);
        let sink = crate::core::test_support::NoopSink;
        let total = std::sync::Arc::new(AtomicU64::new(data.len() as u64));
        let handle = std::thread::spawn(move || {
            let _ = tx.send(WriteSpan {
                offset: 0,
                len: 4096,
            });
            let _ = tx.send(WriteSpan {
                offset: 4096,
                len: 4096,
            });
        });
        let out = run_verify(reader, rx, total, Default::default(), &sink, Duration::ZERO);
        handle.join().unwrap();
        let out = out.unwrap();
        assert_eq!(out.checked_bytes, 8192);
        let mut expect = Hasher::new();
        expect.update(&data);
        assert_eq!(out.blake3_hex, expect.finalize().to_hex().to_string());
    }

    #[test]
    fn corrupted_byte_fails_hash() {
        // The verifier itself only hashes; the corruption detection happens by
        // comparing hashes in the pipeline (covered in pipeline tests). Here we
        // assert that a corrupted device produces a DIFFERENT hash.
        let data: Vec<u8> = vec![3u8; 4096];
        let dev = dev_with(&data);
        dev.corrupt(1000);
        let reader: Box<dyn BlockReader> = Box::new(dev.reader());
        let (tx, rx) = std::sync::mpsc::sync_channel::<WriteSpan>(8);
        let sink = crate::core::test_support::NoopSink;
        let total = std::sync::Arc::new(AtomicU64::new(4096));
        let handle = std::thread::spawn(move || {
            let _ = tx.send(WriteSpan {
                offset: 0,
                len: 4096,
            });
        });
        let out = run_verify(reader, rx, total, Default::default(), &sink, Duration::ZERO);
        handle.join().unwrap();
        let mut expect = Hasher::new();
        expect.update(&data);
        assert_ne!(
            out.unwrap().blake3_hex,
            expect.finalize().to_hex().to_string()
        );
    }

    // Silence unused import when MemoryReader import is only used in some paths.
    #[allow(dead_code)]
    fn _assert_traits() {
        fn is_reader<T: BlockReader>(_: &T) {}
        let dev = MemoryDevice::new();
        let r = dev.reader();
        is_reader(&r);
    }
}
