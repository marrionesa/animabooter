//! Streaming ISO downloader with official resolver chains.
//!
//! Design notes:
//! - ISO URLs and hashes are NEVER hardcoded: every catalog entry resolves,
//!   at download time, against an official index (SHA256SUMS text file or
//!   Fedora's releases.json) and picks the newest matching release. Distros
//!   delete old point releases, so pinned URLs rot within months.
//! - The download streams to `<cache>/<name>.part`, hashing while writing
//!   (SHA-256 from the official index), then atomically renames to the final
//!   name only after verification passes.
//! - Cancellation is cooperative via the shared `CancelToken`; the partial
//!   `.part` file is always cleaned up.
//! - Already-cached files are verified before reuse; a corrupt cache entry
//!   is deleted and re-downloaded.

use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::core::cancel_token::CancelToken;
use crate::error::AppError;

use super::OsEntry;

// ---------------------------------------------------------------------------
// Public shapes
// ---------------------------------------------------------------------------

/// A resolved, ready-to-download official ISO.
#[derive(Debug, Clone)]
pub struct ResolvedIso {
    /// Direct official URL (may redirect to an official mirror).
    pub url: String,
    /// SHA-256 hex digest as published by the official index.
    pub sha256: String,
    /// Exact ISO file name, e.g. "ubuntu-24.04.4-desktop-amd64.iso".
    pub name: String,
}

/// Progress payload for the `download://progress` event.
#[derive(Debug, Clone, Serialize)]
pub struct DownloadProgress {
    pub downloaded: u64,
    pub total: Option<u64>,
    pub speed_mbs: f64,
    pub eta_secs: Option<f64>,
    pub percent: Option<f64>,
}

/// Command result: honest, measured outcome of a finished download.
#[derive(Debug, Clone, Serialize)]
pub struct DownloadOutcome {
    pub path: String,
    pub name: String,
    pub size: u64,
    pub sha256: String,
    /// True when the SHA-256 matched the official index (always true unless
    /// the index published no hash — v1 catalog always does).
    pub verified: bool,
    /// True when the file was already in the cache and passed verification.
    pub cached: bool,
}

/// Event bridge so `downloader` stays Tauri-free and testable.
pub trait DownloadEvents: Send + Sync {
    /// Coarse state machine: resolving | downloading | verifying | done.
    fn status(&self, state: &str);
    /// Throttled progress samples.
    fn progress(&self, p: &DownloadProgress);
    /// Human-readable log line (lands in the same terminal-style log).
    fn log(&self, line: String);
}

// ---------------------------------------------------------------------------
// Official indexes (verified live before shipping this feature)
// ---------------------------------------------------------------------------

const UBUNTU_INDEX: &str = "https://releases.ubuntu.com/noble/SHA256SUMS";
const UBUNTU_BASE: &str = "https://releases.ubuntu.com/noble/";

const XUBUNTU_INDEX: &str =
    "https://cdimage.ubuntu.com/xubuntu/releases/24.04/release/SHA256SUMS";
const XUBUNTU_BASE: &str = "https://cdimage.ubuntu.com/xubuntu/releases/24.04/release/";

const LUBUNTU_INDEX: &str =
    "https://cdimage.ubuntu.com/lubuntu/releases/24.04/release/SHA256SUMS";
const LUBUNTU_BASE: &str = "https://cdimage.ubuntu.com/lubuntu/releases/24.04/release/";

const UBUNTU_MATE_INDEX: &str =
    "https://cdimage.ubuntu.com/ubuntu-mate/releases/24.04/release/SHA256SUMS";
const UBUNTU_MATE_BASE: &str = "https://cdimage.ubuntu.com/ubuntu-mate/releases/24.04/release/";

const DEBIAN_INDEX: &str =
    "https://cdimage.debian.org/debian-cd/current/amd64/iso-cd/SHA256SUMS";
const DEBIAN_BASE: &str = "https://cdimage.debian.org/debian-cd/current/amd64/iso-cd/";

const FEDORA_RELEASES_JSON: &str = "https://fedoraproject.org/releases.json";

