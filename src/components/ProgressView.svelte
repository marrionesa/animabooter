<script lang="ts">
  /**
   * Step 4 — live progress: phase mascot, big percent (or honest byte
   * counters for compressed streams), window speed + running peak, ETA,
   * cancellable (with its own confirmation) and a terminal-style log.
   */
  import { get } from "svelte/store";
  import { fade } from "svelte/transition";
  import { ChevronDown, ChevronUp, Gauge, Timer, Zap } from "lucide-svelte";
  import Mascot from "./Mascot.svelte";
  import ConfirmModal from "./ConfirmModal.svelte";
  import { cancelFlash, toErrorData } from "../lib/ipc";
  import {
    cancelPrompt,
    currentPhase,
    flashError,
    flashStatus,
    logLines,
    progress,
    step,
    t,
    toast,
    verifyProgress,
  } from "../lib/stores";
  import { formatBytes, formatEta, formatPercent, formatSpeed } from "../lib/format";
  import type { MascotState } from "./Mascot.svelte";

  let showLog = $state(false);
  let peakSeen = $state(0);
  let logEl = $state<HTMLElement | null>(null);

  const phaseLabel = {
    writing: () => get(t)("progress.writing"),
    verifying: () => get(t)("progress.verifying"),
    finalizing: () => get(t)("progress.finalizing"),
    done: () => get(t)("progress.done"),
  } as const;

  const mascotState = $derived<MascotState>(
    $flashStatus === "error"
      ? "error"
      : $currentPhase === "finalizing" || $currentPhase === "done"
        ? "done"
        : $currentPhase === "verifying"
          ? "verifying"
          : "writing"
  );

  // Track the running peak client-side (the contract reports window speed).
  $effect(() => {
    const p = $progress;
    if (p && p.speed_mbs > peakSeen) peakSeen = p.speed_mbs;
  });

  // Auto-scroll the terminal as lines arrive.
  $effect(() => {
    void $logLines.length;
    if (showLog && logEl) logEl.scrollTop = logEl.scrollHeight;
  });

  async function doCancel(): Promise<void> {
    cancelPrompt.set(false);
    try {
      await cancelFlash();
    } catch (e) {
      toast.set({ kind: "err", text: toErrorData(e).message });
    }
  }
</script>

