<script lang="ts">
  import { createEventDispatcher } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { t } from "./i18n";

  export let state: "idle" | "recording" | "processing" | "error" = "idle";
  export let errorMsg = "";
  export let injectionFailed = false;
  export let polishFailed = false;
  export let composeContextStatus: "idle" | "capturing" | "ready" | "failed" = "idle";
  export let composeError = "";
  export let audioLevels: number[] = [];
  export let retryableSessionId: string | null = null;
  export let retrying = false;
  /** After inject succeeds: brief green check (idle + flash). */
  export let transcriptionSuccessFlash = false;
  /** Total ms the success flash will be shown — drives the countdown bar. */
  export let successFlashDurationMs = 5000;
  /** Current transcription mode: "dictate" | "smart_compose" */
  export let transcriptionMode: string = "dictate";

  const dispatch = createEventDispatcher<{
    retry: void;
    dismiss: void;
    clipboardCopy: void;
    clipboardDismiss: void;
    successCopy: void;
    composeDismiss: void;
    modeToggle: void;
  }>();

  $: showRetry = state === "error" && !!retryableSessionId;
  $: showComposeAlert =
    transcriptionMode === "smart_compose" &&
    state !== "recording" &&
    (composeContextStatus === "failed" || !!composeError);
  $: composeFailed = composeError === "__compose_failed__";

  function handleRetry(e: MouseEvent) {
    e.stopPropagation();
    dispatch("retry");
  }

  function handleDismiss(e: MouseEvent) {
    e.stopPropagation();
    dispatch("dismiss");
  }

  function handleClipboardCopy(e: MouseEvent) {
    e.stopPropagation();
    dispatch("clipboardCopy");
  }

  function handleClipboardDismiss(e: MouseEvent) {
    e.stopPropagation();
    dispatch("clipboardDismiss");
  }

  function handleSuccessCopy(e: MouseEvent) {
    e.stopPropagation();
    dispatch("successCopy");
  }

  function handleComposeDismiss(e: MouseEvent) {
    e.stopPropagation();
    dispatch("composeDismiss");
  }

  function handleModeToggle(e: MouseEvent) {
    e.stopPropagation();
    dispatch("modeToggle");
  }

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

  function barHeight(level: number): number {
    const MIN_H = 2;
    const MAX_H = 26;
    return Math.round(MIN_H + level * (MAX_H - MIN_H));
  }

  async function handleMousedown(e: MouseEvent) {
    if ((e.target as HTMLElement | null)?.closest("button")) return;
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
  class:error={state === "error" && !injectionFailed}
  class:injection={injectionFailed && !showRetry}
  class:retry={showRetry}
  class:success={transcriptionSuccessFlash}
  class:compose-alert={showComposeAlert}
  class:context-ready={state === "recording" &&
    transcriptionMode === "smart_compose" &&
    composeContextStatus === "ready"}
  class:smart-idle={state === "idle" &&
    transcriptionMode === "smart_compose" &&
    !injectionFailed &&
    !polishFailed &&
    !showComposeAlert &&
    !transcriptionSuccessFlash}
  on:mousedown={handleMousedown}
>
  {#if showRetry}
    <div class="retry-head">
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" aria-hidden="true">
        <circle cx="12" cy="12" r="10" stroke="rgba(248,113,113,0.9)" stroke-width="1.8" />
        <line
          x1="12"
          y1="7"
          x2="12"
          y2="13"
          stroke="rgba(248,113,113,0.9)"
          stroke-width="1.8"
          stroke-linecap="round"
        />
        <circle cx="12" cy="17" r="1.1" fill="rgba(248,113,113,0.9)" />
      </svg>
      <div class="retry-text">
        <p class="retry-title">{$t("hud.retry_title")}</p>
        <p class="retry-msg" title={errorMsg}>{errorMsg || $t("hud.error")}</p>
      </div>
    </div>
    <div class="retry-actions">
      <button class="btn primary" on:click={handleRetry} disabled={retrying}>
        {#if retrying}
          <span class="mini-spinner" aria-hidden="true"></span>
          {$t("hud.retrying")}
        {:else}
          {$t("hud.retry")}
        {/if}
      </button>
      <button class="btn" on:click={handleDismiss} disabled={retrying}>{$t("hud.dismiss")}</button>
    </div>
  {:else if showComposeAlert}
    <div class="compose-alert-head">
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" aria-hidden="true">
        <rect
          x="4"
          y="6"
          width="16"
          height="12"
          rx="2.5"
          stroke="rgba(251,191,36,0.92)"
          stroke-width="1.8"
        />
        <path
          d="M8 6l1.2-2h5.6L16 6M9 12h6"
          stroke="rgba(251,191,36,0.92)"
          stroke-width="1.8"
          stroke-linecap="round"
        />
      </svg>
      <div class="compose-alert-text">
        <p class="compose-alert-title">
          {composeFailed ? $t("hud.compose_failed") : $t("hud.context_failed")}
        </p>
        <p class="compose-alert-msg">
          {#if composeFailed}
            {$t("hud.compose_failed_detail")}
          {:else if composeError}
            {composeError}
          {:else}
            {$t("hud.context_failed_detail")}
          {/if}
        </p>
      </div>
    </div>
    <div class="compose-alert-actions">
      <button class="btn amber-btn" on:click={handleComposeDismiss}>
        {$t("hud.dismiss")}
      </button>
    </div>
  {:else if state === "recording"}
    <div class="dot-wrap">
      <div class="ring"></div>
      <div class="dot"></div>
    </div>
    <div class="waveform" aria-hidden="true">
      {#each audioLevels as level}
        <div class="wave-bar" style="height: {barHeight(level)}px"></div>
      {/each}
    </div>
    {#if transcriptionMode === "smart_compose"}
      <div class="compose-recording-meta">
        <span class="label red">{fmt(seconds)}</span>
        <span
          class="context-chip"
          class:capturing={composeContextStatus === "capturing"}
          class:ready={composeContextStatus === "ready"}
          class:failed={composeContextStatus === "failed"}
        >
          {#if composeContextStatus === "ready"}
            {$t("hud.context_ready")}
          {:else if composeContextStatus === "failed"}
            {$t("hud.context_failed")}
          {:else}
            {$t("hud.context_capturing")}
          {/if}
        </span>
      </div>
    {:else}
      <span class="label red">{fmt(seconds)}</span>
    {/if}
  {:else if state === "processing"}
    <div class="spinner"></div>
    <span class="label blue"
      >{transcriptionMode === "smart_compose" ? $t("hud.composing") : $t("hud.transcribing")}</span
    >
  {:else if state === "error"}
    <div class="err-dot"></div>
    <span class="label red">{errorMsg || $t("hud.error")}</span>
  {:else if injectionFailed}
    <div class="injection-head">
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" aria-hidden="true">
        <rect
          x="8"
          y="2"
          width="8"
          height="4"
          rx="1"
          stroke="rgba(251,191,36,0.9)"
          stroke-width="1.8"
        />
        <path
          d="M6 4H5a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2V6a2 2 0 0 0-2-2h-1"
          stroke="rgba(251,191,36,0.9)"
          stroke-width="1.8"
          stroke-linecap="round"
        />
      </svg>
      <div class="injection-text">
        <p class="injection-title">{$t("hud.copied_title")}</p>
        <p class="injection-msg">{$t("hud.copied_detail")}</p>
      </div>
    </div>
    <div class="injection-actions">
      <button class="btn amber-btn" on:click={handleClipboardCopy}>
        {$t("hud.copy_again")}
      </button>
      <button class="btn" on:click={handleClipboardDismiss}>
        {$t("hud.dismiss")}
      </button>
    </div>
  {:else if polishFailed}
    <div class="err-dot"></div>
    <span class="label amber">{$t("hud.polish_failed")}</span>
  {:else if transcriptionSuccessFlash}
    <div class="success-head">
      <svg
        class="check-icon"
        width="16"
        height="16"
        viewBox="0 0 24 24"
        fill="none"
        aria-hidden="true"
      >
        <circle cx="12" cy="12" r="10" stroke="rgba(62,207,142,0.95)" stroke-width="1.8" />
        <path
          d="M7.5 12.5l2.8 2.8L16.2 8.4"
          stroke="rgba(62,207,142,0.95)"
          stroke-width="1.9"
          stroke-linecap="round"
          stroke-linejoin="round"
        />
      </svg>
      <div class="success-col">
        <span class="label ok">{$t("hud.success")}</span>
        <span class="success-sub">{$t("hud.success_detail")}</span>
      </div>
    </div>
    <div class="success-actions">
      <button class="btn success-btn" on:click={handleSuccessCopy}>
        {$t("hud.copy_manually")}
      </button>
    </div>
    <!-- Auto-dismiss countdown: bar shrinks from right to left over the
         configured duration. CSS animation restarts whenever this block
         is mounted (i.e., each time transcriptionSuccessFlash toggles true). -->
    <div
      class="success-countdown"
      style="--countdown-duration: {successFlashDurationMs}ms"
      aria-hidden="true"
    ></div>
  {:else}
    {#if transcriptionMode === "smart_compose"}
      <svg
        class="compose-idle"
        width="15"
        height="15"
        viewBox="0 0 24 24"
        fill="none"
        aria-label="Smart Compose ready"
      >
        <path
          d="M4 6.5h12a3 3 0 0 1 3 3V17H7a3 3 0 0 1-3-3V6.5z"
          stroke="rgba(196,181,253,0.9)"
          stroke-width="1.8"
          stroke-linejoin="round"
        />
        <path
          d="M7 10h7M7 13.5h4.5M17.2 18.2l3.9-3.9"
          stroke="rgba(196,181,253,0.9)"
          stroke-width="1.8"
          stroke-linecap="round"
        />
        <path d="M20.2 13.2l1.6 1.6-1.2 1.2-1.6-1.6 1.2-1.2z" fill="rgba(196,181,253,0.9)" />
      </svg>
      <span class="label compose">{$t("hud.smart_ready")}</span>
    {:else}
      <svg
        class="mic-idle"
        width="14"
        height="14"
        viewBox="0 0 24 24"
        fill="none"
        aria-label="Ready"
      >
        <rect
          x="9"
          y="2"
          width="6"
          height="11"
          rx="3"
          stroke="rgba(255,255,255,0.55)"
          stroke-width="1.8"
        />
        <path
          d="M5 11a7 7 0 0 0 14 0"
          stroke="rgba(255,255,255,0.55)"
          stroke-width="1.8"
          stroke-linecap="round"
        />
        <line
          x1="12"
          y1="18"
          x2="12"
          y2="22"
          stroke="rgba(255,255,255,0.55)"
          stroke-width="1.8"
          stroke-linecap="round"
        />
        <line
          x1="9"
          y1="22"
          x2="15"
          y2="22"
          stroke="rgba(255,255,255,0.55)"
          stroke-width="1.8"
          stroke-linecap="round"
        />
      </svg>
      <span class="label muted">{$t("hud.idle")}</span>
    {/if}
    <div class="mode-sep" aria-hidden="true"></div>
    <!-- svelte-ignore a11y-click-events-have-key-events -->
    <button
      class="mode-pill"
      class:smart={transcriptionMode === "smart_compose"}
      title={$t("hud.mode.toggle_hint")}
      on:click={handleModeToggle}
      aria-label="{$t('hud.mode.toggle_hint')}: {transcriptionMode === 'smart_compose'
        ? $t('hud.mode.smart_compose')
        : $t('hud.mode.dictate')}"
    >
      {transcriptionMode === "smart_compose"
        ? $t("hud.mode.smart_compose")
        : $t("hud.mode.dictate")}
    </button>
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

  .hud.recording {
    border-color: rgba(239, 68, 68, 0.5);
  }
  .hud.processing {
    border-color: rgba(99, 130, 246, 0.5);
  }
  .hud.error {
    border-color: rgba(239, 68, 68, 0.35);
  }

  .hud.success {
    display: flex;
    flex-direction: column;
    align-items: stretch;
    justify-content: center;
    gap: 8px;
    height: 100px;
    padding: 10px 14px;
    border-radius: 14px;
    border-color: rgba(62, 207, 142, 0.45);
    background: rgba(28, 40, 34, 0.96);
    white-space: normal;
  }

  .hud.smart-idle {
    border-color: rgba(139, 92, 246, 0.34);
    background: rgba(34, 30, 44, 0.96);
  }

  .check-icon {
    flex-shrink: 0;
  }

  .success-head {
    display: flex;
    align-items: flex-start;
    gap: 8px;
  }

  .success-col {
    display: flex;
    flex-direction: column;
    gap: 2px;
    line-height: 1.25;
    min-width: 0;
    flex: 1;
  }

  .success-sub {
    font-size: 11px;
    color: rgba(255, 255, 255, 0.55);
    font-weight: 500;
  }

  .label.ok {
    color: rgba(190, 242, 210, 0.98);
    font-weight: 600;
  }

  .success-actions {
    display: flex;
    justify-content: flex-end;
  }

  .btn.success-btn {
    background: rgba(62, 207, 142, 0.18);
    border-color: rgba(62, 207, 142, 0.42);
    color: rgba(214, 255, 228, 0.96);
  }

  .btn.success-btn:hover:not(:disabled) {
    background: rgba(62, 207, 142, 0.28);
  }

  /* Auto-dismiss countdown bar — shrinks right-to-left over the configured
     duration. GPU-accelerated via transform: scaleX (cheaper than width). */
  .success-countdown {
    height: 2px;
    width: 100%;
    background: rgba(62, 207, 142, 0.85);
    border-radius: 1px;
    transform-origin: left center;
    animation: success-countdown-shrink var(--countdown-duration, 5000ms) linear forwards;
  }
  @keyframes success-countdown-shrink {
    from {
      transform: scaleX(1);
      opacity: 0.85;
    }
    to {
      transform: scaleX(0);
      opacity: 0.4;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .success-countdown {
      animation: none;
      transform: scaleX(0);
    }
  }

  /* Retry panel */
  .hud.retry {
    display: flex;
    flex-direction: column;
    align-items: stretch;
    justify-content: center;
    gap: 8px;
    height: 100px;
    padding: 10px 14px;
    border-radius: 14px;
    background: rgba(32, 24, 26, 0.96);
    border-color: rgba(239, 68, 68, 0.4);
    white-space: normal;
  }

  .hud.injection {
    display: flex;
    flex-direction: column;
    align-items: stretch;
    justify-content: center;
    gap: 8px;
    height: 100px;
    padding: 10px 14px;
    border-radius: 14px;
    background: rgba(26, 24, 18, 0.96);
    border-color: rgba(251, 191, 36, 0.4);
    white-space: normal;
  }

  .hud.compose-alert {
    display: flex;
    flex-direction: column;
    align-items: stretch;
    justify-content: center;
    gap: 8px;
    height: 100px;
    padding: 10px 14px;
    border-radius: 14px;
    background: rgba(30, 25, 18, 0.96);
    border-color: rgba(251, 191, 36, 0.44);
    white-space: normal;
  }

  .hud.context-ready {
    border-color: rgba(139, 92, 246, 0.58);
    box-shadow:
      0 1px 0 rgba(255, 255, 255, 0.06) inset,
      0 0 0 1px rgba(196, 181, 253, 0.18),
      0 0 24px rgba(139, 92, 246, 0.16);
    animation: context-flash 0.42s ease-out;
  }

  @keyframes context-flash {
    0% {
      filter: brightness(1.45);
    }
    100% {
      filter: brightness(1);
    }
  }

  .compose-alert-head {
    display: flex;
    align-items: flex-start;
    gap: 8px;
  }

  .compose-alert-text {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
    flex: 1;
  }

  .compose-alert-title {
    font-size: 12px;
    font-weight: 600;
    color: rgba(251, 191, 36, 0.95);
    margin: 0;
  }

  .compose-alert-msg {
    font-size: 11px;
    color: rgba(255, 255, 255, 0.58);
    margin: 0;
    line-height: 1.35;
  }

  .compose-alert-actions {
    display: flex;
    justify-content: flex-end;
  }

  .injection-head {
    display: flex;
    align-items: flex-start;
    gap: 8px;
  }

  .injection-text {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
    flex: 1;
  }

  .injection-title {
    font-size: 12px;
    font-weight: 600;
    color: rgba(251, 191, 36, 0.95);
    margin: 0;
    letter-spacing: 0.01em;
  }

  .injection-msg {
    font-size: 11px;
    color: rgba(255, 255, 255, 0.55);
    margin: 0;
    line-height: 1.35;
  }

  .injection-actions {
    display: flex;
    gap: 6px;
    justify-content: flex-end;
  }

  .btn.amber-btn {
    background: rgba(251, 191, 36, 0.2);
    border-color: rgba(251, 191, 36, 0.5);
    color: rgba(253, 224, 71, 0.95);
  }

  .btn.amber-btn:hover:not(:disabled) {
    background: rgba(251, 191, 36, 0.32);
  }

  .retry-head {
    display: flex;
    align-items: flex-start;
    gap: 8px;
  }

  .retry-text {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
    flex: 1;
  }

  .retry-title {
    font-size: 12px;
    font-weight: 600;
    color: #f87171;
    margin: 0;
    letter-spacing: 0.01em;
  }

  .retry-msg {
    font-size: 11px;
    color: rgba(255, 255, 255, 0.55);
    margin: 0;
    line-height: 1.35;
    overflow: hidden;
    text-overflow: ellipsis;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    -webkit-box-orient: vertical;
  }

  .retry-actions {
    display: flex;
    gap: 6px;
    justify-content: flex-end;
  }

  .btn {
    padding: 5px 12px;
    border-radius: 7px;
    border: 1px solid rgba(255, 255, 255, 0.14);
    background: rgba(255, 255, 255, 0.06);
    color: rgba(255, 255, 255, 0.8);
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
    font-family: -apple-system, "SF Pro Text", BlinkMacSystemFont, sans-serif;
    transition:
      background 0.12s,
      border-color 0.12s;
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }

  .btn:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.12);
  }

  .btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .btn.primary {
    background: rgba(239, 68, 68, 0.28);
    border-color: rgba(239, 68, 68, 0.5);
    color: #fecaca;
  }

  .btn.primary:hover:not(:disabled) {
    background: rgba(239, 68, 68, 0.42);
  }

  .mini-spinner {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    border: 1.5px solid rgba(254, 202, 202, 0.3);
    border-top-color: #fecaca;
    animation: spin 0.8s linear infinite;
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

  /* Waveform */
  .waveform {
    display: flex;
    align-items: center;
    gap: 2px;
    height: 28px;
    overflow: hidden;
  }
  .wave-bar {
    width: 2px;
    border-radius: 1px;
    background: rgba(248, 113, 113, 0.75);
    flex-shrink: 0;
    transition: height 0.08s ease-out;
    min-height: 2px;
  }

  .compose-recording-meta {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 96px;
  }

  .context-chip {
    color: rgba(255, 255, 255, 0.46);
    font-size: 10.5px;
    font-weight: 600;
    line-height: 1;
  }

  .context-chip.capturing {
    color: rgba(196, 181, 253, 0.72);
  }

  .context-chip.ready {
    color: rgba(190, 242, 210, 0.95);
  }

  .context-chip.failed {
    color: rgba(251, 191, 36, 0.92);
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

  /* Idle icons */
  .mic-idle,
  .compose-idle {
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
  .label.muted {
    color: rgba(255, 255, 255, 0.4);
  }
  .label.compose {
    color: rgba(196, 181, 253, 0.88);
  }
  /* Mode separator + pill */
  .mode-sep {
    width: 1px;
    height: 16px;
    background: rgba(255, 255, 255, 0.12);
    flex-shrink: 0;
  }

  .mode-pill {
    padding: 2px 7px;
    border-radius: 5px;
    border: 1px solid rgba(255, 255, 255, 0.12);
    background: rgba(255, 255, 255, 0.05);
    color: rgba(255, 255, 255, 0.45);
    font-size: 11px;
    font-weight: 500;
    cursor: pointer;
    font-family: -apple-system, "SF Pro Text", BlinkMacSystemFont, sans-serif;
    transition:
      background 0.12s,
      border-color 0.12s,
      color 0.12s;
    white-space: nowrap;
    line-height: 1.4;
  }

  .mode-pill:hover {
    background: rgba(255, 255, 255, 0.1);
    border-color: rgba(255, 255, 255, 0.22);
    color: rgba(255, 255, 255, 0.7);
  }

  .mode-pill.smart {
    background: rgba(139, 92, 246, 0.18);
    border-color: rgba(139, 92, 246, 0.4);
    color: rgba(196, 181, 253, 0.9);
  }

  .mode-pill.smart:hover {
    background: rgba(139, 92, 246, 0.28);
    border-color: rgba(139, 92, 246, 0.55);
    color: rgba(221, 214, 254, 0.98);
  }
</style>
