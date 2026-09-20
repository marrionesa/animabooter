<script lang="ts">
  /**
   * Anima — the reactive wisp mascot.
   * Pure inline SVG + CSS keyframes. No image files, no lottie, no network.
   * States: idle | writing | verifying | done | error
   */
  export type MascotState = "idle" | "writing" | "verifying" | "done" | "error";

  let {
    state = "idle",
    size = 140,
  }: { state?: MascotState; size?: number } = $props();

  const sparkles = [
    { x: 18, y: 22, d: 0 },
    { x: 96, y: 14, d: 0.2 },
    { x: 104, y: 58, d: 0.4 },
    { x: 12, y: 62, d: 0.1 },
    { x: 60, y: 6, d: 0.3 },
  ];
</script>

<div
  class="mascot mascot--{state}"
  style="width:{size}px;height:{size}px"
  role="img"
  aria-label="Anima the mascot — {state}"
>
  <!-- speed lines (writing) -->
  {#if state === "writing"}
    <span class="line line-1"></span>
    <span class="line line-2"></span>
    <span class="line line-3"></span>
  {/if}

  <!-- scan ring (verifying) -->
  {#if state === "verifying"}
    <svg class="scan" viewBox="0 0 120 120" aria-hidden="true">
      <circle cx="60" cy="60" r="44" fill="none" stroke="var(--accent)" stroke-width="2" stroke-dasharray="10 14" stroke-linecap="round" />
    </svg>
  {/if}

  <!-- sparkles (done) -->
  {#if state === "done"}
    {#each sparkles as s, i (i)}
      <span class="sparkle" style="left:{s.x}%;top:{s.y}%;animation-delay:{s.d}s">✦</span>
    {/each}
  {/if}

  <svg class="body" viewBox="0 0 120 120" aria-hidden="true">
    <defs>
      <linearGradient id="anima-grad" x1="0" y1="0" x2="1" y2="1">
        <stop offset="0%" stop-color="var(--accent)" />
        <stop offset="100%" stop-color="var(--accent)" stop-opacity="0.62" />
      </linearGradient>
    </defs>

    <!-- wisp body -->
    <path
      class="wisp"
      fill="url(#anima-grad)"
      d="M60 16c-21 0-35 15-35 36 0 13 3 20 3 28 0 5-2 8.6-4 11.4-1.7 2.5.3 5.9 3.3 5.5 6.6-.9 11.4-4.2 14.8-8.3 5.3 2.6 11.2 4 17.9 4 21 0 35-16.4 35-40.6C95 31 81 16 60 16Z"
    />

    <!-- face -->
    {#if state === "error"}
      <!-- dizzy cross eyes -->
      <g stroke="var(--bg)" stroke-width="4" stroke-linecap="round">
        <line x1="40" y1="48" x2="52" y2="60" />
        <line x1="52" y1="48" x2="40" y2="60" />
        <line x1="68" y1="48" x2="80" y2="60" />
        <line x1="80" y1="48" x2="68" y2="60" />
      </g>
      <path d="M52 76c3 3 6 4 8 4s5-1 8-4" fill="none" stroke="var(--bg)" stroke-width="3.4" stroke-linecap="round" />
    {:else if state === "done"}
      <!-- happy closed eyes + open smile -->
      <path d="M40 54c2.6-3.4 9.4-3.4 12 0" fill="none" stroke="var(--bg)" stroke-width="4" stroke-linecap="round" />
      <path d="M68 54c2.6-3.4 9.4-3.4 12 0" fill="none" stroke="var(--bg)" stroke-width="4" stroke-linecap="round" />
      <path d="M50 66c4 6 16 6 20 0" fill="var(--bg)" opacity="0.9" />
    {:else if state === "verifying"}
      <!-- focused eyes: pupil + scan animation -->
      <circle cx="46" cy="54" r="7.5" fill="var(--bg)" />
      <circle cx="74" cy="54" r="7.5" fill="var(--bg)" />
      <circle cx="46" cy="54" r="2.6" fill="var(--text)" class="pupil" />
      <circle cx="74" cy="54" r="2.6" fill="var(--text)" class="pupil pupil--delay" />
    {:else}
      <!-- idle / writing eyes -->
      <circle cx="46" cy="54" r="6.5" fill="var(--bg)" />
      <circle cx="74" cy="54" r="6.5" fill="var(--bg)" />
      <circle cx="47.5" cy="52.5" r="2.2" fill="var(--text)" />
      <circle cx="75.5" cy="52.5" r="2.2" fill="var(--text)" />
      <path d="M53 68c3.4 3 10.6 3 14 0" fill="none" stroke="var(--bg)" stroke-width="3.4" stroke-linecap="round" />
    {/if}

    <!-- blush -->
    <ellipse cx="38" cy="64" rx="5" ry="3" fill="var(--bg)" opacity="0.25" />
    <ellipse cx="82" cy="64" rx="5" ry="3" fill="var(--bg)" opacity="0.25" />
  </svg>
</div>

<style>
  .mascot {
    position: relative;
    display: grid;
    place-items: center;
  }
  .body {
    width: 100%;
    height: 100%;
    animation: float 3.2s ease-in-out infinite;
  }

  /* ---- states ---- */
  .mascot--writing .body {
    animation: dash 0.9s ease-in-out infinite;
  }
  .mascot--verifying .body {
    animation: zoom 2.4s ease-in-out infinite;
  }
  .mascot--done .body {
    animation: bounce 1.1s ease-in-out infinite;
  }
  .mascot--error .body {
    animation: shake 0.5s ease-in-out infinite;
  }

  .pupil {
    animation: scanx 1.6s ease-in-out infinite;
    transform-origin: 46px 54px;
  }
  .pupil--delay {
    animation-delay: 0.15s;
    transform-origin: 74px 54px;
  }

  .scan {
    position: absolute;
    inset: 0;
    animation: spin 2.4s linear infinite;
    opacity: 0.85;
  }

  .line {
    position: absolute;
    height: 5px;
    width: 26%;
    border-radius: 999px;
    background: var(--accent);
    opacity: 0.55;
    animation: whoosh 0.9s linear infinite;
  }
  .line-1 { top: 34%; left: -18%; }
  .line-2 { top: 50%; left: -26%; width: 32%; animation-delay: 0.15s; }
  .line-3 { top: 66%; left: -14%; width: 20%; animation-delay: 0.3s; }

  .sparkle {
    position: absolute;
    color: var(--accent);
    font-size: 14px;
    animation: pop 1.1s ease-out infinite;
  }

  /* ---- keyframes ---- */
  @keyframes float {
    0%, 100% { transform: translateY(0); }
    50% { transform: translateY(-7px); }
  }
  @keyframes dash {
    0% { transform: translateY(-3px) rotate(-6deg); }
    50% { transform: translateY(3px) rotate(-3deg); }
    100% { transform: translateY(-3px) rotate(-6deg); }
  }
  @keyframes zoom {
    0%, 100% { transform: scale(1); }
    50% { transform: scale(1.05); }
  }
  @keyframes bounce {
    0%, 100% { transform: translateY(0) scale(1); }
    35% { transform: translateY(-12px) scale(1.04); }
    60% { transform: translateY(0) scale(0.98); }
  }
  @keyframes shake {
    0%, 100% { transform: translateX(0) rotate(0deg); }
    25% { transform: translateX(-4px) rotate(-2.5deg); }
    75% { transform: translateX(4px) rotate(2.5deg); }
  }
  @keyframes scanx {
    0%, 100% { transform: translateX(-2.6px); }
    50% { transform: translateX(2.6px); }
  }
  @keyframes spin {
    to { transform: rotate(360deg); }
  }
  @keyframes whoosh {
    0% { transform: translateX(0); opacity: 0; }
    25% { opacity: 0.55; }
    100% { transform: translateX(160%); opacity: 0; }
  }
  @keyframes pop {
    0% { transform: scale(0) translateY(6px); opacity: 0; }
    40% { transform: scale(1.15) translateY(-2px); opacity: 1; }
    100% { transform: scale(0.7) translateY(-12px); opacity: 0; }
  }
</style>