const ARCH_INDEX: &str = "https://geo.mirror.pkgbuild.com/iso/latest/sha256sums.txt";
const ARCH_BASE: &str = "https://geo.mirror.pkgbuild.com/iso/latest/";
const ARCH_NAME: &str = "archlinux-x86_64.iso";

/// Cap the in-memory size of index files (all of them are a few KiB).
const INDEX_MAX_BYTES: usize = 2 * 1024 * 1024;
/// A single chunk read must make progress within this window, otherwise the
/// connection is considered stalled (no total timeout: ISOs are gigabytes).
const CHUNK_TIMEOUT: Duration = Duration::from_secs(120);
/// Progress event throttling window.
const EMIT_INTERVAL: Duration = Duration::from_millis(250);

// ---------------------------------------------------------------------------
// Resolution
// ---------------------------------------------------------------------------

/// Resolve `os_id` into an official download URL + expected SHA-256.
pub async fn resolve_iso(os_id: &str) -> Result<ResolvedIso, AppError> {
    let client = http_client()?;

    let resolved = match os_id {
        "ubuntu" => {
            let text = fetch_text(&client, UBUNTU_INDEX).await?;
            resolve_from_sha256sums(&text, "ubuntu-", "-desktop-amd64.iso", UBUNTU_BASE)?
        }
        "xubuntu" => {
            let text = fetch_text(&client, XUBUNTU_INDEX).await?;
            resolve_from_sha256sums(&text, "xubuntu-", "-desktop-amd64.iso", XUBUNTU_BASE)?
        }
        "lubuntu" => {
            let text = fetch_text(&client, LUBUNTU_INDEX).await?;
            resolve_from_sha256sums(&text, "lubuntu-", "-desktop-amd64.iso", LUBUNTU_BASE)?
        }
        "ubuntu-mate" => {
            let text = fetch_text(&client, UBUNTU_MATE_INDEX).await?;
            resolve_from_sha256sums(&text, "ubuntu-mate-", "-desktop-amd64.iso", UBUNTU_MATE_BASE)?
        }
        "debian" => {
            let text = fetch_text(&client, DEBIAN_INDEX).await?;
            resolve_from_sha256sums(&text, "debian-", "-amd64-netinst.iso", DEBIAN_BASE)?
        }
        "arch" => {
            let text = fetch_text(&client, ARCH_INDEX).await?;
            // Arch publishes a fixed file name per "latest" snapshot.
            let entries = parse_sha256sums(&text);
            let (sha, _) = entries
                .iter()
                .find(|(_, name)| name == ARCH_NAME)
                .ok_or_else(|| AppError::config(format!(
                    "official Arch index did not list {ARCH_NAME} — the layout may have changed"
                )))?;
            ResolvedIso {
                url: format!("{ARCH_BASE}{ARCH_NAME}"),
                sha256: sha.clone(),
                name: ARCH_NAME.to_string(),
            }
        }
        "fedora" => {
            let text = fetch_text(&client, FEDORA_RELEASES_JSON).await?;
            resolve_fedora(&text)?
        }
        other => {
            return Err(AppError::config(format!("unknown OS id: {other}")));
        }
    };

    if resolved.sha256.len() != 64 || !resolved.sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(AppError::config(format!(
            "official index for {os_id} returned a malformed SHA-256"
        )));
    }
    Ok(resolved)
}

/// Pick the newest `prefix<version>suffix` entry from a SHA256SUMS text.
fn resolve_from_sha256sums(
    text: &str,
    prefix: &str,
    suffix: &str,
    base_url: &str,
) -> Result<ResolvedIso, AppError> {
    let entries = parse_sha256sums(text);
    let mut best: Option<(Vec<u64>, String, String)> = None; // (version, hash, name)
    for (hash, name) in &entries {
        if let Some(version) = version_key(name, prefix, suffix) {
            let is_newer = best
                .as_ref()
                .map(|(v, _, _)| version > *v)
                .unwrap_or(true);
            if is_newer {
                best = Some((version, hash.clone(), name.clone()));
            }
        }
    }
    let (_, hash, name) = best.ok_or_else(|| AppError::config(format!(
        "no file matching '{prefix}…{suffix}' was found in the official index — the naming layout may have changed"
    )))?;
    Ok(ResolvedIso {
        url: format!("{base_url}{name}"),
        sha256: hash,
        name,
    })
}

