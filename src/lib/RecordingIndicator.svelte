<script lang="ts">
  export let state: "idle" | "recording" | "processing" | "error" = "idle";
  export let errorMsg = "";
  export let lastTranscription = "";
  export let injectionFailed = false;

  let seconds = 0;
  let timer: ReturnType<typeof setInterval> | null = null;

  function startTimer() {
    if (timer !== null) clearInterval(timer);
    seconds = 0;
    timer = setInterval(() => { seconds = seconds + 1; }, 1000);
  }

  function stopTimer() {
    if (timer !== null) {
      clearInterval(timer);
      timer = null;
    }
    seconds = 0;
  }

  let _prevState = state;
  $: if (state !== _prevState) {
    _prevState = state;
    if (state === "recording") {
      startTimer();
    } else {
      stopTimer();
    }
  }

  function formatTime(s: number) {
    const m = Math.floor(s / 60);
    const sec = s % 60;
    return `${m}:${sec.toString().padStart(2, "0")}`;
  }
</script>

<div
  class="panel"
  class:state-recording={state === "recording"}
  class:state-processing={state === "processing"}
  class:state-error={state === "error"}
  class:state-idle={state === "idle"}
>
  {#if state === "idle" && !injectionFailed && !lastTranscription}
    <!-- minimal idle state -->
    <div class="idle-content">
      <div class="mic-svg">
        <svg width="20" height="20" viewBox="0 0 24 24" fill="none">
          <rect x="9" y="2" width="6" height="13" rx="3" fill="rgba(255,255,255,0.55)"/>
          <path d="M5 10a7 7 0 0 0 14 0" stroke="rgba(255,255,255,0.55)" stroke-width="1.8" stroke-linecap="round"/>
          <line x1="12" y1="19" x2="12" y2="22" stroke="rgba(255,255,255,0.55)" stroke-width="1.8" stroke-linecap="round"/>
          <line x1="8" y1="22" x2="16" y2="22" stroke="rgba(255,255,255,0.55)" stroke-width="1.8" stroke-linecap="round"/>
        </svg>
      </div>
      <span class="idle-label">准备就绪</span>
    </div>

  {:else if state === "recording"}
    <div class="recording-content">
      <div class="dot-wrap">
        <div class="pulse-outer"></div>
        <div class="red-dot"></div>
      </div>
      <div class="recording-right">
        <span class="rec-label">录音中</span>
        <span class="timer">{formatTime(seconds)}</span>
      </div>
    </div>

  {:else if state === "processing"}
    <div class="processing-content">
      <div class="arc-spinner"></div>
      <span class="proc-label">转录中</span>
    </div>

  {:else if state === "error"}
    <div class="error-content">
      <div class="error-icon-wrap">
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none">
          <circle cx="12" cy="12" r="10" stroke="#f87171" stroke-width="2"/>
          <line x1="12" y1="7" x2="12" y2="13" stroke="#f87171" stroke-width="2" stroke-linecap="round"/>
          <circle cx="12" cy="17" r="1.2" fill="#f87171"/>
        </svg>
      </div>
      {#if errorMsg}
        <span class="error-text">{errorMsg}</span>
      {/if}
    </div>

  {/if}

  {#if (state === "idle" || injectionFailed) && lastTranscription}
    <div class="result-row" class:failed={injectionFailed}>
      {#if injectionFailed}
        <div class="clipboard-hint">
          <svg width="13" height="13" viewBox="0 0 24 24" fill="none">
            <rect x="8" y="2" width="8" height="4" rx="1" stroke="rgba(255,200,80,0.85)" stroke-width="1.8"/>
            <path d="M6 4H5a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2V6a2 2 0 0 0-2-2h-1" stroke="rgba(255,200,80,0.85)" stroke-width="1.8" stroke-linecap="round"/>
          </svg>
          <span class="paste-hint">已复制 — ⌘V 粘贴</span>
        </div>
      {/if}
      <div class="transcription-text" class:failed={injectionFailed}>
        {lastTranscription.length > 80 ? lastTranscription.slice(0, 80) + "…" : lastTranscription}
      </div>
    </div>
  {/if}
</div>

<style>
  .panel {
    display: flex;
    flex-direction: column;
    align-items: stretch;
    gap: 10px;
    padding: 14px 18px;
    border-radius: 16px;
    background: rgba(30, 30, 32, 0.85);
    backdrop-filter: blur(20px) saturate(180%);
    -webkit-backdrop-filter: blur(20px) saturate(180%);
    border: 1px solid rgba(255, 255, 255, 0.12);
    box-shadow: 0 8px 40px rgba(0, 0, 0, 0.45), 0 1px 0 rgba(255,255,255,0.06) inset;
    min-width: 200px;
    max-width: 300px;
    transition: border-color 0.2s ease, background 0.2s ease;
  }

  .panel.state-recording {
    border-color: rgba(239, 68, 68, 0.35);
    background: rgba(35, 20, 20, 0.88);
  }

  .panel.state-processing {
    border-color: rgba(99, 130, 246, 0.3);
    background: rgba(20, 24, 40, 0.88);
  }

  .panel.state-error {
    border-color: rgba(248, 113, 113, 0.3);
    background: rgba(35, 20, 20, 0.88);
  }

  /* Idle state */
  .idle-content {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .mic-svg {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 32px;
    height: 32px;
    border-radius: 50%;
    background: rgba(255,255,255,0.06);
  }

  .idle-label {
    font-size: 13px;
    color: rgba(255,255,255,0.45);
    font-weight: 400;
    letter-spacing: 0.01em;
  }

  /* Recording state */
  .recording-content {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .dot-wrap {
    position: relative;
    width: 28px;
    height: 28px;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }

  .red-dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: #ef4444;
    position: relative;
    z-index: 2;
    animation: dot-pulse 1.4s ease-in-out infinite;
  }

  .pulse-outer {
    position: absolute;
    inset: 0;
    border-radius: 50%;
    background: rgba(239, 68, 68, 0.25);
    animation: ring-expand 1.4s ease-out infinite;
  }

  @keyframes dot-pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.7; }
  }

  @keyframes ring-expand {
    0% { transform: scale(0.5); opacity: 0.8; }
    100% { transform: scale(1.8); opacity: 0; }
  }

  .recording-right {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .rec-label {
    font-size: 13px;
    font-weight: 600;
    color: #ef4444;
    letter-spacing: 0.02em;
  }

  .timer {
    font-size: 11px;
    color: rgba(239, 68, 68, 0.7);
    font-variant-numeric: tabular-nums;
    letter-spacing: 0.08em;
    font-weight: 500;
  }

  /* Processing state */
  .processing-content {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .arc-spinner {
    width: 22px;
    height: 22px;
    border-radius: 50%;
    border: 2.5px solid rgba(99, 130, 246, 0.18);
    border-top-color: #818cf8;
    border-right-color: rgba(99, 130, 246, 0.5);
    animation: spin-arc 0.9s cubic-bezier(0.4, 0, 0.6, 1) infinite;
    flex-shrink: 0;
  }

  @keyframes spin-arc {
    to { transform: rotate(360deg); }
  }

  .proc-label {
    font-size: 13px;
    color: rgba(129, 140, 248, 0.9);
    font-weight: 500;
    letter-spacing: 0.02em;
  }

  /* Error state */
  .error-content {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .error-icon-wrap {
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }

  .error-text {
    font-size: 12px;
    color: rgba(248, 113, 113, 0.85);
    line-height: 1.4;
    max-width: 200px;
  }

  /* Result row */
  .result-row {
    display: flex;
    flex-direction: column;
    gap: 5px;
    border-top: 1px solid rgba(255,255,255,0.08);
    padding-top: 9px;
  }

  .clipboard-hint {
    display: flex;
    align-items: center;
    gap: 5px;
  }

  .paste-hint {
    font-size: 11px;
    color: rgba(255, 200, 80, 0.85);
    font-weight: 500;
    letter-spacing: 0.02em;
  }

  .transcription-text {
    font-size: 12px;
    color: rgba(255, 255, 255, 0.45);
    line-height: 1.5;
    word-break: break-word;
  }

  .transcription-text.failed {
    color: rgba(255, 210, 100, 0.8);
  }
</style>
