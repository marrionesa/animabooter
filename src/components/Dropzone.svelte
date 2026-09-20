<script lang="ts">
  /**
   * Step 1 — drop a disk image (drag & drop or click) and show the file
   * chip: name, size, kind badge and the honest "→ X extracted" hint.
   */
  import { onMount } from "svelte";
  import { get } from "svelte/store";
  import { fade, fly } from "svelte/transition";
  import { FileImage, FolderOpen, Loader2, TriangleAlert } from "lucide-svelte";
  import Mascot from "./Mascot.svelte";
  import { detectImage, toErrorData } from "../lib/ipc";
  import {
    selectedImage,
    step,
    t,
  } from "../lib/stores";
  import { formatBytes } from "../lib/format";
  import type { DetectionInfo } from "../lib/types";

  let dragging = $state(false);
  let detecting = $state(false);
  let error = $state<string | null>(null);

  const EXTENSIONS = ["iso", "img", "bin", "dd", "raw", "gz", "gzip", "xz", "zst", "zstd", "bz2", "bzip2"];
  // NOTE: zip is intentionally absent — the pipeline streams gz/xz/zst/bz2
  // and raw images only; a half-supported zip would be a dishonest badge.

  function kindLabel(kind: string): string {
    const tt = get(t);
    const labels: Record<string, string> = {
      raw: tt("drop.kind.raw"),
      gzip: tt("drop.kind.gzip"),
      xz: tt("drop.kind.xz"),
      zstd: tt("drop.kind.zstd"),
      bzip2: tt("drop.kind.bzip2"),
    };
    return labels[kind] ?? kind.toUpperCase();
  }

  async function loadFromPath(path: string): Promise<void> {
    detecting = true;
    error = null;
    try {
      const detection: DetectionInfo = await detectImage(path);
      const name = path.split(/[\\/]/).pop() ?? path;
      selectedImage.set({ path, name, size: detection.file_size, detection });
      step.set("drive");
    } catch (e) {
      const err = toErrorData(e);
      error = err.message || get(t)("drop.invalid");
    } finally {
      detecting = false;
    }
  }

  async function browse(): Promise<void> {
    if (detecting) return;
    try {
      const { open } = await import("@tauri-apps/plugin-dialog");
      const selection = await open({
        multiple: false,
        directory: false,
        filters: [{ name: "Disk images", extensions: EXTENSIONS }],
      });
      if (typeof selection === "string") await loadFromPath(selection);
    } catch {
      error = get(t)("drop.invalid");
    }
  }

  onMount(() => {
    let unlisten: (() => void) | null = null;
    let cancelled = false;
    (async () => {
      try {
        const { getCurrentWebview } = await import("@tauri-apps/api/webview");
        const handler = await getCurrentWebview().onDragDropEvent((event) => {
          const payload = event.payload;
          if (payload.type === "enter" || payload.type === "over") {
            dragging = true;
          } else if (payload.type === "leave") {
            dragging = false;
          } else if (payload.type === "drop") {
            dragging = false;
            const paths = (payload as { paths?: string[] }).paths ?? [];
            const first = paths[0];
            if (first) void loadFromPath(first);
          }
        });
        if (cancelled) handler();
        else unlisten = handler;
      } catch {
        // Not running inside Tauri (e.g. plain browser preview).
      }
    })();
    return () => {
      cancelled = true;
      unlisten?.();
    };
  });
</script>

<div class="mx-auto w-full max-w-xl">
  <button
    type="button"
    class="group relative grid w-full place-items-center gap-4 rounded-3xl border-2 border-dashed px-6 py-10 text-center transition-all duration-200
      {dragging
        ? 'border-accent bg-accentsoft scale-[1.01]'
        : 'border-edge bg-surface hover:border-accent/60 hover:bg-surface2'}"
    onclick={browse}
    ondragover={(e) => {
      e.preventDefault();
      dragging = true;
    }}
    ondragleave={() => (dragging = false)}
    ondrop={(e) => e.preventDefault()}
    aria-busy={detecting}
  >
    {#if detecting}
      <Loader2 class="animate-spin text-accent" size={40} aria-hidden="true" />
      <p class="text-sm text-muted">{$t("drop.detecting")}</p>
    {:else}
      <Mascot state={dragging ? "writing" : "idle"} size={132} />
      <div>
        <p class="text-lg font-semibold text-txt">{$t("drop.title")}</p>
        <p class="mt-1 text-sm text-muted">{$t("drop.subtitle")}</p>
      </div>
      <span class="inline-flex min-h-11 items-center gap-2 rounded-xl bg-accent px-4 py-2.5 text-sm font-semibold text-white transition-transform group-hover:scale-[1.03]">
        <FolderOpen size={18} aria-hidden="true" />
        {$t("drop.browse")}
      </span>
    {/if}
  </button>

  {#if error}
    <div
      class="mt-4 flex items-start gap-2 rounded-xl border border-error/40 bg-error/10 p-3 text-sm text-error"
      transition:fade
      role="alert"
    >
      <TriangleAlert size={16} class="mt-0.5 shrink-0" aria-hidden="true" />
      <span>{error}</span>
    </div>
  {/if}

  {#if $selectedImage}
    <div
      class="mt-5 flex flex-wrap items-center gap-3 rounded-2xl border border-edge bg-surface p-4"
      transition:fly={{ y: 8, duration: 180 }}
    >
      <div class="grid size-11 shrink-0 place-items-center rounded-xl bg-accentsoft text-accent">
        <FileImage size={22} aria-hidden="true" />
      </div>
      <div class="min-w-0 flex-1">
        <p class="truncate font-mono text-sm font-semibold text-txt">{$selectedImage.name}</p>
        <p class="mt-0.5 text-xs text-muted">
          {formatBytes($selectedImage.size)}
          {#if $selectedImage.detection.compressed && $selectedImage.detection.estimated_size}
            · <span class="text-success">{$t("drop.extracted", { size: formatBytes($selectedImage.detection.estimated_size) })}</span>
          {:else if $selectedImage.detection.compressed}
            · {$t("progress.unknownSize")}
          {/if}
        </p>
      </div>
      <span class="rounded-lg bg-accentsoft px-2.5 py-1 font-mono text-xs font-semibold text-accent">
        {kindLabel($selectedImage.detection.kind)}
      </span>
      <button
        type="button"
        class="min-h-9 rounded-lg px-3 py-1.5 text-xs font-medium text-muted transition-colors hover:bg-surface2 hover:text-txt"
        onclick={browse}
      >
        {$t("drop.change")}
      </button>

      {#if $selectedImage.detection.is_windows_iso}
        <div class="w-full rounded-xl border border-warning/40 bg-warning/10 p-3 text-xs text-warning" role="alert">
          <p class="font-semibold">⚠ {$t("drop.windowsIso.title")}</p>
          <p class="mt-0.5 opacity-90">{$t("drop.windowsIso.body")}</p>
        </div>
      {/if}
    </div>
  {/if}
</div>
