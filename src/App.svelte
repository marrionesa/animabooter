<script lang="ts">
  /**
   * Root layout: header (brand + settings), wizard body, sticky footer.
   * Registers the flash://… event listeners once and mirrors them into
   * the stores; fires a desktop notification when the flash completes.
   */
  import { onMount } from "svelte";
  import { get } from "svelte/store";
  import { fade } from "svelte/transition";
  import { Languages, Palette, Shield, ShieldAlert } from "lucide-svelte";
  import Mascot from "./components/Mascot.svelte";
  import Wizard from "./components/Wizard.svelte";
  import ConfirmModal from "./components/ConfirmModal.svelte";
  import { getSettings, onDone, onError, onLog, onPhase, onProgress, onVerify, setSettings, toErrorData } from "./lib/ipc";
  import {
    applyTheme,
    currentPhase,
    flashError,
    flashStatus,
    lang,
    logLines,
    progress,
    result,
    settings,
    showToast,
    t,
    toast,
    verifyProgress,
  } from "./lib/stores";
  import type { Lang, Settings, Theme } from "./lib/types";
  import { THEMES } from "./lib/types";

  let unsafeOpen = $state(false);
  let unsafeStage = $state(0);
  let settingsReady = $state(false);

  onMount(() => {
    const unlisteners: Array<() => void> = [];
    let disposed = false;

    (async () => {
      // --- settings first so the theme applies before first paint of body ---
      try {
        const loaded = await getSettings();
        settings.set(loaded);
        if (loaded.theme) applyTheme(loaded.theme);
      } catch {
        // Defaults already applied.
      }
      settingsReady = true;

      // --- flash://… event bridge ---
      const registrations = await Promise.all([
        onPhase((phase) => currentPhase.set(phase)),
        onProgress((p) => progress.set(p)),
        onVerify((v) => verifyProgress.set(v)),
        onLog((line) => logLines.update((lines) => [...lines.slice(-400), line])),
        onDone((d) => {
          // Guard against the command result racing with this event.
          if (!get(result)) result.set(d);
          flashStatus.set("done");
          void notifyDone();
        }),
        onError((e) => {
          if (!get(flashError)) flashError.set(e);
          if (get(flashStatus) === "running") flashStatus.set("error");
        }),
      ]);
      if (disposed) registrations.forEach((u) => u());
      else unlisteners.push(...registrations);
    })();

    return () => {
      disposed = true;
      unlisteners.forEach((u) => u());
    };
  });

  async function notifyDone(): Promise<void> {
    try {
      const notification = await import("@tauri-apps/plugin-notification");
      let granted = await notification.isPermissionGranted();
      if (!granted) {
        const requested = await notification.requestPermission();
        granted = requested === "granted";
      }
      if (granted) {
        notification.sendNotification({
          title: "AnimaBooter",
          body: get(t)("result.notify"),
        });
      }
    } catch {
      // Notifications are optional; never block the UI on them.
    }
  }

  async function persist(next: Settings): Promise<void> {
    settings.set(next);
    try {
      await setSettings(next);
    } catch (e) {
      showToast("err", toErrorData(e).message);
    }
  }

  function cycleTheme(): void {
    const current = $settings.theme;
    const next: Theme = THEMES[(THEMES.indexOf(current) + 1) % THEMES.length] ?? "midnight";
    void persist({ ...$settings, theme: next });
  }

  function cycleLang(): void {
    const next: Lang = $lang === "en" ? "es" : "en";
    void persist({ ...$settings, language: next });
  }

  function themeLabel(theme: Theme): string {
    return { midnight: "Midnight", "catppuccin-mocha": "Catppuccin", "tokyo-night": "Tokyo Night" }[theme];
  }

  function requestUnsafeToggle(): void {
    if ($settings.unsafe_mode) {
      void persist({ ...$settings, unsafe_mode: false });
      return;
    }
    unsafeStage = 1;
    unsafeOpen = true;
  }

  function confirmUnsafe(): void {
    if (unsafeStage === 1) {
      unsafeStage = 2;
      return;
    }
    unsafeOpen = false;
    unsafeStage = 0;
    void persist({ ...$settings, unsafe_mode: true });
  }
