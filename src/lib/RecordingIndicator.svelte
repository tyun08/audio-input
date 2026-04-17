<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { t } from "./i18n";

  export let state: "idle" | "recording" | "processing" | "error" = "idle";
  export let errorMsg = "";
  export let lastTranscription = "";
  export let injectionFailed = false;
  export let polishFailed = false;

  let seconds = 0;
  let timer: ReturnType<typeof setInterval> | null = null;

  function startTimer() {
    if (timer !== null) clearInterval(timer);
    seconds = 0;
    timer = setInterval(() => {
      seconds += 1;
    }, 1000);
  }

  function stopTimer() {
    if (timer !== null) {
      clearInterval(timer);
      timer = null;
    }
    seconds = 0;
  }

  $: if (state === "recording") {
    if (timer === null) startTimer();
  } else {
    if (timer !== null) stopTimer();
  }

  function fmt(s: number) {
    return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
  }

  async function handleMousedown() {
    await getCurrentWindow().startDragging();
  }
</script>

<!-- svelte-ignore a11y-no-noninteractive-element-interactions -->
<div
  class="hud"
  role="region"
  aria-label="Recording status"
  class:recording={state === "recording"}
  class:processing={state === "processing"}
  class:error={state === "error" || injectionFailed}
  on:mousedown={handleMousedown}
>
  {#if state === "recording"}
    <div class="dot-wrap">
      <div class="ring"></div>
      <div class="dot"></div>
    </div>
    <span class="label red">{fmt(seconds)}</span>
  {:else if state === "processing"}
    <div class="spinner"></div>
    <span class="label blue">{$t("hud.transcribing")}</span>
  {:else if state === "error"}
    <div class="err-dot"></div>
    <span class="label red">{errorMsg || $t("hud.error")}</span>
  {:else if injectionFailed && lastTranscription}
    <svg class="clip-icon" width="13" height="13" viewBox="0 0 24 24" fill="none">
      <rect
        x="8"
        y="2"
        width="8"
        height="4"
        rx="1"
        stroke="rgba(255,200,80,0.9)"
        stroke-width="1.8"
      />
      <path
        d="M6 4H5a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2V6a2 2 0 0 0-2-2h-1"
        stroke="rgba(255,200,80,0.9)"
        stroke-width="1.8"
        stroke-linecap="round"
      />
    </svg>
    <span class="label amber">{$t("hud.copied")}</span>
  {:else if polishFailed}
    <div class="err-dot"></div>
    <span class="label amber">{$t("hud.polish_failed")}</span>
  {/if}
</div>

<style>
  .hud {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 14px;
    height: 40px;
    border-radius: 999px;
    background: rgba(36, 36, 38, 0.96);
    border: 1px solid rgba(255, 255, 255, 0.08);
    box-shadow: 0 1px 0 rgba(255, 255, 255, 0.06) inset;
    white-space: nowrap;
    transition: border-color 0.2s;
    cursor: grab;
  }

  .hud:active {
    cursor: grabbing;
  }

  .hud.processing {
    border-color: rgba(99, 130, 246, 0.25);
  }
  .hud.error {
    border-color: rgba(239, 68, 68, 0.25);
  }

  /* Red dot + pulse */
  .dot-wrap {
    position: relative;
    width: 16px;
    height: 16px;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: #ef4444;
    position: relative;
    z-index: 2;
    animation: blink 1.4s ease-in-out infinite;
  }
  .ring {
    position: absolute;
    inset: 0;
    border-radius: 50%;
    background: rgba(239, 68, 68, 0.3);
    animation: expand 1.4s ease-out infinite;
  }
  @keyframes blink {
    0%,
    100% {
      opacity: 1;
    }
    50% {
      opacity: 0.65;
    }
  }
  @keyframes expand {
    0% {
      transform: scale(0.4);
      opacity: 0.9;
    }
    100% {
      transform: scale(1.9);
      opacity: 0;
    }
  }

  /* Spinner */
  .spinner {
    width: 14px;
    height: 14px;
    border-radius: 50%;
    border: 2px solid rgba(99, 130, 246, 0.15);
    border-top-color: #818cf8;
    animation: spin 0.8s linear infinite;
    flex-shrink: 0;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  /* Error dot */
  .err-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: #f87171;
    flex-shrink: 0;
  }

  /* Clipboard icon */
  .clip-icon {
    flex-shrink: 0;
  }

  /* Labels */
  .label {
    font-size: 13px;
    font-weight: 500;
    letter-spacing: 0.01em;
    font-family: -apple-system, "SF Pro Text", BlinkMacSystemFont, sans-serif;
    font-variant-numeric: tabular-nums;
  }
  .label.red {
    color: #f87171;
  }
  .label.blue {
    color: #818cf8;
  }
  .label.amber {
    color: rgba(255, 200, 80, 0.9);
  }
</style>
