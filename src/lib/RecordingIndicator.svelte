<script lang="ts">
  import { createEventDispatcher } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { t } from "./i18n";

  export let state: "idle" | "recording" | "processing" | "error" = "idle";
  export let errorMsg = "";
  export let injectionFailed = false;
  export let polishFailed = false;
  export let audioLevels: number[] = [];
  export let retryableSessionId: string | null = null;
  export let retrying = false;
  /** After inject succeeds: brief green check (idle + flash). */
  export let transcriptionSuccessFlash = false;
  /** Transcript shown in the editable post-success review HUD. */
  export let transcript = "";

  const dispatch = createEventDispatcher<{
    retry: void;
    dismiss: void;
    clipboardCopy: void;
    clipboardDismiss: void;
    successCopy: void;
    successEdit: string;
    successFocus: void;
  }>();

  $: showRetry = state === "error" && !!retryableSessionId;

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

  function handleSuccessInput(e: Event) {
    dispatch("successEdit", (e.currentTarget as HTMLTextAreaElement).value);
  }

  function handleSuccessFocus(e: FocusEvent) {
    e.stopPropagation();
    dispatch("successFocus");
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
    if ((e.target as HTMLElement | null)?.closest("button, textarea")) return;
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
    <span class="label red">{fmt(seconds)}</span>
  {:else if state === "processing"}
    <div class="spinner"></div>
    <span class="label blue">{$t("hud.transcribing")}</span>
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
    <textarea
      class="success-transcript"
      value={transcript}
      spellcheck="true"
      aria-label="Editable transcription"
      on:input={handleSuccessInput}
      on:focus={handleSuccessFocus}
      on:mousedown|stopPropagation
    ></textarea>
    <div class="success-actions">
      <button class="btn success-btn" on:click={handleSuccessCopy}>
        {$t("hud.copy_manually")}
      </button>
    </div>
  {:else}
    <svg class="mic-idle" width="14" height="14" viewBox="0 0 24 24" fill="none" aria-label="Ready">
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
    justify-content: flex-start;
    gap: 8px;
    height: calc(100% - 8px);
    padding: 10px 14px;
    border-radius: 14px;
    border-color: rgba(62, 207, 142, 0.45);
    background: rgba(28, 40, 34, 0.96);
    white-space: normal;
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
    flex-shrink: 0;
  }

  .success-transcript {
    width: 100%;
    min-height: 70px;
    flex: 1;
    resize: none;
    overflow-y: auto;
    border-radius: 8px;
    border: 1px solid rgba(190, 242, 210, 0.18);
    background: rgba(255, 255, 255, 0.06);
    color: rgba(255, 255, 255, 0.92);
    padding: 8px 9px;
    font-family: -apple-system, "SF Pro Text", BlinkMacSystemFont, sans-serif;
    font-size: 12px;
    line-height: 1.45;
    outline: none;
    user-select: text;
    cursor: text;
  }

  .success-transcript:focus {
    border-color: rgba(62, 207, 142, 0.56);
    background: rgba(255, 255, 255, 0.09);
  }

  .btn.success-btn {
    background: rgba(62, 207, 142, 0.18);
    border-color: rgba(62, 207, 142, 0.42);
    color: rgba(214, 255, 228, 0.96);
  }

  .btn.success-btn:hover:not(:disabled) {
    background: rgba(62, 207, 142, 0.28);
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

  /* Idle mic icon */
  .mic-idle {
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
</style>
