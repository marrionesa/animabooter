<script lang="ts">
  /**
   * Modal with hold-to-confirm (1.2 s) — the anti-misclick pattern that
   * makes destructive actions feel deliberate. Works with mouse, touch and
   * keyboard (hold Enter or Space).
   */
  import type { Snippet } from "svelte";
  import { AlertTriangle } from "lucide-svelte";
  import { fade, scale } from "svelte/transition";

  let {
    open,
    title,
    body,
    confirmLabel,
    onconfirm,
    oncancel,
    requireHold = true,
    holdMs = 1200,
    danger = true,
    children,
  }: {
    open: boolean;
    title: string;
    body: string;
    confirmLabel: string;
    onconfirm: () => void;
    oncancel: () => void;
    requireHold?: boolean;
    holdMs?: number;
    danger?: boolean;
    children?: Snippet;
  } = $props();

  let holding = $state(false);
  let fill = $state(0);
  let buttonEl = $state<HTMLButtonElement | null>(null);
  let rafId = 0;
  let startAt = 0;

  function begin(): void {
    if (!open) return;
    if (!requireHold) {
      onconfirm();
      return;
    }
    if (holding) return;
    holding = true;
    startAt = performance.now();
    const tick = (): void => {
      if (!holding) return;
      fill = Math.min(1, (performance.now() - startAt) / holdMs);
      if (fill >= 1) {
        holding = false;
        fill = 0;
        onconfirm();
        return;
      }
      rafId = requestAnimationFrame(tick);
    };
    rafId = requestAnimationFrame(tick);
  }

  function abort(): void {
    holding = false;
    fill = 0;
    if (rafId) cancelAnimationFrame(rafId);
  }

  function onKeydown(event: KeyboardEvent): void {
    if ((event.key === "Enter" || event.key === " ") && !event.repeat) {
      event.preventDefault();
      begin();
    } else if (event.key === "Escape") {
      event.preventDefault();
      oncancel();
    }
  }

  function onKeyup(event: KeyboardEvent): void {
    if (event.key === "Enter" || event.key === " ") abort();
  }

  $effect(() => {
    if (open) {
      // Focus the destructive control so keyboard users are one key away.
      queueMicrotask(() => buttonEl?.focus());
    } else {
      abort();
    }
    return () => abort();
  });
</script>

<svelte:window onkeydown={open ? onKeydown : undefined} onkeyup={open ? onKeyup : undefined} />

{#if open}
  <div
    class="fixed inset-0 z-50 grid place-items-center bg-black/60 p-4 backdrop-blur-sm"
    transition:fade={{ duration: 140 }}
    role="presentation"
    onpointerdown={(e) => {
      if (e.target === e.currentTarget) oncancel();
    }}
  >
    <div
      class="w-full max-w-md rounded-2xl border border-edge bg-surface p-6 shadow-2xl"
      transition:scale={{ duration: 160, start: 0.96 }}
      role="alertdialog"
      aria-modal="true"
      aria-label={title}
    >
      <div class="flex items-start gap-3">
        <div class="grid size-10 shrink-0 place-items-center rounded-xl bg-error/15 text-error">
          <AlertTriangle size={22} aria-hidden="true" />
        </div>
        <div class="min-w-0">
          <h2 class="text-lg font-semibold text-txt">{title}</h2>
          <p class="mt-1 text-sm text-muted">{body}</p>
        </div>
      </div>

      {#if children}
        {@render children()}
      {/if}

      <div class="mt-6 flex items-center justify-end gap-3">
        <button
          type="button"
          class="min-h-11 rounded-xl px-4 py-2.5 text-sm font-medium text-muted transition-colors hover:bg-surface2 hover:text-txt"
          onclick={oncancel}
        >
          Esc
        </button>
        <button
          bind:this={buttonEl}
          type="button"
          class="relative min-h-11 select-none overflow-hidden rounded-xl px-5 py-2.5 text-sm font-semibold transition-colors {danger ? 'bg-error text-white' : 'bg-accent text-white'}"
          onpointerdown={begin}
          onpointerup={abort}
          onpointerleave={abort}
          onpointercancel={abort}
          oncontextmenu={(e) => e.preventDefault()}
        >
          {#if requireHold}
            <span
              class="absolute inset-y-0 left-0 {danger ? 'bg-red-800/70' : 'bg-black/30'}"
              style="width:{(fill * 100).toFixed(1)}%"
              aria-hidden="true"
            ></span>
          {/if}
          <span class="relative z-10 flex items-center gap-2">{confirmLabel}</span>
        </button>
      </div>
    </div>
  </div>
{/if}
