# Changelog

All notable changes to AnimaBooter will be documented in this file.

The format is inspired by [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to the history of its public releases — entries are
added as changes are made, not reconstructed afterwards.

## [Unreleased]

### Changed

### Fixed

## [0.1.0-alpha] - 2026-09-25

### Added

- Initial public documentation set: bilingual README, contributing guide,
  security policy, changelog.
- Flashing pipeline with reader/writer/verifier stages, live progress and
  cancellation (`flash`, `cancel_flash`, `list_drives`, `eject`).
- Image detection by magic bytes with gzip/xz/zstd/bzip2 decompression.
- Wizard UI with reactive mascot, drive list, confirmation modal and
  per-drive safety checks (root/system disk rejection).
- Bilingual interface (EN/ES), three themes, Tauri 2 capabilities kept to
  the minimum (`core`, `dialog`, `notification`, `opener`).

### Changed

- Tauri `beforeDevCommand`/`beforeBuildCommand` now use pnpm, matching the
  package manager actually used by the project.
