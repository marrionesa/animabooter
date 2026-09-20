<script lang="ts">
  /**
   * Root layout: header (brand + settings), wizard body, sticky footer.
   * Registers the flash://… event listeners once and mirrors them into
   * the stores; fires a desktop notification when the flash completes.
   */
  import { onMount } from "svelte";
  import { get } from "svelte/store";
  import { fade } from "svelte/transition";
  import { CircleHelp, Languages, Palette, Settings as SettingsIcon, Shield, ShieldAlert } from "lucide-svelte";
  import Mascot from "./components/Mascot.svelte";
  import Wizard from "./components/Wizard.svelte";
  import ConfirmModal from "./components/ConfirmModal.svelte";
  import OverlayDialog from "./components/OverlayDialog.svelte";
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
  let settingsOpen = $state(false);
  let helpOpen = $state(false);

  const REPOSITORY_URL = "https://github.com/marrionesa/animabooter";
  const AUTHOR_URL = "https://github.com/marrionesa";

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

  function preventContextMenu(event: MouseEvent): void {
    event.preventDefault();
  }
</script>

<svelte:window oncontextmenu={preventContextMenu} />

<div class="flex min-h-screen flex-col bg-bg text-txt">
  <header class="sticky top-0 z-40 border-b border-edge bg-bg/85 backdrop-blur">
    <div class="mx-auto flex w-full max-w-3xl items-center justify-between gap-3 px-4 py-3">
      <div class="flex items-center gap-3">
        <Mascot state={$flashStatus === "running" ? "writing" : "idle"} size={44} />
        <div class="leading-tight">
          <p class="font-bold tracking-tight text-txt"><a class="transition-colors hover:text-accent" href={REPOSITORY_URL} target="_blank" rel="noreferrer">{$t("app.name")}</a></p>
          <p class="text-[11px] text-muted">{$t("app.tagline")}</p>
        </div>
      </div>

      <div class="flex items-center gap-1.5" role="toolbar" aria-label={$t("app.toolbar")}>
        <button
          type="button"
          class="grid min-h-11 min-w-11 place-items-center rounded-xl border border-edge bg-surface text-muted transition-colors hover:border-accent/50 hover:text-txt"
          onclick={() => (settingsOpen = true)}
          title={$t("settings.title")}
          aria-label={$t("settings.title")}
        >
          <SettingsIcon size={19} aria-hidden="true" />
        </button>
        <button
          type="button"
          class="grid min-h-11 min-w-11 place-items-center rounded-xl border border-edge bg-surface text-muted transition-colors hover:border-accent/50 hover:text-txt"
          onclick={() => (helpOpen = true)}
          title={$t("help.title")}
          aria-label={$t("help.title")}
        >
          <CircleHelp size={20} aria-hidden="true" />
        </button>
      </div>
    </div>
  </header>

  <main class="mx-auto w-full max-w-3xl flex-1 px-4 py-8">
    <Wizard />
  </main>

  <footer class="mt-auto border-t border-edge px-4 pb-[max(0.75rem,env(safe-area-inset-bottom))] pt-3 text-center">
    <p class="font-mono text-[11px] text-muted">
      <a class="transition-colors hover:text-accent" href={REPOSITORY_URL} target="_blank" rel="noreferrer">{$t("app.name")}</a>
      <span> · {$t("app.version")} · {$t("app.createdBy")} </span>
      <a class="transition-colors hover:text-accent" href={AUTHOR_URL} target="_blank" rel="noreferrer">marrionesa</a>
      <span> · {$t("app.openSource")}</span>
    </p>
  </footer>
</div>

<OverlayDialog
  open={settingsOpen}
  title={$t("settings.title")}
  closeLabel={$t("common.close")}
  onclose={() => (settingsOpen = false)}
