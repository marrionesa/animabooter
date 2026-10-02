//! Core flash pipeline.
//!
//! Three stages connected by bounded mpsc channels (natural backpressure):
//!
//! ```text
//! [Reader] ──blocks──► [Writer] ──spans──► [Verifier]
//! ```
//!
//! The core never depends on Tauri: all UI feedback goes through the
//! `EventSink` trait, which keeps this crate unit-testable on any machine.
//!
//! NOTE: this module is named `core` per the project layout. Within this
//! crate always reference it as `crate::core::…` (never unqualified `core::`,
//! which would resolve to the standard library prelude).

pub mod cancel_token;
pub mod pipeline;
pub mod progress;
pub mod verifier;
pub mod writer;

use serde::Serialize;

/// Streaming block size for every stage (4 MiB). The image is NEVER fully
/// loaded into RAM.
pub const BLOCK_SIZE: usize = 4 * 1024 * 1024;

/// Flash phases reported to the frontend (`flash://phase`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Writing,
    Verifying,
    Finalizing,
    Done,
}

/// `flash://progress` payload. `total`, `eta_secs` and `percent` are null
/// when the uncompressed size of the source image is unknown (compressed
/// streams): we prefer honest byte counters over fake percentages.
#[derive(Debug, Clone, Serialize)]
pub struct ProgressPayload {
    pub written: u64,
    pub total: Option<u64>,
    pub speed_mbs: f64,
    pub eta_secs: Option<f64>,
    pub percent: Option<f64>,
}

/// `flash://verify` payload (verification has its own progress channel).
#[derive(Debug, Clone, Serialize)]
pub struct VerifyPayload {
    pub checked: u64,
    pub total: u64,
    pub percent: f64,
}

/// `flash://done` payload.
#[derive(Debug, Clone, Serialize)]
pub struct DonePayload {
    pub total_bytes: u64,
    pub elapsed_secs: f64,
    pub avg_speed_mbs: f64,
    pub peak_speed_mbs: f64,
    pub verified: bool,
    pub sha256: String,
}

/// `flash://error` payload.
#[derive(Debug, Clone, Serialize)]
pub struct ErrorPayload {
    pub message: String,
    pub hint: Option<String>,
}

/// UI feedback bridge. Implemented by `TauriEventSink` (production) and by
/// test sinks (unit tests), so the pipeline itself stays UI-free.
pub trait EventSink: Send + Sync + 'static {
    fn phase(&self, phase: Phase);
    fn progress(&self, payload: &ProgressPayload);
    fn verify_progress(&self, payload: &VerifyPayload);
    fn log(&self, line: String);
    fn error(&self, payload: &ErrorPayload);
    fn done(&self, payload: &DonePayload);
}

/// UTC wall-clock timestamp for log lines, e.g. `[14:03:22]`.
/// Kept dependency-free on purpose (no chrono).
pub fn timestamp() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let t = secs % 86_400;
    format!("[{:02}:{:02}:{:02}]", t / 3600, (t % 3600) / 60, t % 60)
}

/// Emit an error payload built from `AppError` through the sink.
pub fn emit_error(sink: &dyn EventSink, err: &crate::error::AppError) {
    sink.error(&ErrorPayload {
        message: err.to_string(),
        hint: err.hint(),
    });
}

/// Minimal sink used by unit tests (discards every event).
/// Production uses `TauriEventSink` (see `state.rs`).
#[cfg(test)]
pub mod test_support {
    use super::{EventSink, Phase};

    #[derive(Default)]
    pub struct NoopSink;

    impl EventSink for NoopSink {
        fn phase(&self, _phase: Phase) {}
        fn progress(&self, _payload: &super::ProgressPayload) {}
        fn verify_progress(&self, _payload: &super::VerifyPayload) {}
        fn log(&self, _line: String) {}
        fn error(&self, _payload: &super::ErrorPayload) {}
        fn done(&self, _payload: &super::DonePayload) {}
    }
}
