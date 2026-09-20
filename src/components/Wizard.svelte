<script lang="ts">
  /**
   * The 4-step wizard: Image → Drive → Confirm → Flash (+ Result).
   * Fully keyboard accessible; ESC always steps back (during flash it opens
   * the cancel confirmation instead of leaving the device mid-write).
   */
  import { get } from "svelte/store";
  import { Check, ChevronLeft } from "lucide-svelte";
  import { fade } from "svelte/transition";
  import Dropzone from "./Dropzone.svelte";
  import DriveList from "./DriveList.svelte";
  import ConfirmModal from "./ConfirmModal.svelte";
  import ProgressView from "./ProgressView.svelte";
  import ResultCard from "./ResultCard.svelte";
  import {
    cancelPrompt,
    flashStatus,
    resetFlashState,
    selectedDrive,
    selectedImage,
    startFlashAndRun,
    step,
    t,
    verifyEnabled,
  } from "../lib/stores";

  let showConfirm = $state(false);

  const stepKeys = ["steps.image", "steps.drive", "steps.confirm", "steps.write"] as const;
  const stepValue = $derived<number>(
    $step === "image" ? 0 : $step === "drive" ? 1 : $step === "confirm" ? 2 : 3
  );

  function onKeydown(event: KeyboardEvent): void {
    if (event.key !== "Escape") return;
    // While a flash is running, ESC only opens the cancel confirmation.
    if ($step === "flash" && $flashStatus === "running") {
      cancelPrompt.set(true);
      return;
    }
    if (showConfirm) {
      showConfirm = false;
      return;
    }
    if ($step === "drive") {
      selectedDrive.set(null);
      step.set("image");
    } else if ($step === "result") {
      // Result is a terminal step; ESC resets to the beginning.
      resetAll();
    }
  }

  function resetAll(): void {
    resetFlashState();
    selectedDrive.set(null);
    step.set("image");
  }

  async function startFlash(): Promise<void> {
    showConfirm = false;
    await startFlashAndRun();
  }
</script>

<svelte:window onkeydown={onKeydown} />

<!-- step dots -->
{#if $step !== "confirm"}
  <ol class="mx-auto mb-6 flex w-fit items-center gap-2" aria-label="Wizard progress">
    {#each stepKeys as key, i (key)}
      <li class="flex items-center gap-2">
        <span
          class="grid size-7 place-items-center rounded-full border text-xs font-bold transition-colors
            {i < stepValue
              ? 'border-success bg-success/15 text-success'
              : i === stepValue
                ? 'border-accent bg-accent text-white'
                : 'border-edge bg-surface text-muted'}"
          aria-current={i === stepValue ? "step" : undefined}
        >
          {#if i < stepValue}<Check size={14} aria-hidden="true" />{:else}{i + 1}{/if}
        </span>
        <span class="hidden text-xs font-medium text-muted sm:inline">{$t(key)}</span>
        {#if i < stepKeys.length - 1}
          <span class="h-px w-4 bg-edge sm:w-8" aria-hidden="true"></span>
        {/if}
      </li>
    {/each}
  </ol>
{/if}

{#if $step !== "image" && $step !== "confirm" && $step !== "flash"}
  <button
    type="button"
    class="mx-auto mb-4 flex min-h-9 items-center gap-1 rounded-lg px-3 py-1.5 text-xs font-medium text-muted transition-colors hover:bg-surface2 hover:text-txt"
    onclick={() => step.set($step === "result" ? "drive" : "drive")}
  >
    <ChevronLeft size={14} aria-hidden="true" />
    {$t("common.back")}
  </button>
{/if}

{#key $step}
  <div in:fade={{ duration: 150 }}>
    {#if $step === "image"}
      <Dropzone />
    {:else if $step === "drive"}
      <DriveList />
    {:else if $step === "flash"}
      <ProgressView />
    {:else if $step === "result"}
      <ResultCard />
    {/if}
  </div>
{/key}

<ConfirmModal
  open={showConfirm}
  title={$t("confirm.title")}
  body={$t("confirm.subtitle")}
  confirmLabel={$t("confirm.hold")}
  onconfirm={startFlash}
  oncancel={() => {
    showConfirm = false;
    step.set("drive");
  }}
>
  {#snippet children()}
    <dl class="mt-4 space-y-2 rounded-xl border border-edge bg-terminal p-4 font-mono text-xs">
      <div class="flex gap-2">
        <dt class="w-16 shrink-0 text-muted">{$t("confirm.image")}</dt>
        <dd class="truncate text-txt">{$selectedImage?.name}</dd>
      </div>
      <div class="flex gap-2">
        <dt class="w-16 shrink-0 text-muted">{$t("confirm.target")}</dt>
        <dd class="truncate text-error">{$selectedDrive?.path}</dd>
      </div>
      <div class="flex gap-2">
        <dt class="w-16 shrink-0 text-muted">{$t("confirm.size")}</dt>
        <dd class="text-txt">{$selectedDrive ? `${$selectedDrive.vendor} ${$selectedDrive.model}`.trim() : ""}</dd>
      </div>
      <div class="flex gap-2">
        <dt class="w-16 shrink-0 text-muted">{$t("confirm.bus")}</dt>
        <dd class="text-txt">{$selectedDrive?.bus.toUpperCase()}</dd>
      </div>
    </dl>

    {#if $selectedDrive?.is_system}
      <p class="mt-3 rounded-lg border border-error/40 bg-error/10 p-2.5 text-xs font-semibold text-error" role="alert">
        {$t("confirm.systemWarn")}
      </p>
    {/if}

    <label class="mt-3 flex min-h-11 cursor-pointer items-center gap-2.5 rounded-xl border border-edge bg-surface2 px-3 text-sm text-txt">
      <input type="checkbox" bind:checked={$verifyEnabled} class="size-4 accent-[var(--accent)]" />
      {$t("confirm.verify")}
    </label>
    <p class="mt-2 text-[11px] text-muted">{$t("confirm.holdKeyHint")}</p>
  {/snippet}
</ConfirmModal>