>
  <div class="mt-5 space-y-3">
    <section class="rounded-xl border border-edge bg-bg/45 p-4">
      <div class="flex items-center gap-2 text-sm font-medium text-txt">
        <Palette size={17} class="text-accent" aria-hidden="true" />
        {$t("settings.theme")}
      </div>
      <div class="mt-3 flex items-center justify-between gap-3">
        <p class="text-sm text-muted">{themeLabel($settings.theme)}</p>
        <button
          type="button"
          class="min-h-10 rounded-xl border border-edge bg-surface px-3 text-sm font-medium text-txt transition-colors hover:border-accent/50"
          onclick={cycleTheme}
          aria-label="{$t('settings.changeTheme')}: {themeLabel($settings.theme)}"
        >
          {$t("settings.change")}
        </button>
      </div>
    </section>

    <section class="rounded-xl border border-edge bg-bg/45 p-4">
      <div class="flex items-center gap-2 text-sm font-medium text-txt">
        <Languages size={17} class="text-accent" aria-hidden="true" />
        {$t("settings.language")}
      </div>
      <div class="mt-3 flex items-center justify-between gap-3">
        <p class="text-sm text-muted">{$lang === "es" ? "Español" : "English"}</p>
        <button
          type="button"
          class="min-h-10 rounded-xl border border-edge bg-surface px-3 text-sm font-medium text-txt transition-colors hover:border-accent/50"
          onclick={cycleLang}
          aria-label="{$t('settings.changeLanguage')}: {$lang.toUpperCase()}"
        >
          {$t("settings.change")}
        </button>
      </div>
    </section>

    <section class="rounded-xl border border-edge bg-bg/45 p-4">
      <div class="flex items-center gap-2 text-sm font-medium text-txt">
        {#if $settings.unsafe_mode}
          <ShieldAlert size={17} class="text-error" aria-hidden="true" />
        {:else}
          <Shield size={17} class="text-accent" aria-hidden="true" />
        {/if}
        {$t("settings.unsafe.titleShort")}
      </div>
      <div class="mt-3 flex items-center justify-between gap-3">
        <p class="text-sm text-muted">{$settings.unsafe_mode ? $t("settings.unsafe.on") : $t("settings.unsafe.off")}</p>
        <button
          type="button"
          class="min-h-10 rounded-xl border px-3 text-sm font-medium transition-colors
            {$settings.unsafe_mode
              ? 'border-error/50 bg-error/10 text-error hover:bg-error/20'
              : 'border-edge bg-surface text-txt hover:border-accent/50'}"
          onclick={requestUnsafeToggle}
          aria-pressed={$settings.unsafe_mode}
        >
          {$settings.unsafe_mode ? $t("settings.unsafe.disable") : $t("settings.unsafe.enable")}
        </button>
      </div>
    </section>
  </div>
</OverlayDialog>

<OverlayDialog
  open={helpOpen}
  title={$t("help.title")}
  closeLabel={$t("common.close")}
  onclose={() => (helpOpen = false)}
>
  <div class="mt-5 space-y-5 text-sm">
    <section>
      <h3 class="font-medium text-txt">{$t("help.how.title")}</h3>
      <ol class="mt-2 space-y-2 text-muted">
        <li><span class="mr-2 font-mono text-accent">01</span>{$t("help.how.image")}</li>
        <li><span class="mr-2 font-mono text-accent">02</span>{$t("help.how.drive")}</li>
        <li><span class="mr-2 font-mono text-accent">03</span>{$t("help.how.flash")}</li>
      </ol>
    </section>

    <section class="rounded-xl border border-edge bg-bg/45 p-4">
      <h3 class="font-medium text-txt">{$t("help.safety.title")}</h3>
      <p class="mt-1.5 leading-relaxed text-muted">{$t("help.safety.body")}</p>
    </section>

    <section class="border-t border-edge pt-4">
      <h3 class="font-medium text-txt">{$t("help.about.title")}</h3>
      <p class="mt-1.5 leading-relaxed text-muted">
        <a class="text-accent underline-offset-2 hover:underline" href={REPOSITORY_URL} target="_blank" rel="noreferrer">{$t("app.name")}</a>
        {$t("help.about.body")}
        <a class="text-accent underline-offset-2 hover:underline" href={AUTHOR_URL} target="_blank" rel="noreferrer">marrionesa</a>.
        {$t("help.about.license")}
      </p>
      <p class="mt-2 font-mono text-xs text-accent">{$t("app.version")}</p>
    </section>
  </div>
</OverlayDialog>

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
