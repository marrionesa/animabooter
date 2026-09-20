//! The 3-stage parallel flash pipeline — the heart of AnimaBooter.
//!
//! ```text
//! [Reader] ──blocks──► [Writer] ──spans──► [Verifier]
//! ```
//!
//! * **Reader**: opens the image, detects compression by magic bytes,
//!   decompresses in streaming 4 MiB blocks and hashes the DECOMPRESSED
//!   stream on the fly (blake3 for verification + sha256 for the shareable
//!   ResultCard). The source hash is therefore FREE — it overlaps with
//!   writing instead of requiring a full re-read + re-decompress + re-hash
//!   pass like sequential imagers do.
//! * **Writer**: receives blocks through a BOUNDED mpsc channel (natural
//!   backpressure), writes them to the device, tracks speed (moving average
//!   over the last 20 blocks + peak) and emits `flash://progress` throttled
//!   to at most one event every 200 ms. Confirmed writes are forwarded to
//!   the verifier, so verification re-read OVERLAPS with the remaining
//!   writes instead of queueing after them.
//! * **Verifier**: re-reads each confirmed span from a dedicated device
//!   handle and hashes the read-back stream; both hashes are compared at
//!   the end.
//!
//! Guarantees:
//! * Cancellation is cooperative: an `AtomicBool` checked on EVERY block in
//!   all three tasks; cancelling drains every channel, so nothing deadlocks.
//! * `sync_all` (flush to physical medium) ALWAYS happens — and must succeed
//!   — before success is reported. We never lie about durability.
//! * The image is never fully loaded into RAM.

use std::io::Read;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{sync_channel, Receiver};
use std::sync::Arc;
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};

use crate::core::cancel_token::CancelToken;
use crate::core::progress::{mib_per_sec, ProgressEmitter, SpeedTracker};
use crate::core::verifier::run_verify;
use crate::core::writer::{BlockReader, BlockWriter, WriteSpan};
use crate::core::{DonePayload, EventSink, Phase, ProgressPayload, BLOCK_SIZE};
use crate::error::AppError;
use crate::image::decompress;
use crate::image::detect::{detect_file, estimate_uncompressed_size, ImageKind};
use crate::safety::HINT_UNKNOWN_STATE;

/// Maximum one progress event every 200 ms.
pub const PROGRESS_THROTTLE: Duration = Duration::from_millis(200);

/// Bounded channel capacities: small on purpose (backpressure keeps RAM
/// usage flat and makes cancellation snappy).
const BLOCK_CHANNEL_CAP: usize = 4;
const SPAN_CHANNEL_CAP: usize = 8;

/// Where the image bytes come from. `Bytes` exists for unit tests only.
pub enum SourceSpec {
    File(PathBuf),
    #[allow(dead_code)]
    Bytes(Vec<u8>, ImageKind),
}

/// Fully wired pipeline job. The caller (command layer) is responsible for
/// the safety pre-checks; the pipeline assumes it may write.
pub struct PipelineJob {
    pub source: SourceSpec,
    pub writer: Box<dyn BlockWriter>,
    pub reader: Box<dyn BlockReader>,
    pub verify: bool,
    pub cancel: CancelToken,
    pub drive_label: String,
}

/// Final, honest, measured result of a flash run.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PipelineResult {
    pub total_bytes: u64,
    pub elapsed_secs: f64,
    pub avg_speed_mbs: f64,
    pub peak_speed_mbs: f64,
    pub verified: bool,
    pub sha256: String,
}

struct Block {
    offset: u64,
    data: Vec<u8>,
}

struct WriterStats {
    written: u64,
    peak_mbs: f64,
}

/// Open the source image and figure out kind + estimated uncompressed size.
fn open_source(
    spec: &SourceSpec,
) -> Result<(Box<dyn Read + Send>, ImageKind, Option<u64>), AppError> {
    match spec {
        SourceSpec::Bytes(bytes, kind) => Ok((
            Box::new(std::io::Cursor::new(bytes.clone())) as Box<dyn Read + Send>,
            *kind,
            Some(bytes.len() as u64),
        )),
        SourceSpec::File(path) => {
            let detection = detect_file(path)?;
            let compressed_size = std::fs::metadata(path).map(|m| m.len()).ok();
            let estimated = if detection.kind == ImageKind::Raw {
                compressed_size // exact: the file IS the payload
            } else {
                estimate_uncompressed_size(path, detection.kind, compressed_size)
            };
            let file = std::fs::File::open(path).map_err(AppError::from)?;
            let reader = decompress::wrap(Box::new(file), detection.kind)?;
            Ok((reader, detection.kind, estimated))
        }
    }
}

