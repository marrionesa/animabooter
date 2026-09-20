//! Speed tracking and event throttling for the pipeline stages.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// One MiB in bytes.
pub const MIB: f64 = 1024.0 * 1024.0;

/// Window size (in block samples) for the moving-average speed.
pub const WINDOW_SAMPLES: usize = 20;

/// Rolling speed tracker over the last `WINDOW_SAMPLES` samples.
///
/// * `sample()` returns the current window average (MiB/s).
/// * `peak_mbs()` returns the highest instantaneous speed seen.
#[derive(Debug)]
pub struct SpeedTracker {
    samples: VecDeque<(Instant, u64)>,
    last: Option<(Instant, u64)>,
    peak_mbs: f64,
}

impl SpeedTracker {
    pub fn new() -> Self {
        Self {
            samples: VecDeque::with_capacity(WINDOW_SAMPLES + 1),
            last: None,
            peak_mbs: 0.0,
        }
    }

    /// Register a block write at `now` reaching `cumulative_bytes` in total.
    /// Returns the window average speed in MiB/s.
    pub fn sample(&mut self, now: Instant, cumulative_bytes: u64) -> f64 {
        if let Some((last_t, last_b)) = self.last {
            let dt = now.duration_since(last_t).as_secs_f64();
            if dt > 0.0 && cumulative_bytes >= last_b {
                let instant = (cumulative_bytes - last_b) as f64 / dt / MIB;
                if instant > self.peak_mbs {
                    self.peak_mbs = instant;
                }
            }
        }
        self.last = Some((now, cumulative_bytes));
        self.samples.push_back((now, cumulative_bytes));
        while self.samples.len() > WINDOW_SAMPLES {
            self.samples.pop_front();
        }
        self.window_average(now, cumulative_bytes)
    }

    /// Average speed across the retained window.
    fn window_average(&self, now: Instant, cumulative: u64) -> f64 {
        if let Some(&(old_t, old_b)) = self.samples.front() {
            let dt = now.duration_since(old_t).as_secs_f64();
            if dt > f64::EPSILON && cumulative >= old_b {
                return (cumulative - old_b) as f64 / dt / MIB;
            }
        }
        0.0
    }

    pub fn peak_mbs(&self) -> f64 {
        self.peak_mbs
    }

    /// Number of retained samples (exposed for tests).
    #[cfg(test)]
    pub fn samples(&self) -> usize {
        self.samples.len()
    }
}

impl Default for SpeedTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Average MiB/s over a whole duration. Zero when `secs` is not positive.
pub fn mib_per_sec(bytes: u64, secs: f64) -> f64 {
    if secs <= 0.0 {
        0.0
    } else {
        bytes as f64 / secs / MIB
    }
}

/// Throttles event emission to at most one event per `min_interval`.
/// The very first call always emits (so the UI gets immediate feedback).
#[derive(Debug)]
pub struct ProgressEmitter {
    min_interval: Duration,
    last_emit: Option<Instant>,
}

impl ProgressEmitter {
    pub fn new(min_interval: Duration) -> Self {
        Self {
            min_interval,
            last_emit: None,
        }
    }

    /// Returns true when an event should be emitted at `now`.
    pub fn should_emit(&mut self, now: Instant) -> bool {
        match self.last_emit {
            None => {
                self.last_emit = Some(now);
                true
            }
            Some(last) => {
                if now.duration_since(last) >= self.min_interval {
                    self.last_emit = Some(now);
                    true
                } else {
                    false
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::BLOCK_SIZE;

    #[test]
    fn window_average_math_is_exact() {
        // 5 blocks of 4 MiB, one per second => 4 MiB/s both window and overall.
        let t0 = Instant::now();
        let mut tracker = SpeedTracker::new();
        tracker.sample(t0, 0);
        for i in 1..=5u64 {
            let t = t0 + Duration::from_secs(i);
            let avg = tracker.sample(t, i * BLOCK_SIZE as u64);
            assert!((avg - 4.0).abs() < 1e-9, "expected 4 MiB/s, got {avg}");
        }
        assert_eq!(tracker.samples(), WINDOW_SAMPLES.min(6));
    }

    #[test]
    fn peak_tracks_fastest_instant() {
        let t0 = Instant::now();
        let mut tracker = SpeedTracker::new();
        tracker.sample(t0, 0);
        // 8 MiB in 1 s => 8 MiB/s
        tracker.sample(t0 + Duration::from_secs(1), (8.0 * MIB) as u64);
        // 4 MiB in the next 2 s => 2 MiB/s
        tracker.sample(t0 + Duration::from_secs(3), (12.0 * MIB) as u64);
        assert!((tracker.peak_mbs() - 8.0).abs() < 1e-9);
        // Window spans ALL retained samples: 12 MiB over the full 3 s => 4 MiB/s.
        assert!(
            (tracker
                .window_only_avg_assert_helper(t0 + Duration::from_secs(3), (12.0 * MIB) as u64)
                - 4.0)
                .abs()
                < 1e-9
        );
    }

    impl SpeedTracker {
        // Small helper so the test above can check the last window explicitly
        // without exposing `window_average` publicly.
        fn window_only_avg_assert_helper(&self, now: Instant, cumulative: u64) -> f64 {
            self.window_average(now, cumulative)
        }
    }

    #[test]
    fn throttle_first_event_always_emits_and_then_respects_interval() {
        let mut emitter = ProgressEmitter::new(Duration::from_millis(200));
        let t0 = Instant::now();
        assert!(emitter.should_emit(t0));
        assert!(!emitter.should_emit(t0 + Duration::from_millis(150)));
        assert!(emitter.should_emit(t0 + Duration::from_millis(200)));
        assert!(!emitter.should_emit(t0 + Duration::from_millis(300)));
        assert!(emitter.should_emit(t0 + Duration::from_millis(401)));
    }

    #[test]
    fn mib_per_sec_handles_zero_time() {
        assert_eq!(mib_per_sec(1024, 0.0), 0.0);
        assert!((mib_per_sec(2 * 1024 * 1024, 1.0) - 2.0).abs() < 1e-9);
    }
}
