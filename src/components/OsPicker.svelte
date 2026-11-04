<script lang="ts">
  /**
   * "Download an official OS" — modal picker + download progress.
   * Chosen from the main screen (Dropzone); when the download finishes the
   * ISO is already selected as the wizard image and the wizard moves to the
   * drive step, so this modal auto-closes. Cancel aborts (and deletes the
   * .part file) without surfacing an error.
   */
  import { onMount } from "svelte";
  import { get } from "svelte/store";
  import { fade, scale } from "svelte/transition";
  import { Download, ExternalLink, ShieldCheck, X } from "lucide-svelte";
  import Mascot from "./Mascot.svelte";
  import { cancelDownload, toErrorData } from "../lib/ipc";
  import {
    downloadActive,
    downloadError,
    downloadLog,
    downloadProgress,
    downloadStatus,
    loadOsCatalog,
    osCatalog,
    startDownloadAndSelect,
    t,
    toast,
  } from "../lib/stores";
  import { formatBytes, formatEta, formatPercent, formatSpeed } from "../lib/format";
  import type { OsCatalogEntry } from "../lib/types";

  let openState = $state(false);
  let picked: OsCatalogEntry | null = $state(null);
  let showLog = $state(false);
  let logEl = $state<HTMLElement | null>(null);
  let closeButton = $state<HTMLButtonElement | null>(null);
  let catalogRequested = false;

  const statusLabel = {
    resolving: () => get(t)("osd.status.resolving"),
    downloading: () => get(t)("osd.status.downloading"),
    verifying: () => get(t)("osd.status.verifying"),
    done: () => get(t)("osd.status.done"),
  } as const;

  // Auto-scroll the terminal-style log.
  $effect(() => {
    void $downloadLog.length;
    if (showLog && logEl) logEl.scrollTop = logEl.scrollHeight;
  });

  // Success: the wizard already moved to the drive step — close shortly so
  // the user perceives the "done" state before the hand-off.
  $effect(() => {
    if (picked && !$downloadActive && !$downloadError) {
      const id = window.setTimeout(() => {
        openState = false;
        picked = null;
        showLog = false;
      }, 900);
      return () => window.clearTimeout(id);
    }
  });

  onMount(() => {
    if (catalogRequested) return;
    catalogRequested = true;
    void loadOsCatalog();
  });

  function close(): void {
    // Closing during an active download is the same as cancelling it.
    if ($downloadActive) {
      void cancelDownload();
    }
    openState = false;
    picked = null;
  }

  async function pick(entry: OsCatalogEntry): Promise<void> {
    if ($downloadActive) return;
    picked = entry;
    showLog = false;
    await startDownloadAndSelect(entry.id);
  }

  async function abort(): Promise<void> {
    try {
      await cancelDownload();
    } catch (e) {
      toast.set({ kind: "err", text: toErrorData(e).message });
    }
  }

  function backToPicker(): void {
    downloadError.set(null);
    picked = null;
  }
</script>

<!-- trigger: sits next to "Explorar archivos" on the main screen -->
<button
  type="button"
  class="inline-flex min-h-11 items-center gap-2 rounded-xl border border-edge bg-surface px-4 py-2.5 text-sm font-semibold text-txt transition-colors hover:bg-surface2"
  onclick={() => (openState = true)}
>
  <Download size={18} aria-hidden="true" />
  {$t("osd.pick.button")}
</button>