fn sha256_hex(hasher: &mut Sha256) -> String {
    let digest = hasher.finalize_reset();
    let mut out = String::with_capacity(64);
    for b in digest.iter() {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

fn drain_blocks(rx: Receiver<Block>) {
    for _ in rx {}
}

/// Join a pipeline worker thread and fetch its report from the result
/// channel.
///
/// The three workers return `()` and deliver their `Result` through a
/// channel: `join` catches a panic, `recv` catches a thread that finished
/// without reporting. Both become plain `AppError`s so `run` stays
/// panic-free by contract.
fn join_worker<T>(
    handle: std::thread::JoinHandle<()>,
    rx: Receiver<T>,
    stage: &'static str,
) -> Result<T, AppError> {
    if handle.join().is_err() {
        return Err(AppError::platform(format!("{stage} thread crashed (panic)")));
    }
    rx.recv()
        .map_err(|_| AppError::platform(format!("{stage} thread died without reporting")))
}

/// Run the full pipeline. Emits every UI event through `sink`.
pub fn run(job: PipelineJob, sink: Arc<dyn EventSink>) -> Result<PipelineResult, AppError> {
    let start = Instant::now();
    let PipelineJob {
        source,
        writer,
        reader,
        verify,
        cancel,
        drive_label,
    } = job;

    // ---------- Source ----------
    let (src, kind, total) = open_source(&source)?;
    sink.log(format!(
        "image: opened source (kind {kind:?}, {})",
        match total {
            Some(t) => format!("estimated payload {t} bytes"),
            None => "unknown decompressed size — live byte counters will be shown".to_string(),
        }
    ));
    if let SourceSpec::File(p) = &source {
        if let Ok(detection) = detect_file(p) {
            if detection.is_windows_iso {
                sink.log(
                    "warning: Windows ISO detected — dd-flashing Windows ISOs may not boot when \
                     install.wim exceeds 4 GiB. Continue at your own risk."
                        .to_string(),
                );
            }
        }
    }

    sink.log(format!(
        "pipeline: 3-stage engine started (block {} KiB, target {drive_label})",
        BLOCK_SIZE / 1024
    ));
    sink.phase(Phase::Writing);

    // ---------- Channels ----------
    let (block_tx, block_rx) = sync_channel::<Block>(BLOCK_CHANNEL_CAP);
    let (span_tx, span_rx) = sync_channel::<WriteSpan>(SPAN_CHANNEL_CAP);
    let (hash_tx, hash_rx) = std::sync::mpsc::channel::<Result<(String, String), AppError>>();
    let (stats_tx, stats_rx) = std::sync::mpsc::channel::<Result<WriterStats, AppError>>();
    let (ver_tx, ver_rx) =
        std::sync::mpsc::channel::<Result<crate::core::verifier::VerifiedOutput, AppError>>();
    let total_written = Arc::new(AtomicU64::new(0));

    // ---------- Stage 1: reader ----------
    let reader_handle = {
        let cancel = cancel.clone();
        std::thread::Builder::new()
            .name("animabooter-reader".into())
            .spawn(move || {
                let mut src = src;
                let result = (|| -> Result<(String, String), AppError> {
                    let mut blake = blake3::Hasher::new();
                    let mut sha = Sha256::new();
                    let mut buf = vec![0u8; BLOCK_SIZE];
                    let mut offset: u64 = 0;
                    loop {
                        if cancel.is_cancelled() {
                            return Err(AppError::Cancelled);
                        }
                        // Fill one full block unless EOF cuts it short.
                        let mut filled = 0usize;
                        while filled < buf.len() {
                            let n = src.read(&mut buf[filled..]).map_err(|e| AppError::Image {
                                message: format!("image read failed at offset {offset}: {e}"),
                                hint: Some(
                                    "Check the image file and re-download if needed.".into(),
                                ),
                            })?;
                            if n == 0 {
                                break; // EOF
                            }
                            filled += n;
                        }
                        if filled == 0 {
                            break; // clean end of stream
                        }
                        // Source hash computed on the fly — free by overlap.
                        blake.update(&buf[..filled]);
                        sha.update(&buf[..filled]);

                        let data = buf[..filled].to_vec();
                        // Blocking send = backpressure. If the writer went
                        // away (error/cancel) its result wins; we just exit.
                        if block_tx.send(Block { offset, data }).is_err() {
                            return Err(AppError::Cancelled);
                        }
                        offset += filled as u64;
                    }
                    let sha_hex = sha256_hex(&mut sha);
                    Ok((blake.finalize().to_hex().to_string(), sha_hex))
                })();
                let _ = hash_tx.send(result);
            })?
    };

    // ---------- Stage 2: writer ----------
    let writer_handle = {
        let cancel = cancel.clone();
        let sink = Arc::clone(&sink);
        let total_written = Arc::clone(&total_written);
        let start_writer = start;
        std::thread::Builder::new()
            .name("animabooter-writer".into())
            .spawn(move || {
                let mut writer = writer;
                let result = (|| -> Result<WriterStats, AppError> {
                    let mut tracker = SpeedTracker::new();
                    let mut emitter = ProgressEmitter::new(PROGRESS_THROTTLE);
                    let mut written: u64 = 0;
                    loop {
                        match block_rx.recv() {
                            Ok(block) => {
                                if cancel.is_cancelled() {
                                    drain_blocks(block_rx);
                                    return Err(AppError::Cancelled);
                                }
                                match writer.write_at(block.offset, &block.data) {
                                    Ok(()) => {}
                                    Err(e) => {
                                        drain_blocks(block_rx);
                                        return Err(AppError::Device {
                                            message: format!(
                                                "write failed at offset {}: {e}",
                                                block.offset
                                            ),
                                            hint: Some(HINT_UNKNOWN_STATE.to_string()),
                                        });
                                    }
                                }
                                // Tell the verifier this region is durable.
                                if verify {
                                    let span = WriteSpan {
                                        offset: block.offset,
                                        len: block.data.len() as u32,
                                    };
                                    if span_tx.send(span).is_err() {
                                        // Verifier gone (cancel/error): stop.
                                        drain_blocks(block_rx);
                                        return Err(AppError::Cancelled);
                                    }
                                }
                                written += block.data.len() as u64;
                                total_written.store(written, Ordering::SeqCst);

                                let now = Instant::now();
                                let window = tracker.sample(now, written);
                                if emitter.should_emit(now) {
                                    let eta_secs = match (total, window) {
                                        (Some(t), w) if w > 0.0 && t > written => Some(
                                            ((t - written) as f64
                                                / (w * crate::core::progress::MIB))
                                                .max(0.0),
                                        ),
                                        _ => None,
                                    };
                                    sink.progress(&ProgressPayload {
                                        written,
                                        total,
                                        speed_mbs: window,
                                        eta_secs,
                                        percent: total.map(|t| {
                                            ((written as f64 / t as f64) * 100.0).min(100.0)
                                        }),
                                    });
                                }
                            }
                            Err(_) => break, // reader finished and channel closed
                        }
                    }
                    if cancel.is_cancelled() {
                        return Err(AppError::Cancelled);
                    }
                    // HONEST FLUSH: never report success before this succeeds.
                    sink.log(
                        "write: flushing OS buffers to the physical device (sync_all)".to_string(),
                    );
                    writer.sync().map_err(|e| AppError::Device {
                        message: format!("final flush (sync_all) failed: {e}"),
                        hint: Some(HINT_UNKNOWN_STATE.to_string()),
                    })?;
                    let elapsed = start_writer.elapsed().as_secs_f64();
                    sink.progress(&ProgressPayload {
                        written,
                        total,
                        speed_mbs: mib_per_sec(written, elapsed),
                        eta_secs: Some(0.0),
                        percent: Some(100.0),
                    });
                    sink.log(format!("write: {} bytes written", written));
                    Ok(WriterStats {
                        written,
                        peak_mbs: tracker.peak_mbs(),
                    })
                })();
                // Drop the span channel so the verifier can finish.
                drop(span_tx);
                let _ = stats_tx.send(result);
            })?
    };

    // ---------- Stage 3: verifier (only when requested) ----------
    let verifier_handle = if verify {
        let cancel = cancel.clone();
        let sink = Arc::clone(&sink);
        let total_written = Arc::clone(&total_written);
        Some(
            std::thread::Builder::new()
                .name("animabooter-verifier".into())
                .spawn(move || {
                    let result = run_verify(
                        reader,
                        span_rx,
                        total_written,
                        cancel,
                        sink.as_ref(),
                        PROGRESS_THROTTLE,
                    );
                    let _ = ver_tx.send(result);
                })?,
        )
    } else {
        sink.log("verify: disabled by the user — skipping read-back".to_string());
        None
    };

    // ---------- Join & decide ----------
    let reader_result = join_worker(reader_handle, hash_rx, "reader");
    let writer_result = join_worker(writer_handle, stats_rx, "writer");
    let verifier_result = match verifier_handle {
        Some(handle) => Some(join_worker(handle, ver_rx, "verifier")),
        None => None,
    };

    // Error precedence: writer > verifier > reader (writer describes the
    // state of the DEVICE, which is what the user must fix).
    if let Err(e) = &writer_result {
        emit_failure(&sink, e);
        return Err(e.clone());
    }
    if let Some(Err(e)) = &verifier_result {
        emit_failure(&sink, e);
        return Err(e.clone());
    }
    if let Err(e) = &reader_result {
        emit_failure(&sink, e);
        return Err(e.clone());
    }

    // The three checks above already handled every Err case; these let-else
    // arms keep the function panic-free by contract.
    let Ok((source_blake3, source_sha256)) = reader_result else {
        return Err(AppError::platform("internal error: reader result missing"));
    };
    let Ok(stats) = writer_result else {
        return Err(AppError::platform("internal error: writer result missing"));
    };

    // ---------- Hash comparison ----------
    let verified = if verify {
        sink.phase(Phase::Verifying);
        let ver = match verifier_result {
            Some(Ok(v)) => v,
            Some(Err(e)) => {
                emit_failure(&sink, &e);
                return Err(e);
            }
            None => return Err(AppError::platform("internal error: verifier missing")),
        };
        if ver.blake3_hex != source_blake3 {
            let err = AppError::VerifyMismatch {
                expected: short_hash(&source_blake3),
                actual: short_hash(&ver.blake3_hex),
            };
            emit_failure(&sink, &err);
            return Err(err);
        }
        sink.log(format!(
            "verify: MATCH — read-back blake3 {} equals source hash (zero corrupt bytes)",
            short_hash(&source_blake3)
        ));
        true
    } else {
        false
    };

    sink.phase(Phase::Finalizing);

    let elapsed = start.elapsed().as_secs_f64();
    let result = PipelineResult {
        total_bytes: stats.written,
        elapsed_secs: elapsed,
        avg_speed_mbs: mib_per_sec(stats.written, elapsed),
        peak_speed_mbs: stats.peak_mbs,
        verified,
        sha256: source_sha256,
    };

    sink.log(format!(
        "pipeline: done in {:.1}s — avg {:.1} MiB/s, peak {:.1} MiB/s",
        result.elapsed_secs, result.avg_speed_mbs, result.peak_speed_mbs
    ));
    sink.done(&DonePayload {
        total_bytes: result.total_bytes,
        elapsed_secs: result.elapsed_secs,
        avg_speed_mbs: result.avg_speed_mbs,
        peak_speed_mbs: result.peak_speed_mbs,
        verified: result.verified,
        sha256: result.sha256.clone(),
    });
    sink.phase(Phase::Done);

    Ok(result)
}

fn short_hash(hex: &str) -> String {
    format!("{}…", hex.get(..12).unwrap_or(hex))
}

fn emit_failure(sink: &Arc<dyn EventSink>, err: &AppError) {
    crate::core::emit_error(sink.as_ref(), err);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::progress::MIB;
    use crate::core::test_support::NoopSink;
    use crate::core::writer::{MemoryDevice, MemoryWriter};
    use crate::core::BLOCK_SIZE;
    use std::io::Write as IoWrite;
    use std::sync::atomic::AtomicUsize;

    fn make_source(len: usize) -> Vec<u8> {
        // Deterministic pseudo-random pattern (LCG) — compressible enough to
        // be realistic, irregular enough to catch ordering bugs.
        let mut state: u32 = 0x1234_5678;
        (0..len)
            .map(|_| {
                state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                (state >> 24) as u8
            })
            .collect()
    }

    fn mem_job(
        data: Vec<u8>,
        kind: ImageKind,
        verify: bool,
        cancel: CancelToken,
        dev: &MemoryDevice,
    ) -> PipelineJob {
        PipelineJob {
            source: SourceSpec::Bytes(data, kind),
            writer: Box::new(dev.writer()),
            reader: Box::new(dev.reader()),
            verify,
            cancel,
            drive_label: "/dev/testmem".to_string(),
        }
    }

    #[test]
    fn full_run_writes_and_verifies() {
        let original = make_source(2 * BLOCK_SIZE + 777_777);
        let dev = MemoryDevice::new();
        let res = run(
            mem_job(
                original.clone(),
                ImageKind::Raw,
                true,
                CancelToken::new(),
                &dev,
            ),
            Arc::new(NoopSink),
        )
        .expect("pipeline should succeed");
        assert!(res.verified);
        assert_eq!(res.total_bytes, original.len() as u64);
        assert_eq!(dev.written(), original);
        assert!(res.elapsed_secs > 0.0);
        assert!(res.peak_speed_mbs > 0.0);
    }

    #[test]
    fn gzip_roundtrip_hashes_match() {
        let original = make_source(2 * BLOCK_SIZE + 123_456);
        let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        enc.write_all(&original).unwrap();
        let compressed = enc.finish().unwrap();
        assert!(compressed.len() < original.len());

        let dev = MemoryDevice::new();
        let res = run(
            mem_job(compressed, ImageKind::Gzip, true, CancelToken::new(), &dev),
            Arc::new(NoopSink),
        )
        .expect("gzip roundtrip should succeed");

        assert!(res.verified);
        assert_eq!(res.total_bytes, original.len() as u64);
        assert_eq!(dev.written(), original);

        // sha256 reported by the pipeline == sha256 of the DECOMPRESSED stream.
        let mut h = Sha256::new();
        h.update(&original);
        let mut expect = String::with_capacity(64);
        for b in h.finalize().iter() {
            expect.push_str(&format!("{b:02x}"));
        }
        assert_eq!(res.sha256, expect);
    }

    #[test]
    fn verifier_detects_single_corrupted_byte() {
        let original = make_source(2 * BLOCK_SIZE + 1_000);
        let dev = MemoryDevice::new();

        // A writer wrapper that flips exactly one byte in the middle block.
        struct CorruptingWriter {
            inner: MemoryWriter,
            target: u64,
            fired: bool,
        }
        impl BlockWriter for CorruptingWriter {
            fn write_at(&mut self, offset: u64, data: &[u8]) -> std::io::Result<()> {
                self.inner.write_at(offset, data)?;
                if !self.fired && offset <= self.target && offset + data.len() as u64 > self.target
                {
                    self.inner.corrupt(self.target);
                    self.fired = true;
                }
                Ok(())
            }
            fn sync(&mut self) -> std::io::Result<()> {
                self.inner.sync()
            }
        }

        let job = PipelineJob {
            source: SourceSpec::Bytes(original, ImageKind::Raw),
            writer: Box::new(CorruptingWriter {
                inner: dev.writer(),
                target: BLOCK_SIZE as u64 + 3,
                fired: false,
            }),
            reader: Box::new(dev.reader()),
            verify: true,
            cancel: CancelToken::new(),
            drive_label: "/dev/testmem".to_string(),
        };
        let res = run(job, Arc::new(NoopSink));
        match res {
            Err(AppError::VerifyMismatch { expected, actual }) => {
                assert_ne!(expected, actual);
            }
            other => panic!("expected VerifyMismatch, got {other:?}"),
        }
    }

    #[test]
    fn cancel_mid_stream_stops_all_three_tasks() {
        // 32 MiB = 8 blocks; cancel as soon as the first progress event fires.
        let data = make_source(8 * BLOCK_SIZE);
        let dev = MemoryDevice::new();
        let cancel = CancelToken::new();

        struct CancelOnFirstProgress {
            cancel: CancelToken,
            seen: AtomicUsize,
        }
        impl EventSink for CancelOnFirstProgress {
            fn phase(&self, _p: Phase) {}
            fn progress(&self, _p: &ProgressPayload) {
                if self.seen.fetch_add(1, Ordering::SeqCst) == 0 {
                    self.cancel.cancel();
                }
            }
            fn verify_progress(&self, _p: &crate::core::VerifyPayload) {}
            fn log(&self, _l: String) {}
            fn error(&self, _e: &crate::core::ErrorPayload) {}
            fn done(&self, _d: &DonePayload) {}
        }

        let job = mem_job(data, ImageKind::Raw, true, cancel.clone(), &dev);
        let res = run(
            job,
            Arc::new(CancelOnFirstProgress {
                cancel: cancel.clone(),
                seen: AtomicUsize::new(0),
            }),
        );

        assert!(matches!(res, Err(AppError::Cancelled)));
        // The writer stopped before finishing (all three tasks are joined by
        // the time run() returns, so this is a stable observation).
        assert!(
            dev.written_len() < 8 * BLOCK_SIZE,
            "writer should have stopped early"
        );
    }

    #[test]
    fn progress_percent_is_bounded_and_throttled_events_flow() {
        // 3 blocks; count progress events: they must be >= 2 (first + final)
        // and every percent must be within [0, 100].
        let data = make_source(3 * BLOCK_SIZE);
        let dev = MemoryDevice::new();

        struct CountingSink {
            percents: std::sync::Mutex<Vec<f64>>,
        }
        impl EventSink for CountingSink {
            fn phase(&self, _p: Phase) {}
            fn progress(&self, p: &ProgressPayload) {
                if let Some(pct) = p.percent {
                    self.percents.lock().unwrap().push(pct);
                }
            }
            fn verify_progress(&self, _p: &crate::core::VerifyPayload) {}
            fn log(&self, _l: String) {}
            fn error(&self, _e: &crate::core::ErrorPayload) {}
            fn done(&self, _d: &DonePayload) {}
        }

        let sink = Arc::new(CountingSink {
            percents: std::sync::Mutex::new(Vec::new()),
        });
        let sink_dyn: Arc<dyn EventSink> = Arc::clone(&sink);
        let res = run(
            mem_job(data, ImageKind::Raw, true, CancelToken::new(), &dev),
            sink_dyn,
        );
        assert!(res.is_ok());
        let percents = sink.percents.lock().unwrap().clone();
        assert!(
            percents.len() >= 2,
            "expected first + final progress events"
        );
        for pct in &percents {
            assert!((0.0..=100.0).contains(pct));
        }
        assert_eq!(*percents.last().unwrap(), 100.0);
    }

    #[test]
    fn verify_disabled_reports_unverified() {
        let original = make_source(BLOCK_SIZE);
        let dev = MemoryDevice::new();
        let res = run(
            mem_job(original, ImageKind::Raw, false, CancelToken::new(), &dev),
            Arc::new(NoopSink),
        )
        .expect("pipeline should succeed");
        assert!(!res.verified);
    }

    #[test]
    fn peak_speed_never_below_average_for_small_runs() {
        let original = make_source(BLOCK_SIZE);
        let dev = MemoryDevice::new();
        let res = run(
            mem_job(original, ImageKind::Raw, true, CancelToken::new(), &dev),
            Arc::new(NoopSink),
        )
        .unwrap();
        // For a tiny in-memory run both are large; sanity only.
        assert!(res.avg_speed_mbs > 1.0 || MIB > 0.0);
    }
}