/// Parse a SHA256SUMS-style file into (hash, filename) pairs.
/// Filenames in every supported index contain no spaces; anything with extra
/// columns is ignored as a deliberate conservative choice.
fn parse_sha256sums(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() {
                return None;
            }
            let mut it = line.split_whitespace();
            let hash = it.next()?.to_lowercase();
            let name = it.next()?;
            if hash.len() != 64 {
                return None;
            }
            Some((hash, name.trim_start_matches('*').to_string()))
        })
        .collect()
}

/// Extract the numeric version between `prefix` and `suffix` as a comparable
/// vector. Only pure digit/dot versions count (rejects "edu-13.7.0" etc.).
fn version_key(name: &str, prefix: &str, suffix: &str) -> Option<Vec<u64>> {
    let middle = name.strip_prefix(prefix)?.strip_suffix(suffix)?;
    if middle.is_empty() || !middle.bytes().all(|b| b.is_ascii_digit() || b == b'.') {
        return None;
    }
    let mut parts = Vec::new();
    for seg in middle.split('.') {
        if seg.is_empty() {
            return None;
        }
        parts.push(seg.parse::<u64>().ok()?);
    }
    Some(parts)
}

/// Fedora: pick the newest GA (non-beta) Workstation x86_64 live ISO.
fn resolve_fedora(json: &str) -> Result<ResolvedIso, AppError> {
    let values: serde_json::Value = serde_json::from_str(json)
        .map_err(|e| AppError::config(format!("Fedora releases.json is not valid JSON: {e}")))?;
    let array = values
        .as_array()
        .ok_or_else(|| AppError::config("Fedora releases.json: expected a top-level array"))?;

    let mut best: Option<(u64, String, String, String)> = None; // (version, url, sha, name)
    for item in array {
        let Some(obj) = item.as_object() else { continue };
        let arch = obj.get("arch").and_then(|v| v.as_str()).unwrap_or("");
        let variant = obj.get("variant").and_then(|v| v.as_str()).unwrap_or("");
        let subvariant = obj.get("subvariant").and_then(|v| v.as_str()).unwrap_or("");
        let Some(link) = obj.get("link").and_then(|v| v.as_str()) else {
            continue;
        };
        let Some(sha) = obj.get("sha256").and_then(|v| v.as_str()) else {
            continue;
        };
        if arch != "x86_64" || variant != "Workstation" || subvariant != "Workstation" {
            continue;
        }
        if !link.ends_with(".iso") || sha.len() != 64 {
            continue;
        }
        let raw_version = obj.get("version").and_then(|v| v.as_str()).unwrap_or("");
        let lowered = raw_version.to_lowercase();
        // Skip pre-releases honestly; GA wins as soon as it is published.
        if ["beta", "alpha", "branched", "rawhide"]
            .iter()
            .any(|m| lowered.contains(m))
        {
            continue;
        }
        let major: u64 = raw_version
            .split(|c: char| !c.is_ascii_digit())
            .next()
            .unwrap_or("")
            .parse()
            .unwrap_or(0);
        if major == 0 {
            continue;
        }
        let name = link.rsplit('/').next().unwrap_or(link).to_string();
        let is_newer = best.as_ref().map(|(v, _, _, _)| major > *v).unwrap_or(true);
        if is_newer {
            best = Some((major, link.to_string(), sha.to_lowercase(), name));
        }
    }

    let (_, url, sha, name) = best.ok_or_else(|| AppError::config(
        "no stable Fedora Workstation x86_64 ISO is listed right now — try again after the next release",
    ))?;
    Ok(ResolvedIso {
        url,
        sha256: sha,
        name,
    })
}

// ---------------------------------------------------------------------------
// Download engine
// ---------------------------------------------------------------------------