{#if openState}
  <div
    class="fixed inset-0 z-50 grid place-items-center bg-black/60 p-4 backdrop-blur-sm"
    transition:fade={{ duration: 140 }}
    role="presentation"
    onpointerdown={(event) => {
      if (event.target === event.currentTarget) close();
    }}
  >
    <div
      class="max-h-[min(42rem,calc(100vh-2rem))] w-full max-w-lg overflow-y-auto rounded-2xl border border-edge bg-surface p-5 shadow-2xl sm:p-6"
      transition:scale={{ duration: 160, start: 0.96 }}
      role="dialog"
      aria-modal="true"
      aria-label={$t("osd.pick.title")}
    >
      <div class="flex items-center justify-between gap-4">
        <h2 class="text-lg font-semibold text-txt">{$t("osd.pick.title")}</h2>
        <button
          bind:this={closeButton}
          type="button"
          class="grid size-10 shrink-0 place-items-center rounded-xl text-muted transition-colors hover:bg-surface2 hover:text-txt"
          onclick={close}
          aria-label={$t("common.close")}
          title={$t("common.close")}
        >
          <X size={20} aria-hidden="true" />
        </button>
      </div>

      {#if !$downloadActive && !picked}
        <!-- catalog grid -->
        <p class="mt-1 text-sm text-muted">{$t("osd.pick.subtitle")}</p>
        <div class="mt-4 grid grid-cols-1 gap-2 sm:grid-cols-2">
          {#each $osCatalog as entry (entry.id)}
            <div class="group relative">
              <button
                type="button"
                class="flex min-h-20 w-full flex-col justify-center gap-1 rounded-xl border border-edge bg-surface2/50 p-3 pr-10 text-left transition-colors hover:border-accent/60 hover:bg-surface2"
                onclick={() => pick(entry)}
              >
                <span class="flex items-center gap-2 font-semibold text-txt">
                  <span aria-hidden="true">{entry.icon}</span>
                  {entry.name}
                </span>
                <span class="text-xs text-muted">
                  ~{formatBytes(entry.size_bytes)} · amd64/x86_64
                </span>
              </button>
              <a
                class="absolute right-2 top-2 grid size-7 place-items-center rounded-lg text-muted transition-colors hover:bg-surface hover:text-accent"
                href={entry.website}
                target="_blank"
                rel="noreferrer"
                aria-label={$t("osd.pick.website", { name: entry.name })}
                title={$t("osd.pick.website", { name: entry.name })}
              >
                <ExternalLink size={14} aria-hidden="true" />
              </a>
            </div>
          {:else}
            <p class="col-span-2 rounded-xl border border-edge bg-surface2/50 p-4 text-sm text-muted">
              {$t("osd.pick.empty")}
            </p>
          {/each}
        </div>
        <p class="mt-4 flex items-start gap-2 rounded-xl border border-edge bg-terminal p-3 text-[11px] text-muted">
          <ShieldCheck size={14} class="mt-0.5 shrink-0 text-success" aria-hidden="true" />
          {$t("osd.pick.trustNote")}
        </p>
      {:else}
        <!-- download progress / error panel -->
        <div class="mt-3 grid place-items-center gap-2">
          <Mascot state={$downloadStatus === "verifying" || $downloadStatus === "done" ? "done" : "writing"} size={120} />
          <h3 class="text-base font-semibold text-txt">
            {$downloadStatus ? statusLabel[$downloadStatus]() : ""}
          </h3>
          {#if picked}
            <p class="font-mono text-xs text-muted">{picked.name}</p>
          {/if}
        </div>

        {#if !$downloadError}
          <div class="mt-3 text-center">
            {#if $downloadProgress && $downloadProgress.percent != null}
              <p class="font-mono text-5xl font-bold text-accent">
                {formatPercent($downloadProgress.percent)}
              </p>
              {#if $downloadProgress.total}
                <p class="mt-1 text-xs text-muted">
                  {formatBytes($downloadProgress.downloaded)}
                  {$t("progress.of", { total: formatBytes($downloadProgress.total) })}
                </p>
              {/if}
            {:else}
              <p class="font-mono text-4xl font-bold text-muted">…</p>
            {/if}
          </div>

          <div
            class="mt-4 h-3 overflow-hidden rounded-full bg-surface2"
            role="progressbar"
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={$downloadProgress?.percent ?? undefined}
          >
            <div
              class="h-full rounded-full bg-accent transition-[width] duration-200"
              style="width:{$downloadProgress?.percent != null
                ? Math.min(100, $downloadProgress.percent)
                : 6}%"
            ></div>
          </div>

          {#if $downloadProgress}
            <div class="mt-4 grid grid-cols-2 gap-2 text-center">
              <div class="rounded-xl border border-edge bg-surface p-3">
                <p class="text-[11px] font-medium uppercase tracking-wide text-muted">
                  {$t("progress.speed")}
                </p>
                <p class="mt-1 font-mono text-sm font-semibold text-txt">
                  {formatSpeed($downloadProgress.speed_mbs)}
                </p>
              </div>
              <div class="rounded-xl border border-edge bg-surface p-3">
                <p class="text-[11px] font-medium uppercase tracking-wide text-muted">
                  {$t("progress.eta")}
                </p>
                <p class="mt-1 font-mono text-sm font-semibold text-txt">
                  {formatEta($downloadProgress.eta_secs)}
                </p>
              </div>
            </div>
          {/if}
        {/if}

        {#if $downloadLog.length > 0}
          <button
            type="button"
            class="mt-4 inline-flex min-h-9 items-center gap-1.5 rounded-lg px-3 py-1.5 text-xs font-medium text-muted transition-colors hover:bg-surface2 hover:text-txt"
            onclick={() => (showLog = !showLog)}
            aria-expanded={showLog}
          >
            {$t("progress.log")}
          </button>
          {#if showLog}
            <div
              bind:this={logEl}
              class="terminal mt-2 max-h-36 overflow-y-auto rounded-xl border border-edge p-3"
              aria-live="polite"
            >
              {#each $downloadLog as line, i (i)}
                <p class="whitespace-pre-wrap break-all text-muted">{line}</p>
              {/each}
            </div>
          {/if}
        {/if}

        {#if $downloadError}
          <div class="mt-4 rounded-2xl border border-error/40 bg-error/10 p-4" role="alert">
            <p class="text-sm font-semibold text-error">{$downloadError.message}</p>
            {#if $downloadError.hint}
              <p class="mt-2 text-xs text-muted">
                <span class="font-semibold text-txt">{$t("error.hint")}:</span>
                {$downloadError.hint}
              </p>
            {/if}
            <div class="mt-4 flex justify-end gap-3">
              <button
                type="button"
                class="min-h-11 rounded-xl border border-edge px-4 py-2 text-sm font-medium text-muted transition-colors hover:bg-surface2 hover:text-txt"
                onclick={backToPicker}
              >
                {$t("common.back")}
              </button>
              {#if picked}
                <button
                  type="button"
                  class="min-h-11 rounded-xl bg-accent px-4 py-2 text-sm font-semibold text-white transition-transform hover:scale-[1.02]"
                  onclick={() => pick(picked!)}
                >
                  {$t("common.retry")}
                </button>
              {/if}
            </div>
          </div>
        {/if}

        {#if $downloadActive}
          <div class="mt-5 flex justify-center">
            <button
              type="button"
              class="min-h-11 rounded-xl border border-error/50 px-5 py-2.5 text-sm font-semibold text-error transition-colors hover:bg-error/10"
              onclick={abort}
            >
              {$t("common.cancel")}
            </button>
          </div>
        {/if}
      {/if}
    </div>
  </div>
{/if}
