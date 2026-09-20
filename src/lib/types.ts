/**
 * Types mirroring the Rust IPC contract (src-tauri). Keep in sync with:
 *   - core/mod.rs      (Phase + payloads)
 *   - core/pipeline.rs (PipelineResult)
 *   - safety.rs        (DriveInfo)
 *   - commands/mod.rs  (DetectionInfo, Settings)
 */

export interface DriveInfo {
  id: string;
  path: string;
  model: string;
  vendor: string;
  size_bytes: number;
  removable: boolean;
  bus: string;
  serial: string;
  is_system: boolean;
}

export type ImageKind = "raw" | "gzip" | "xz" | "zstd" | "bzip2";

export interface DetectionInfo {
  kind: ImageKind;
  compressed: boolean;
  is_windows_iso: boolean;
  file_size: number;
  estimated_size: number | null;
}

export type Phase = "writing" | "verifying" | "finalizing" | "done";

export interface ProgressData {
  written: number;
  total: number | null;
  speed_mbs: number;
  eta_secs: number | null;
  percent: number | null;
}

export interface VerifyData {
  checked: number;
  total: number;
  percent: number;
}

export interface DoneData {
  total_bytes: number;
  elapsed_secs: number;
  avg_speed_mbs: number;
  peak_speed_mbs: number;
  verified: boolean;
  sha256: string;
}

export interface ErrorData {
  message: string;
  hint: string | null;
}

export type Theme = "midnight" | "catppuccin-mocha" | "tokyo-night";
export type Lang = "en" | "es";

export interface Settings {
  unsafe_mode: boolean;
  theme: Theme;
  language: Lang | null;
}

export interface SelectedImage {
  path: string;
  name: string;
  size: number;
  detection: DetectionInfo;
}

export const THEMES: Theme[] = ["midnight", "catppuccin-mocha", "tokyo-night"];
