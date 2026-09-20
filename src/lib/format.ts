/** Human formatting helpers. All sizes are binary units (KiB/MiB/GiB). */

const KIB = 1024;
const MIB = 1024 * KIB;
const GIB = 1024 * MIB;

export function formatBytes(bytes: number | null | undefined): string {
  if (bytes == null || Number.isNaN(bytes)) return "—";
  if (bytes < KIB) return `${bytes} B`;
  if (bytes < MIB) return `${(bytes / KIB).toFixed(1)} KiB`;
  if (bytes < GIB) return `${(bytes / MIB).toFixed(1)} MiB`;
  return `${(bytes / GIB).toFixed(2)} GiB`;
}

export function formatSpeed(mbs: number): string {
  if (!Number.isFinite(mbs)) return "—";
  if (mbs < 1) return `${(mbs * KIB).toFixed(0)} KiB/s`;
  return `${mbs.toFixed(1)} MiB/s`;
}

export function formatDuration(secs: number): string {
  if (!Number.isFinite(secs) || secs < 0) return "—";
  if (secs < 1) return `${Math.round(secs * 1000)} ms`;
  if (secs < 60) return `${secs.toFixed(1)} s`;
  const m = Math.floor(secs / 60);
  const s = Math.round(secs % 60);
  if (m < 60) return `${m}m ${s.toString().padStart(2, "0")}s`;
  const h = Math.floor(m / 60);
  return `${h}h ${(m % 60).toString().padStart(2, "0")}m`;
}

export function formatEta(secs: number | null): string {
  if (secs == null || !Number.isFinite(secs)) return "—";
  if (secs <= 0) return "0s";
  if (secs < 60) return `${Math.ceil(secs)}s`;
  return `${Math.floor(secs / 60)}m ${Math.ceil(secs % 60)}s`;
}

export function formatPercent(percent: number | null): string {
  if (percent == null || !Number.isFinite(percent)) return "";
  return `${Math.min(100, Math.max(0, percent)).toFixed(0)}%`;
}

/** "ab12…ef90" from a 64-char hex digest. */
export function shortHash(sha: string): string {
  if (!sha || sha.length < 12) return sha || "—";
  return `${sha.slice(0, 6)}…${sha.slice(-4)}`;
}

export function timestampLabel(): string {
  const d = new Date();
  const pad = (n: number) => n.toString().padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
}
