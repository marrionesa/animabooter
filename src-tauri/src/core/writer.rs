//! Block-oriented device abstractions.
//!
//! The pipeline never touches a raw device directly: it goes through the
//! `BlockWriter` / `BlockReader` traits, implemented per platform. This keeps
//! the pipeline testable (`MemoryDevice` is the unit-test target) and the
//! platform code thin.

use std::io;
use std::sync::{Arc, Mutex};

/// Sector-aligned writer target (physical device or memory).
pub trait BlockWriter: Send {
    /// Write `data` at absolute byte `offset`.
    /// Implementations must satisfy platform alignment requirements on their
    /// own (e.g. sector alignment under FILE_FLAG_NO_BUFFERING on Windows).
    fn write_at(&mut self, offset: u64, data: &[u8]) -> io::Result<()>;

    /// Flush all OS-level buffers to the physical medium. The pipeline MUST
    /// call this — and it MUST succeed — before success can be reported.
    fn sync(&mut self) -> io::Result<()>;

    /// Native sector size in bytes (fallback: 512).
    fn sector_size(&self) -> u64 {
        512
    }
}

/// Read-back target used by the verifier. A separate handle from the writer
/// so the re-read observes what actually landed on the device.
pub trait BlockReader: Send {
    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> io::Result<()>;
}

/// Confirmed-write span sent from the writer to the verifier.
#[derive(Debug, Clone, Copy)]
pub struct WriteSpan {
    pub offset: u64,
    pub len: u32,
}

/// In-memory device used by unit tests. NOT a stub: production targets
/// implement the exact same traits against real device files/handles.
#[derive(Clone, Default)]
pub struct MemoryDevice(Arc<Mutex<Vec<u8>>>);

impl MemoryDevice {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(Vec::new())))
    }

    /// Full copy of everything written so far (test assertions only).
    pub fn written(&self) -> Vec<u8> {
        self.0.lock().expect("memory device poisoned").clone()
    }

    /// Number of bytes written so far.
    pub fn written_len(&self) -> usize {
        self.0.lock().expect("memory device poisoned").len()
    }

    pub fn writer(&self) -> MemoryWriter {
        MemoryWriter {
            device: self.clone(),
        }
    }

    pub fn reader(&self) -> MemoryReader {
        MemoryReader {
            device: self.clone(),
        }
    }

    /// Flip one byte (test support for verification-failure cases).
    pub fn corrupt(&self, offset: u64) {
        let mut guard = self.0.lock().expect("memory device poisoned");
        let i = offset as usize;
        if let Some(byte) = guard.get_mut(i) {
            *byte ^= 0x5A; // XOR with a non-zero byte always changes the value
        }
    }
}

pub struct MemoryWriter {
    device: MemoryDevice,
}

impl MemoryWriter {
    pub fn corrupt(&self, offset: u64) {
        self.device.corrupt(offset);
    }
}

impl BlockWriter for MemoryWriter {
    fn write_at(&mut self, offset: u64, data: &[u8]) -> io::Result<()> {
        let mut guard = self.device.0.lock().expect("memory device poisoned");
        let start = offset as usize;
        let end = start + data.len();
        if guard.len() < end {
            guard.resize(end, 0);
        }
        guard[start..end].copy_from_slice(data);
        Ok(())
    }

    fn sync(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub struct MemoryReader {
    device: MemoryDevice,
}

impl BlockReader for MemoryReader {
    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> io::Result<()> {
        let guard = self.device.0.lock().expect("memory device poisoned");
        let start = offset as usize;
        let end = start + buf.len();
        if end > guard.len() {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "read past end of written data",
            ));
        }
        buf.copy_from_slice(&guard[start..end]);
        Ok(())
    }
}
