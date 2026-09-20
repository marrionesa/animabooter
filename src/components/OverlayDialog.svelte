<script lang="ts">
  /** A small, reusable modal for non-destructive app panels. */
  import type { Snippet } from "svelte";
  import { X } from "lucide-svelte";
  import { fade, scale } from "svelte/transition";

  let {
    open,
    title,
    closeLabel,
    onclose,
    children,
  }: {
    open: boolean;
    title: string;
    closeLabel: string;
    onclose: () => void;
    children?: Snippet;
  } = $props();

  let closeButton = $state<HTMLButtonElement | null>(null);

  function onKeydown(event: KeyboardEvent): void {
    if (event.key === "Escape") {
      event.preventDefault();
      onclose();
    }
  }

  $effect(() => {
    if (open) queueMicrotask(() => closeButton?.focus());
  });
</script>

<svelte:window onkeydown={open ? onKeydown : undefined} />

{#if open}
  <div
    class="fixed inset-0 z-50 grid place-items-center bg-black/60 p-4 backdrop-blur-sm"
    transition:fade={{ duration: 140 }}
    role="presentation"
    onpointerdown={(event) => {
      if (event.target === event.currentTarget) onclose();
    }}
  >
    <div
      class="max-h-[min(42rem,calc(100vh-2rem))] w-full max-w-lg overflow-y-auto rounded-2xl border border-edge bg-surface p-5 shadow-2xl sm:p-6"
      transition:scale={{ duration: 160, start: 0.96 }}
      role="dialog"
      aria-modal="true"
      aria-label={title}
    >
      <div class="flex items-center justify-between gap-4">
        <h2 class="text-lg font-semibold text-txt">{title}</h2>
        <button
          bind:this={closeButton}
          type="button"
          class="grid size-10 shrink-0 place-items-center rounded-xl text-muted transition-colors hover:bg-surface2 hover:text-txt"
          onclick={onclose}
          aria-label={closeLabel}
          title={closeLabel}
        >
          <X size={20} aria-hidden="true" />
        </button>
      </div>

      {#if children}
        {@render children()}
      {/if}
    </div>
  </div>
{/if}
