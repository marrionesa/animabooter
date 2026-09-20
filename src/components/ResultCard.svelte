<script lang="ts">
  /**
   * Step 5 — the shareable ResultCard (neofetch-style): ASCII Anima on the
   * left, JetBrains Mono stats on the right. Copy summary + Export PNG
   * (rendered locally on a canvas — no external services, ever).
   */
  import { get } from "svelte/store";
  import { fly } from "svelte/transition";
  import { CheckCircle2, Copy, Download, PlugZap, RotateCcw } from "lucide-svelte";
  import Mascot from "./Mascot.svelte";
  import { ejectDrive, toErrorData } from "../lib/ipc";
  import {
    result,
    selectedDrive,
    selectedImage,
    showToast,
    step,
    t,
  } from "../lib/stores";
  import { formatBytes, formatDuration, formatSpeed, shortHash, timestampLabel } from "../lib/format";

  let copied = $state(false);
  let ejecting = $state(false);
  let ejected = $state(false);

  const REPOSITORY_URL = "https://github.com/marrionesa/animabooter";
  const AUTHOR_URL = "https://github.com/marrionesa";

  const ASCII = [
    "      ╭──────╮",
    "     ╱  ◕  ◕  ╲",
    "    │    ▽    │",
    "    │  ╰──╯  │",
    "     ╲ ▁▁▁▁▁ ╱",
    "      ╰╮────╭╯",
    "        ～～",
  ];

  async function copySummary(): Promise<void> {
    const r = $result;
    if (!r) return;
    const tt = get(t);
    const lines = [
      "animabooter@usb",
      "───────────────",
      `${tt("result.image")}: ${$selectedImage?.name ?? "?"}`,
      `${tt("result.size")}: ${formatBytes(r.total_bytes)}`,
      `${tt("result.time")}: ${formatDuration(r.elapsed_secs)}`,
      `${tt("result.avg")}: ${formatSpeed(r.avg_speed_mbs)}`,
      `${tt("result.peak")}: ${formatSpeed(r.peak_speed_mbs)}`,
      r.verified
        ? `${tt("result.verified")} (${tt("result.sha256")}: ${shortHash(r.sha256)})`
        : tt("result.unverified"),
      `${tt("result.when")}: ${timestampLabel()}`,
      r.verified ? tt("result.zeroCorrupt") : "",
      "AnimaBooter v0.1 · open source · 2026",
      `Repository: ${REPOSITORY_URL}`,
      `Created by marrionesa: ${AUTHOR_URL}`,
    ].filter(Boolean);
    try {
      await navigator.clipboard.writeText(lines.join("\n"));
      copied = true;
      window.setTimeout(() => (copied = false), 2200);
    } catch {
      showToast("err", "clipboard unavailable");
    }
  }

  /** Render the neofetch card into a PNG entirely client-side. */
  async function exportPng(): Promise<void> {
    const r = $result;
    if (!r) return;
    const tt = get(t);
    const W = 760;
    const H = 420;
    const canvas = document.createElement("canvas");
    canvas.width = W;
    canvas.height = H;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    // background card
    ctx.fillStyle = "#1a1d29";
    ctx.fillRect(0, 0, W, H);
    ctx.strokeStyle = "#2a2e3f";
    ctx.lineWidth = 2;
    ctx.strokeRect(1, 1, W - 2, H - 2);

    // header
    ctx.fillStyle = "#8b5cf6";
    ctx.font = "bold 26px 'JetBrains Mono', monospace";
    ctx.fillText("animabooter@usb", 260, 56);
    ctx.fillStyle = "#2a2e3f";
    ctx.fillRect(260, 70, 470, 2);

    // ascii mascot (left column)
    ctx.font = "13px 'JetBrains Mono', monospace";
    ctx.fillStyle = "#8b5cf6";
    ASCII.forEach((line, i) => ctx.fillText(line, 36, 120 + i * 20));
    ctx.fillStyle = "#9ca3af";
    ctx.fillText("  Anima says hi ✦", 36, 120 + ASCII.length * 20 + 8);

    // stats (right column)
    const rows: Array<[string, string]> = [
      [tt("result.image"), $selectedImage?.name ?? "?"],
      [tt("result.size"), formatBytes(r.total_bytes)],
      [tt("result.time"), formatDuration(r.elapsed_secs)],
      [tt("result.avg"), formatSpeed(r.avg_speed_mbs)],
      [tt("result.peak"), formatSpeed(r.peak_speed_mbs)],
      [
        "verified",
        r.verified ? `OK ✓ (sha256: ${shortHash(r.sha256)})` : tt("result.unverified"),
      ],
      [tt("result.when"), timestampLabel()],
    ];
    ctx.font = "14px 'JetBrains Mono', monospace";
    rows.forEach(([key, value], i) => {
      const y = 110 + i * 30;
      ctx.fillStyle = "#9ca3af";
      ctx.fillText(`${key}:`, 260, y);
      ctx.fillStyle = r.verified && key === "verified" ? "#34d399" : "#e5e7eb";
      ctx.fillText(value.length > 34 ? value.slice(0, 33) + "…" : value, 400, y);
    });

    if (r.verified) {
      ctx.fillStyle = "#34d399";
      ctx.font = "bold 15px 'JetBrains Mono', monospace";
      ctx.fillText(tt("result.zeroCorrupt"), 260, 110 + rows.length * 30 + 14);
    }

    // footer
    ctx.fillStyle = "#9ca3af";
    ctx.font = "12px 'JetBrains Mono', monospace";
    ctx.fillText("animabooter v0.1 · open source · 2026", 260, H - 24);

    const blob = await new Promise<Blob | null>((resolve) => canvas.toBlob(resolve, "image/png"));
    if (!blob) {
      showToast("err", "PNG export failed");
      return;
    }
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = "animabooter-summary.png";
    a.click();
    URL.revokeObjectURL(url);
  }

  async function eject(): Promise<void> {
    const drive = $selectedDrive;
    if (!drive || ejecting) return;
    ejecting = true;
    try {
      await ejectDrive(drive.path);
      ejected = true;
      showToast("ok", get(t)("common.ejected"));
    } catch (e) {
      showToast("err", toErrorData(e).message);
    } finally {
      ejecting = false;
    }
  }

  function flashAnother(): void {
    selectedImage.set(null);
    selectedDrive.set(null);
    result.set(null);
    step.set("image");
  }