<div class="mx-auto w-full max-w-xl" in:fade={{ duration: 180 }}>
  <div class="grid place-items-center gap-2">
    <Mascot state={mascotState} size={150} />
    <h2 class="text-lg font-semibold text-txt">
      {$flashStatus === "error" ? get(t)("error.title") : phaseLabel[$currentPhase]()}
    </h2>
  </div>

  {#if $flashStatus === "error" && $flashError}
    <!-- error panel -->
    <div class="mt-4 rounded-2xl border border-error/40 bg-error/10 p-4" role="alert">
      <p class="text-sm font-semibold text-error">{$flashError.message}</p>
      {#if $flashError.hint}
        <p class="mt-2 text-xs text-muted">
          <span class="font-semibold text-txt">{$t("error.hint")}:</span>
          {$flashError.hint}
        </p>
      {/if}
      <div class="mt-4 flex justify-end gap-3">
        <button
          type="button"
          class="min-h-11 rounded-xl border border-edge px-4 py-2 text-sm font-medium text-muted transition-colors hover:bg-surface2 hover:text-txt"
          onclick={() => {
            flashError.set(null);
            flashStatus.set("idle");
            step.set("drive");
          }}
        >
          {$t("error.backToDrives")}
        </button>
      </div>
    </div>
  {:else}
    <!-- big percent / written bytes -->
    <div class="mt-3 text-center">
      {#if $progress && $progress.percent != null}
        <p class="font-mono text-5xl font-bold text-accent">{formatPercent($progress.percent)}</p>
        {#if $progress.total}
          <p class="mt-1 text-xs text-muted">
            {formatBytes($progress.written)} {$t("progress.of", { total: formatBytes($progress.total) })}
          </p>
        {/if}
      {:else if $progress}
        <p class="font-mono text-4xl font-bold text-accent">{formatBytes($progress.written)}</p>
        <p class="mt-1 text-xs text-muted">{$t("progress.unknownSize")}</p>
      {:else}
        <p class="font-mono text-4xl font-bold text-muted">—</p>
      {/if}
    </div>

    <!-- write bar -->
    <div class="mt-4 h-3 overflow-hidden rounded-full bg-surface2" role="progressbar" aria-valuemin={0} aria-valuemax={100} aria-valuenow={$progress?.percent ?? undefined}>
      <div
        class="h-full rounded-full bg-accent transition-[width] duration-200"
        style="width:{$progress?.percent != null ? Math.min(100, $progress.percent) : 6}%"
      ></div>
    </div>

    {#if $currentPhase === "finalizing" && $flashStatus === "running"}
      <p class="mt-3 rounded-xl border border-warning/40 bg-warning/10 p-2.5 text-center text-xs font-medium text-warning" role="status">
        {$t("progress.finalizingHint")}
      </p>
    {/if}

    <!-- verify bar -->
    {#if $verifyProgress && ($currentPhase === "verifying" || $currentPhase === "finalizing")}
      <div class="mt-3" in:fade>
        <div class="mb-1 flex items-center justify-between text-xs text-muted">
          <span>{$t("progress.verifyBar")}</span>
          <span class="font-mono">{formatBytes($verifyProgress.checked)} · {formatPercent($verifyProgress.percent)}</span>
        </div>
        <div class="h-2 overflow-hidden rounded-full bg-surface2">
          <div class="h-full rounded-full bg-success transition-[width] duration-200" style="width:{Math.min(100, $verifyProgress.percent)}%"></div>
        </div>
      </div>
    {/if}

    <!-- stats row -->
    {#if $progress}
      <div class="mt-4 grid grid-cols-3 gap-2 text-center">
        <div class="rounded-xl border border-edge bg-surface p-3">
          <p class="flex items-center justify-center gap-1 text-[11px] font-medium uppercase tracking-wide text-muted">
            <Zap size={12} aria-hidden="true" /> {$t("progress.speed")}
          </p>
          <p class="mt-1 font-mono text-sm font-semibold text-txt">{formatSpeed($progress.speed_mbs)}</p>
        </div>
        <div class="rounded-xl border border-edge bg-surface p-3">
          <p class="flex items-center justify-center gap-1 text-[11px] font-medium uppercase tracking-wide text-muted">
            <Gauge size={12} aria-hidden="true" /> {$t("progress.peak")}
          </p>
          <p class="mt-1 font-mono text-sm font-semibold text-txt">{formatSpeed(peakSeen)}</p>
        </div>
        <div class="rounded-xl border border-edge bg-surface p-3">
          <p class="flex items-center justify-center gap-1 text-[11px] font-medium uppercase tracking-wide text-muted">
            <Timer size={12} aria-hidden="true" /> {$t("progress.eta")}
          </p>
          <p class="mt-1 font-mono text-sm font-semibold text-txt">{formatEta($progress.eta_secs)}</p>
        </div>
      </div>
    {/if}

    <!-- log toggle -->
    <button
      type="button"
      class="mt-4 inline-flex min-h-9 items-center gap-1.5 rounded-lg px-3 py-1.5 text-xs font-medium text-muted transition-colors hover:bg-surface2 hover:text-txt"
      onclick={() => (showLog = !showLog)}
      aria-expanded={showLog}
    >
      {#if showLog}<ChevronUp size={14} aria-hidden="true" />{:else}<ChevronDown size={14} aria-hidden="true" />{/if}
      {showLog ? $t("progress.logHide") : $t("progress.log")}
    </button>
    {#if showLog}
      <div
        bind:this={logEl}
        class="terminal mt-2 max-h-48 overflow-y-auto rounded-xl border border-edge p-3"
        aria-live="polite"
      >
        {#each $logLines as line, i (i)}
          <p class="whitespace-pre-wrap break-all text-muted">{line}</p>
        {/each}
      </div>
    {/if}

    <!-- cancel -->
    <div class="mt-5 flex justify-center">
      <button
        type="button"
        class="min-h-11 rounded-xl border border-error/50 px-5 py-2.5 text-sm font-semibold text-error transition-colors hover:bg-error/10"
        onclick={() => cancelPrompt.set(true)}
      >
        {$t("common.cancel")}
      </button>
    </div>
  {/if}
</div>

<ConfirmModal
  open={$cancelPrompt}
  title={$t("progress.cancel.title")}
  body={$t("progress.cancel.body")}
  confirmLabel={$t("progress.cancel.confirm")}
  holdMs={900}
  onconfirm={doCancel}
  oncancel={() => cancelPrompt.set(false)}
/>
