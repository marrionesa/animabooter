# Changelog

All notable changes to AnimaBooter will be documented in this file.

The format is inspired by [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to the history of its public releases — entries are
added as changes are made, not reconstructed afterwards.

## [Unreleased]

### Changed

### Fixed

## [0.1.0] - 2026-09-25

Verification status at release: Linux is hardware-tested by the author
(real flash + boot verified); Windows and macOS are CI-built but not yet
hardware-tested. Binaries are unsigned (SmartScreen / Gatekeeper will
warn on first run).

### Added

- Flashing pipeline with three parallel stages (reader → writer → verifier),
  live progress with speed/ETA, cooperative cancellation and an honest
  durability guarantee: `sync_all` must succeed before success is reported.
- Image detection by magic bytes (`detect_file`) with transparent
  decompression of gzip, xz, zstd and bzip2 images.
- Per-drive safety checks: root/system and internal disks are rejected unless
  unsafe mode is explicitly enabled; destructive opens are audited.
- Tauri 2 IPC commands: `list_drives`, `flash`, `cancel_flash`, `eject`,
  `detect_image`, settings and `restart_as_admin` (Windows UAC elevation).
- Wizard UI (Svelte 5 + Tailwind 4): drive list, image dropzone, confirmation
  modal, progress view with reactive mascot and result card; three themes and
  a bilingual interface (EN/ES).
- Cross-platform device backends:
  - Linux via udev, writes through the block device.
  - macOS via `diskutil list -plist` / `info -plist`, writes to the raw
    `/dev/rdiskN` character device (10–20x faster than `/dev/diskN`).
  - Windows via Win32 volume handles with lock/dismount IOCTLs, physical
    drive geometry probing and media ejection.
- Initial public documentation set: bilingual README, contributing guide,
  security policy, changelog, MIT license.
- CI workflows for Linux (fmt, clippy, tests, svelte-check, build), Windows
  and macOS, plus a tagged release workflow (all manual by design for now).

### Changed

- Tauri `beforeDevCommand`/`beforeBuildCommand` now use pnpm, matching the
  package manager actually used by the project.
- Committed `pnpm-lock.yaml` so dependency resolution is reproducible.

### Fixed

- Satisfied `cargo clippy -D warnings` in the flashing pipeline and unix
  platform code (type aliases for complex result types, `while let` loops,
  `Option::map`).
- macOS backend: `plist::Value::as_dictionary` is the correct API name (the
  backend had never been compiled on macOS), `Value::from_reader` is fed a
  `Cursor` (`&[u8]` does not implement `Seek`), and a duplicate
  `open_unix_pair` import was removed.
- Windows backend: updated Win32 import paths to the `windows` 0.58 layout
  (`OpenProcessToken` moved to `System::Threading`, `GENERIC_*` to
  `Foundation`, `FSCTL_*`/`IOCTL_STORAGE_*` to `System::Ioctl`), fixed
  closure mutability in the read-back reader and removed an unused
  re-export of `restart_as_admin`.
