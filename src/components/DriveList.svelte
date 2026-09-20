<script lang="ts">
  /**
   * Step 2 — removable drive cards with bus icons, red SYSTEM badges in
   * unsafe mode, refresh and honest empty/error states.
   */
  import { onMount } from "svelte";
  import { get } from "svelte/store";
  import { fly, fade } from "svelte/transition";
  import { CreditCard, HardDrive, RefreshCw, ShieldAlert, Usb } from "lucide-svelte";
  import { listDrives, toErrorData } from "../lib/ipc";
  import {
    drives,
    drivesError,
    loadingDrives,
    selectedDrive,
    settings,
    step,
    t,
  } from "../lib/stores";
  import { formatBytes } from "../lib/format";
  import type { DriveInfo } from "../lib/types";

  async function refresh(): Promise<void> {
    loadingDrives.set(true);
    drivesError.set(null);
    try {
      const list = await listDrives();
      drives.set(list);
      // Drop the selection if the drive vanished between scans.
      const current = $selectedDrive;
      if (current && !list.some((d) => d.path === current.path)) {
        selectedDrive.set(null);
      }
    } catch (e) {
      drivesError.set(toErrorData(e).message);
    } finally {
      loadingDrives.set(false);
    }
  }

  function busIcon(bus: string) {
    if (bus === "usb") return Usb;
    if (bus === "sd") return CreditCard; // closest match in lucide 0.474 for SD cards
    return HardDrive;
  }

  function busLabel(bus: string): string {
    const tt = get(t);
    if (bus === "usb") return tt("misc.bus.usb");
    if (bus === "sd") return tt("misc.bus.sd");
    if (bus === "sata") return tt("misc.bus.sata");
    if (bus === "nvme") return tt("misc.bus.nvme");
    return tt("misc.bus.unknown");
  }

  function select(drive: DriveInfo): void {
    selectedDrive.set(drive);
    step.set("confirm");
  }

  onMount(() => {
    void refresh();
  });
</script>

<div class="mx-auto w-full max-w-2xl">
  <div class="mb-4 flex items-center justify-between">
    <h2 class="text-lg font-semibold text-txt">{$t("drives.title")}</h2>
    <button
      type="button"
      class="inline-flex min-h-11 items-center gap-2 rounded-xl border border-edge bg-surface px-4 py-2 text-sm font-medium text-muted transition-colors hover:border-accent/50 hover:text-txt"
      onclick={refresh}
      disabled={$loadingDrives}
    >
      <RefreshCw size={16} class={$loadingDrives ? "animate-spin" : ""} aria-hidden="true" />
      {$t("drives.refresh")}
    </button>
  </div>

  {#if $drivesError}
    <div class="rounded-2xl border border-error/40 bg-error/10 p-4 text-sm text-error" role="alert" transition:fade>
      {$t("drives.failed", { message: $drivesError })}
    </div>
  {:else if !$loadingDrives && $drives.length === 0}
    <div class="grid place-items-center gap-3 rounded-3xl border border-dashed border-edge bg-surface px-6 py-14 text-center" transition:fade>
      <HardDrive size={40} class="text-muted" aria-hidden="true" />
      <div>
        <p class="font-semibold text-txt">{$t("drives.empty")}</p>
        <p class="mt-1 text-sm text-muted">{$t("drives.emptyHint")}</p>
      </div>
    </div>
  {:else}
    <ul class="grid max-h-96 gap-3 overflow-y-auto pr-1">
      {#each $drives as drive (drive.path)}
        {@const Icon = busIcon(drive.bus)}
        {@const isSelected = $selectedDrive?.path === drive.path}
        <li transition:fly={{ y: 6, duration: 160 }}>
          <button
            type="button"
            class="flex w-full items-center gap-4 rounded-2xl border p-4 text-left transition-all
              {isSelected
                ? 'border-accent bg-accentsoft'
                : 'border-edge bg-surface hover:border-accent/50 hover:bg-surface2'}"
            onclick={() => select(drive)}
            aria-pressed={isSelected}
          >
            <div class="grid size-12 shrink-0 place-items-center rounded-xl {isSelected ? 'bg-accent text-white' : 'bg-accentsoft text-accent'}">
              <Icon size={24} aria-hidden="true" />
            </div>
            <div class="min-w-0 flex-1">
              <p class="truncate font-semibold text-txt">
                {#if drive.vendor}{drive.vendor} {/if}{drive.model || drive.id}
              </p>
              <p class="mt-0.5 font-mono text-xs text-muted">
                {drive.path} · {formatBytes(drive.size_bytes)} · {busLabel(drive.bus)}
                {#if drive.serial}
                  · {drive.serial}
                {/if}
              </p>
            </div>
            <div class="flex shrink-0 flex-col items-end gap-1.5">
              {#if drive.is_system}
                <span class="inline-flex items-center gap-1 rounded-lg bg-error/15 px-2 py-1 font-mono text-[11px] font-bold text-error">
                  <ShieldAlert size={12} aria-hidden="true" />
                  {$t("drives.system")}
                </span>
              {:else if !drive.removable}
                <span class="rounded-lg bg-warning/15 px-2 py-1 font-mono text-[11px] font-bold text-warning">
                  {$t("drives.internal")}
                </span>
              {:else}
                <span class="rounded-lg bg-success/15 px-2 py-1 font-mono text-[11px] font-bold text-success">
                  {$t("drives.removable")}
                </span>
              {/if}
              <span class="text-xs font-medium {isSelected ? 'text-accent' : 'text-muted'}">
                {isSelected ? $t("drives.selected") : $t("drives.select")}
              </span>
            </div>
          </button>
        </li>
      {/each}
    </ul>
  {/if}
</div>