</script>

<div class="mx-auto w-full max-w-2xl" in:fly={{ y: 10, duration: 220 }}>
  <div class="mb-4 flex items-center justify-center gap-2 text-success">
    <CheckCircle2 size={20} aria-hidden="true" />
    <h2 class="text-lg font-semibold">{$t("progress.done")} — {$t("result.title")}</h2>
  </div>

  <!-- neofetch card -->
  <div class="overflow-hidden rounded-2xl border border-edge bg-surface shadow-xl">
    <div class="grid gap-0 sm:grid-cols-[220px_1fr]">
      <div class="hidden place-items-center border-r border-edge bg-terminal p-4 sm:grid">
        <Mascot state="done" size={128} />
      </div>
      <div class="terminal p-5 leading-relaxed">
        <p class="font-bold text-accent"><a href={REPOSITORY_URL} target="_blank" rel="noreferrer" class="hover:underline">animabooter@usb</a></p>
        <p class="mb-2 text-edge">─────────────</p>
        <p><span class="text-muted">{$t("result.image")}:</span> <span class="text-txt">{$selectedImage?.name ?? "?"}</span></p>
        <p><span class="text-muted">{$t("result.size")}:</span> <span class="text-txt">{formatBytes($result?.total_bytes)}</span></p>
        <p><span class="text-muted">{$t("result.time")}:</span> <span class="text-txt">{formatDuration($result?.elapsed_secs ?? NaN)}</span></p>
        <p><span class="text-muted">{$t("result.avg")}:</span> <span class="text-txt">{formatSpeed($result?.avg_speed_mbs ?? NaN)}</span></p>
        <p><span class="text-muted">{$t("result.peak")}:</span> <span class="text-txt">{formatSpeed($result?.peak_speed_mbs ?? NaN)}</span></p>
        <p>
          <span class="text-muted">verified:</span>
          {#if $result?.verified}
            <span class="text-success">OK ✓ ({shortHash($result.sha256)})</span>
          {:else}
            <span class="text-warning">{$t("result.unverified")}</span>
          {/if}
        </p>
        {#if $result?.verified}
          <p class="text-success">{$t("result.zeroCorrupt")}</p>
        {/if}
      </div>
    </div>
  </div>

  <!-- actions -->
  <div class="mt-4 flex flex-wrap justify-center gap-3">
    <button
      type="button"
      class="inline-flex min-h-11 items-center gap-2 rounded-xl bg-accent px-4 py-2.5 text-sm font-semibold text-white transition-transform hover:scale-[1.02]"
      onclick={copySummary}
    >
      {#if copied}<CheckCircle2 size={16} aria-hidden="true" />{:else}<Copy size={16} aria-hidden="true" />{/if}
      {copied ? $t("common.copied") : $t("common.copy")}
    </button>
    <button
      type="button"
      class="inline-flex min-h-11 items-center gap-2 rounded-xl border border-edge bg-surface px-4 py-2.5 text-sm font-semibold text-txt transition-colors hover:bg-surface2"
      onclick={exportPng}
    >
      <Download size={16} aria-hidden="true" />
      {$t("result.exportPng")}
    </button>
    <button
      type="button"
      class="inline-flex min-h-11 items-center gap-2 rounded-xl border border-edge bg-surface px-4 py-2.5 text-sm font-semibold text-txt transition-colors hover:bg-surface2 disabled:opacity-50"
      onclick={eject}
      disabled={ejecting || ejected || !$selectedDrive}
    >
      <PlugZap size={16} aria-hidden="true" />
      {ejected ? $t("common.ejected") : ejecting ? $t("common.ejecting") : $t("common.eject")}
    </button>
    <button
      type="button"
      class="inline-flex min-h-11 items-center gap-2 rounded-xl border border-edge bg-surface px-4 py-2.5 text-sm font-semibold text-muted transition-colors hover:bg-surface2 hover:text-txt"
      onclick={flashAnother}
    >
      <RotateCcw size={16} aria-hidden="true" />
      {$t("common.flashAnother")}
    </button>
  </div>
</div>
