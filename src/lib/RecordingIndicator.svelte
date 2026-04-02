<script lang="ts">
  export let state: "idle" | "recording" | "processing" | "error" = "idle";
  export let errorMsg = "";
  export let lastTranscription = "";
  export let injectionFailed = false;

  // 录音计时
  let seconds = 0;
  let timer: ReturnType<typeof setInterval> | null = null;

  $: {
    if (state === "recording") {
      seconds = 0;
      timer = setInterval(() => seconds++, 1000);
    } else {
      if (timer) {
        clearInterval(timer);
        timer = null;
      }
    }
  }

  function formatTime(s: number) {
    const m = Math.floor(s / 60);
    const sec = s % 60;
    return `${m}:${sec.toString().padStart(2, "0")}`;
  }
</script>

<div class="indicator" class:recording={state === "recording"} class:processing={state === "processing"} class:error={state === "error"}>
  <div class="icon-wrap">
    {#if state === "idle"}
      <!-- 空闲时不显示 -->
    {:else if state === "recording"}
      <div class="pulse-ring"></div>
      <div class="mic-icon recording">
        <svg width="28" height="28" viewBox="0 0 24 24" fill="none">
          <rect x="9" y="2" width="6" height="13" rx="3" fill="currentColor"/>
          <path d="M5 10a7 7 0 0 0 14 0" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>
          <line x1="12" y1="19" x2="12" y2="22" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>
          <line x1="8" y1="22" x2="16" y2="22" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>
        </svg>
      </div>
      <div class="timer">{formatTime(seconds)}</div>
    {:else if state === "processing"}
      <div class="spinner"></div>
      <div class="label">转录中...</div>
    {:else if state === "error"}
      <div class="error-icon">
        <svg width="24" height="24" viewBox="0 0 24 24" fill="none">
          <circle cx="12" cy="12" r="10" stroke="currentColor" stroke-width="2"/>
          <line x1="12" y1="8" x2="12" y2="12" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>
          <circle cx="12" cy="16" r="1" fill="currentColor"/>
        </svg>
      </div>
      {#if errorMsg}
        <div class="error-msg">{errorMsg}</div>
      {/if}
    {/if}
  </div>

  {#if lastTranscription && state === "idle"}
    <div class="transcription-preview" class:failed={injectionFailed}>
      {lastTranscription}
    </div>
    {#if injectionFailed}
      <div class="injection-hint">已复制到剪贴板，请手动粘贴 (⌘V)</div>
    {/if}
  {/if}
</div>

<style>
  .indicator {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 12px;
    padding: 24px;
    border-radius: 20px;
    backdrop-filter: blur(20px);
    -webkit-backdrop-filter: blur(20px);
    background: rgba(30, 30, 35, 0.85);
    border: 1px solid rgba(255, 255, 255, 0.08);
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.4);
    min-width: 160px;
    transition: all 0.25s ease;
  }

  .indicator.recording {
    background: rgba(220, 38, 38, 0.15);
    border-color: rgba(239, 68, 68, 0.3);
  }

  .indicator.processing {
    background: rgba(37, 99, 235, 0.15);
    border-color: rgba(59, 130, 246, 0.3);
  }

  .indicator.error {
    background: rgba(161, 98, 7, 0.15);
    border-color: rgba(234, 179, 8, 0.3);
  }

  .icon-wrap {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    position: relative;
  }

  /* 录音中：麦克风图标 */
  .mic-icon {
    position: relative;
    z-index: 2;
    color: #ef4444;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  /* 脉冲扩散圆环 */
  .pulse-ring {
    position: absolute;
    width: 56px;
    height: 56px;
    border-radius: 50%;
    background: rgba(239, 68, 68, 0.2);
    animation: pulse 1.5s ease-out infinite;
    z-index: 1;
  }

  @keyframes pulse {
    0% { transform: scale(0.8); opacity: 0.8; }
    100% { transform: scale(1.8); opacity: 0; }
  }

  .timer {
    font-size: 14px;
    font-weight: 600;
    color: #ef4444;
    font-variant-numeric: tabular-nums;
    letter-spacing: 0.05em;
  }

  /* 处理中：旋转圆圈 */
  .spinner {
    width: 32px;
    height: 32px;
    border: 3px solid rgba(59, 130, 246, 0.2);
    border-top-color: #3b82f6;
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  .label {
    font-size: 13px;
    color: #93c5fd;
    font-weight: 500;
  }

  /* 错误 */
  .error-icon {
    color: #eab308;
  }

  .error-msg {
    font-size: 12px;
    color: #fde68a;
    text-align: center;
    max-width: 180px;
    line-height: 1.4;
  }

  /* 转录预览 */
  .transcription-preview {
    font-size: 13px;
    color: rgba(255, 255, 255, 0.5);
    text-align: center;
    max-width: 200px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    padding: 0 8px;
  }

  .transcription-preview.failed {
    color: rgba(255, 220, 100, 0.85);
    white-space: normal;
    word-break: break-all;
  }

  .injection-hint {
    font-size: 11px;
    color: rgba(255, 200, 60, 0.6);
    text-align: center;
    max-width: 200px;
  }
</style>
