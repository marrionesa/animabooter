/**
 * Global client state (Svelte stores) + i18n + theme application.
 */
import { derived, get, writable } from "svelte/store";
import en from "./i18n/en.json";
import es from "./i18n/es.json";
import { startFlash, toErrorData } from "./ipc";
import type {
  DoneData,
  DriveInfo,
  ErrorData,
  Lang,
  Phase,
  ProgressData,
  SelectedImage,
  Settings,
  Theme,
  VerifyData,
} from "./types";

// ---------------------------------------------------------------------------
// Wizard state
// ---------------------------------------------------------------------------

export type Step = "image" | "drive" | "confirm" | "flash" | "result";

export const step = writable<Step>("image");
export const drives = writable<DriveInfo[]>([]);
export const loadingDrives = writable(false);
export const drivesError = writable<string | null>(null);
export const selectedImage = writable<SelectedImage | null>(null);
export const selectedDrive = writable<DriveInfo | null>(null);
export const verifyEnabled = writable(true);

// Flash runtime state
export const flashStatus = writable<"idle" | "running" | "error" | "done">("idle");
export const currentPhase = writable<Phase>("writing");
export const progress = writable<ProgressData | null>(null);
export const verifyProgress = writable<VerifyData | null>(null);
export const logLines = writable<string[]>([]);
export const flashError = writable<ErrorData | null>(null);
export const result = writable<DoneData | null>(null);
export const cancelPrompt = writable(false);
export const toast = writable<{ kind: "ok" | "err"; text: string } | null>(null);

export function resetFlashState(): void {
  flashStatus.set("idle");
  currentPhase.set("writing");
  progress.set(null);
  verifyProgress.set(null);
  logLines.set([]);
  flashError.set(null);
  result.set(null);
  cancelPrompt.set(false);
}

export function showToast(kind: "ok" | "err", text: string): void {
  toast.set({ kind, text });
  window.setTimeout(() => toast.set(null), 3200);
}

/** Run the flash command and mirror its outcome into the wizard state. */
export async function startFlashAndRun(): Promise<void> {
  const image = get(selectedImage);
  const drive = get(selectedDrive);
  if (!image || !drive) return;

  flashStatus.set("running");
  currentPhase.set("writing");
  progress.set(null);
  verifyProgress.set(null);
  logLines.set([]);
  flashError.set(null);
  result.set(null);
  cancelPrompt.set(false);
  step.set("flash");

  try {
    const done = await startFlash(image.path, drive.path, get(verifyEnabled));
    // The flash://done event usually lands first; only fall back to the
    // command result so we never render twice.
    if (!get(result)) result.set(done);
    flashStatus.set("done");
    step.set("result");
  } catch (e) {
    const err = toErrorData(e);
    if (!get(flashError)) flashError.set(err);
    flashStatus.set("error");
  }
}

// ---------------------------------------------------------------------------
// Settings + theme
// ---------------------------------------------------------------------------

export const settings = writable<Settings>({
  unsafe_mode: false,
  theme: "midnight",
  language: null,
});

export function applyTheme(theme: Theme): void {
  document.documentElement.dataset.theme = theme;
}

settings.subscribe((s) => {
  if (typeof document !== "undefined") applyTheme(s.theme);
});

// ---------------------------------------------------------------------------
// i18n — plain JSON dictionaries, auto-detected from the system locale.
// ---------------------------------------------------------------------------

const dictionaries: Record<Lang, Record<string, string>> = { en, es };

function detectLang(): Lang {
  const langs = typeof navigator !== "undefined" ? navigator.languages ?? [navigator.language] : [];
  for (const candidate of langs) {
    if (candidate && candidate.toLowerCase().startsWith("es")) return "es";
  }
  return "en";
}

export const lang = writable<Lang>(detectLang());

// A persisted language preference always wins over detection.
settings.subscribe((s) => {
  if (s.language) lang.set(s.language);
});

export type Translate = (key: string, vars?: Record<string, string | number>) => string;

export const t = derived(lang, (current): Translate => {
  return (key, vars) => {
    let text = dictionaries[current][key] ?? dictionaries.en[key] ?? key;
    if (vars) {
      for (const [name, value] of Object.entries(vars)) {
        text = text.replaceAll(`{${name}}`, String(value));
      }
    }
    return text;
  };
});
