/**
 * Typed IPC layer — the single place that talks to the Rust backend.
 *
 * Contract (keep in sync with src-tauri/src/commands):
 *   list_drives()                       -> DriveInfo[]
 *   detect_image(image_path)            -> DetectionInfo
 *   flash(image_path, drive_path, verify) -> DoneData-shaped PipelineResult
 *   cancel_flash()                      -> void
 *   eject(drive_path)                   -> void
 *   get_settings()                      -> Settings
 *   set_settings(settings)              -> void
 *   restart_as_admin()                  -> void   (Windows only)
 *
 * Events (app.emit):
 *   flash://phase    { phase: writing|verifying|finalizing|done }
 *   flash://progress { written, total, speed_mbs, eta_secs, percent }
 *   flash://verify   { checked, total, percent }
 *   flash://log      { line }
 *   flash://done     { total_bytes, elapsed_secs, avg_speed_mbs, peak_speed_mbs, verified, sha256 }
 *   flash://error    { message, hint }
 */
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  DetectionInfo,
  DoneData,
  DriveInfo,
  ErrorData,
  Phase,
  ProgressData,
  Settings,
  VerifyData,
} from "./types";

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

export async function listDrives(): Promise<DriveInfo[]> {
  return invoke<DriveInfo[]>("list_drives");
}

export async function detectImage(imagePath: string): Promise<DetectionInfo> {
  return invoke<DetectionInfo>("detect_image", { imagePath });
}

export async function startFlash(
  imagePath: string,
  drivePath: string,
  verify: boolean,
): Promise<DoneData> {
  return invoke<DoneData>("flash", { imagePath, drivePath, verify });
}

export async function cancelFlash(): Promise<void> {
  return invoke("cancel_flash");
}

export async function ejectDrive(drivePath: string): Promise<void> {
  return invoke("eject", { drivePath });
}

export async function getSettings(): Promise<Settings> {
  return invoke<Settings>("get_settings");
}

export async function setSettings(settings: Settings): Promise<void> {
  return invoke("set_settings", { settings });
}

export async function restartAsAdmin(): Promise<void> {
  return invoke("restart_as_admin");
}

// ---------------------------------------------------------------------------
// Events — every function returns its own unlisten handle.
// ---------------------------------------------------------------------------

export function onPhase(cb: (phase: Phase) => void): Promise<UnlistenFn> {
  // The backend emits the Phase enum directly as a JSON string
  // ("writing" | "verifying" | "finalizing" | "done"), not a wrapped object.
  return listen<Phase>("flash://phase", (e) => cb(e.payload));
}

export function onProgress(cb: (p: ProgressData) => void): Promise<UnlistenFn> {
  return listen<ProgressData>("flash://progress", (e) => cb(e.payload));
}

export function onVerify(cb: (p: VerifyData) => void): Promise<UnlistenFn> {
  return listen<VerifyData>("flash://verify", (e) => cb(e.payload));
}

export function onLog(cb: (line: string) => void): Promise<UnlistenFn> {
  return listen<{ line: string }>("flash://log", (e) => cb(e.payload.line));
}

export function onDone(cb: (d: DoneData) => void): Promise<UnlistenFn> {
  return listen<DoneData>("flash://done", (e) => cb(e.payload));
}

export function onError(cb: (e: ErrorData) => void): Promise<UnlistenFn> {
  return listen<ErrorData>("flash://error", (e) => cb(e.payload));
}

/** Normalize thrown IPC errors into the { message, hint } shape. */
export function toErrorData(err: unknown): ErrorData {
  if (err && typeof err === "object" && "message" in err) {
    const e = err as { message?: unknown; hint?: unknown };
    return {
      message: typeof e.message === "string" ? e.message : String(err),
      hint: typeof e.hint === "string" ? e.hint : null,
    };
  }
  return { message: String(err), hint: null };
}