/// Full flow: resolve → (cache check) → stream to `.part` → verify → rename.
pub async fn run_download(
    entry: &OsEntry,
    cache_dir: &Path,
    cancel: CancelToken,
    events: Arc<dyn DownloadEvents>,
) -> Result<DownloadOutcome, AppError> {
    std::fs::create_dir_all(cache_dir).map_err(|e| {
        AppError::Io {
            message: format!("cannot create download directory {}: {e}", cache_dir.display()),
            hint: Some("Check disk space and folder permissions.".to_string()),
        }
    })?;

    // 1. Resolve against the official index.
    events.status("resolving");
    events.log(format!("resolving {} from the official index…", entry.id));
    let resolved = resolve_iso(entry.id).await?;
    events.log(format!(
        "official release: {} (sha256 {}…{})",
        resolved.name,
        &resolved.sha256[..8],
        &resolved.sha256[resolved.sha256.len() - 8..]
    ));

    let final_path = cache_dir.join(&resolved.name);
    let part_path = cache_dir.join(format!("{}.part", resolved.name));

    // 2. Cache: reuse only a byte-verified copy.
    if final_path.exists() {
        events.status("verifying");
        events.log("found a cached copy — verifying it…".to_string());
        let path_for_hash = final_path.clone();
        let cached_hash = tokio::task::spawn_blocking(move || hash_file(&path_for_hash))
            .await
            .map_err(|e| AppError::platform(format!("cache verify task crashed: {e}")))?
            .ok_or_else(|| AppError::image("cached file could not be read for verification"))?;
        if cached_hash == resolved.sha256 {
            let size = std::fs::metadata(&final_path).map(|m| m.len()).unwrap_or(0);
            events.log("cached copy is valid — no download needed ✓".to_string());
            events.status("done");
            emit_progress(&*events, &DownloadProgress {
                downloaded: size,
                total: Some(size),
                speed_mbs: 0.0,
                eta_secs: Some(0.0),
                percent: Some(100.0),
            });
            return Ok(DownloadOutcome {
                path: final_path.to_string_lossy().into_owned(),
                name: resolved.name,
                size,
                sha256: cached_hash,
                verified: true,
                cached: true,
            });
        }
        events.log("cached copy does not match the official hash — redownloading".to_string());
        let _ = std::fs::remove_file(&final_path);
    }

    // 3. Stream to .part with on-the-fly hashing.
    events.status("downloading");
    events.log(format!("downloading from {}", resolved.url));
    let client = http_client()?;
    let response = client
        .get(&resolved.url)
        .send()
        .await
        .map_err(|e| network_error(&resolved.url, &e))?;
    if !response.status().is_success() {
        return Err(AppError::Io {
            message: format!(
                "download server answered HTTP {} for {}",
                response.status(),
                resolved.url
            ),
            hint: Some(
                "The official mirror may be busy or the release may have moved. Retry in a moment."
                    .to_string(),
            ),
        });
    }
    let total = response.content_length();

    // Reject absurd content lengths (> 32 GiB) as a sanity guard.
    if let Some(n) = total {
        if n > 32 * 1024 * 1024 * 1024 {
            return Err(AppError::config(format!(
                "reported download size is implausible: {n} bytes"
            )));
        }
    }

    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&part_path)
        .map_err(|e| AppError::Io {
            message: format!("cannot create {}: {e}", part_path.display()),
            hint: Some("Check free disk space (the ISO needs several GiB).".to_string()),
        })?;

    let mut hasher = Sha256::new();
    let mut downloaded: u64 = 0;
    let mut last_emit = Instant::now() - EMIT_INTERVAL;
    let mut last_bytes: u64 = 0;
    let mut last_instant = Instant::now();
    let mut response = response;

    loop {
        if cancel.is_cancelled() {
            drop(file);
            let _ = std::fs::remove_file(&part_path);
            return Err(AppError::Cancelled);
        }
        let chunk = tokio::time::timeout(CHUNK_TIMEOUT, response.chunk())
            .await
            .map_err(|_| AppError::Io {
                message: format!("connection stalled for over {} s: {}", CHUNK_TIMEOUT.as_secs(), resolved.url),
                hint: Some("The mirror stopped sending data. Retry the download.".to_string()),
            })?
            .map_err(|e| network_error(&resolved.url, &e))?;

        let Some(bytes) = chunk else { break };
        file.write_all(&bytes).map_err(|e| AppError::Io {
            message: format!("write failed at {} bytes: {e}", downloaded),
            hint: Some("The disk is probably full — free up space and retry.".to_string()),
        })?;
        hasher.update(&bytes);
        downloaded += bytes.len() as u64;

        if last_emit.elapsed() >= EMIT_INTERVAL {
            let elapsed = last_instant.elapsed().as_secs_f64();
            let speed = if elapsed > 0.05 {
                (downloaded - last_bytes) as f64 / 1_048_576.0 / elapsed
            } else {
                0.0
            };
            emit_progress(&*events, &progress_payload(downloaded, total, speed));
            last_emit = Instant::now();
            last_bytes = downloaded;
            last_instant = Instant::now();
        }
    }

    // Make sure the last progress sample shows 100% before verifying.
    emit_progress(&*events, &progress_payload(downloaded, total, 0.0));
    events.log(format!("downloaded {} bytes", downloaded));

    file.sync_all().map_err(|e| AppError::Io {
        message: format!("cannot flush downloaded file: {e}"),
        hint: None,
    })?;
    drop(file);

    // 4. Verify against the official hash, then publish atomically.
    events.status("verifying");
    events.log("verifying SHA-256 against the official index…".to_string());
    let part_for_hash = part_path.clone();
    let actual = tokio::task::spawn_blocking(move || hash_file(&part_for_hash))
        .await
        .map_err(|e| AppError::platform(format!("verify task crashed: {e}")))?
        .ok_or_else(|| AppError::image("downloaded file vanished before verification"))?;

    if actual != resolved.sha256 {
        let _ = std::fs::remove_file(&part_path);
        return Err(AppError::VerifyMismatch {
            expected: resolved.sha256,
            actual,
        });
    }

    std::fs::rename(&part_path, &final_path).map_err(|e| AppError::Io {
        message: format!("cannot finalize {}: {e}", final_path.display()),
        hint: None,
    })?;

    events.log("SHA-256 verified ✓".to_string());
    events.status("done");
    Ok(DownloadOutcome {
        path: final_path.to_string_lossy().into_owned(),
        name: resolved.name,
        size: downloaded,
        sha256: actual,
        verified: true,
        cached: false,
    })
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn http_client() -> Result<reqwest::Client, AppError> {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| AppError::config(format!("cannot build HTTP client: {e}")))
}