</script>

<div class="flex min-h-screen flex-col bg-bg text-txt">
  <header class="sticky top-0 z-40 border-b border-edge bg-bg/85 backdrop-blur">
    <div class="mx-auto flex w-full max-w-3xl items-center justify-between gap-3 px-4 py-3">
      <div class="flex items-center gap-3">
        <Mascot state={$flashStatus === "running" ? "writing" : "idle"} size={44} />
        <div class="leading-tight">
          <p class="font-bold tracking-tight text-txt">{$t("app.name")}</p>
          <p class="text-[11px] text-muted">{$t("app.tagline")}</p>
        </div>
      </div>

      <div class="flex items-center gap-1.5" role="toolbar" aria-label="{$t('settings.theme')} / {$t('settings.language')}">
        <button
          type="button"
          class="grid min-h-11 min-w-11 place-items-center rounded-xl border border-edge bg-surface text-muted transition-colors hover:border-accent/50 hover:text-txt"
          onclick={cycleTheme}
          title="{$t('settings.theme')}: {themeLabel($settings.theme)}"
          aria-label="{$t('settings.theme')}: {themeLabel($settings.theme)}"
        >
          <Palette size={18} aria-hidden="true" />
        </button>
        <button
          type="button"
          class="grid min-h-11 min-w-11 place-items-center rounded-xl border border-edge bg-surface font-mono text-xs font-bold uppercase text-muted transition-colors hover:border-accent/50 hover:text-txt"
          onclick={cycleLang}
          title="{$t('settings.language')}: {$lang.toUpperCase()}"
          aria-label="{$t('settings.language')}: {$lang.toUpperCase()}"
        >
          <span class="flex items-center gap-1"><Languages size={14} aria-hidden="true" />{$lang}</span>
        </button>
        <button
          type="button"
          class="grid min-h-11 min-w-11 place-items-center rounded-xl border transition-colors
            {$settings.unsafe_mode
              ? 'border-error/60 bg-error/15 text-error'
              : 'border-edge bg-surface text-muted hover:border-accent/50 hover:text-txt'}"
          onclick={requestUnsafeToggle}
          title={$settings.unsafe_mode ? $t("settings.unsafe.on") : $t("settings.unsafe.off")}
          aria-pressed={$settings.unsafe_mode}
          aria-label={$settings.unsafe_mode ? $t("settings.unsafe.on") : $t("settings.unsafe.off")}
        >
          {#if $settings.unsafe_mode}
            <ShieldAlert size={18} aria-hidden="true" />
          {:else}
            <Shield size={18} aria-hidden="true" />
          {/if}
        </button>
      </div>
    </div>
  </header>

  <main class="mx-auto w-full max-w-3xl flex-1 px-4 py-8">
    <Wizard />
  </main>

  <footer class="mt-auto border-t border-edge px-4 pb-[max(0.75rem,env(safe-area-inset-bottom))] pt-3 text-center">
    <p class="font-mono text-[11px] text-muted">{$t("app.version")}</p>
  </footer>
</div>

{#if $toast}
  <div
    class="fixed bottom-5 left-1/2 z-50 -translate-x-1/2 rounded-xl border px-4 py-2.5 text-sm font-medium shadow-xl
      {$toast.kind === 'ok' ? 'border-success/50 bg-success/15 text-success' : 'border-error/50 bg-error/15 text-error'}"
    role="status"
    transition:fade={{ duration: 140 }}
  >
    {$toast.text}
  </div>
{/if}

<ConfirmModal
  open={unsafeOpen}
  title={unsafeStage === 1 ? $t("settings.unsafe.title") : $t("settings.unsafe.title2")}
  body={unsafeStage === 1 ? $t("settings.unsafe.body1") : $t("settings.unsafe.body2")}
  confirmLabel={unsafeStage === 1 ? $t("settings.unsafe.enable") : $t("settings.unsafe.enable")}
  onconfirm={confirmUnsafe}
  oncancel={() => {
    unsafeOpen = false;
    unsafeStage = 0;
  }}
/>