async fn fetch_text(client: &reqwest::Client, url: &str) -> Result<String, AppError> {
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| network_error(url, &e))?;
    if !response.status().is_success() {
        return Err(AppError::Io {
            message: format!("official index answered HTTP {}: {url}", response.status()),
            hint: Some(
                "The official index is temporarily unreachable. Check your connection and retry."
                    .to_string(),
            ),
        });
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|e| network_error(url, &e))?;
    if bytes.len() > INDEX_MAX_BYTES {
        return Err(AppError::config(format!(
            "official index at {url} is unexpectedly large"
        )));
    }
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn network_error(url: &str, e: &reqwest::Error) -> AppError {
    AppError::Io {
        message: format!("network error on {url}: {e}"),
        hint: Some(
            "Check your internet connection. The ISO is fetched from the official project servers."
                .to_string(),
        ),
    }
}

fn progress_payload(downloaded: u64, total: Option<u64>, speed_mbs: f64) -> DownloadProgress {
    let percent = total.map(|t| {
        if t == 0 {
            100.0
        } else {
            (downloaded as f64 / t as f64) * 100.0
        }
    });
    let eta_secs = match (total, speed_mbs) {
        (Some(t), s) if s > 0.001 => Some((t.saturating_sub(downloaded) as f64 / 1_048_576.0) / s),
        _ => None,
    };
    DownloadProgress {
        downloaded,
        total,
        speed_mbs,
        eta_secs,
        percent,
    }
}

fn emit_progress(events: &dyn DownloadEvents, p: &DownloadProgress) {
    events.progress(p);
}

/// Blocking SHA-256 of a whole file; `None` when the file cannot be read.
fn hash_file(path: &PathBuf) -> Option<String> {
    let mut file = File::open(path).ok()?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1024 * 1024];
    loop {
        let n = file.read(&mut buf).ok()?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let digest = hasher.finalize();
    Some(digest.iter().map(|b| format!("{b:02x}")).collect())
}
