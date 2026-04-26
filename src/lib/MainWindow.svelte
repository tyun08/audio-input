<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import type { UnlistenFn } from "@tauri-apps/api/event";
  import { onMount, onDestroy } from "svelte";

  export let state: "idle" | "recording" | "processing" | "error" = "idle";
  export let errorMsg = "";
  export let lastTranscription = "";
  export let streamingText = "";
  export let audioLevels: number[] = [];
  export let shortcut = "Meta+Shift+Space";

  let outputEl: HTMLDivElement;
  let editedText = "";
  let prevTranscription = "";
  let copyFeedback = false;

  // Parse shortcut string → display badges
  function parseShortcut(s: string): string[] {
    return s.split("+").map((part) => {
      switch (part.trim()) {
        case "Meta":
        case "Super":
        case "Cmd":
          return "⌘";
        case "Shift":
          return "⇧";
        case "Ctrl":
        case "Control":
          return "⌃";
        case "Alt":
        case "Option":
          return "⌥";
        default:
          return part;
      }
    });
  }

  $: shortcutKeys = parseShortcut(shortcut);

  // Direct Tauri event listener — bypasses Svelte's prop/reactivity batching
  // so each streaming token updates the DOM immediately as it arrives.
  let unlistenStream: UnlistenFn | null = null;

  onMount(async () => {
    console.log("[MainWindow] onMount — setting up stream listener, outputEl:", !!outputEl);
    unlistenStream = await listen<string>("transcription-stream", (e) => {
      console.log("[MainWindow] stream token:", JSON.stringify(e.payload), "outputEl:", !!outputEl);
      if (outputEl) {
        outputEl.textContent = (outputEl.textContent ?? "") + e.payload;
      }
    });
    console.log("[MainWindow] stream listener ready");
  });

  onDestroy(() => {
    unlistenStream?.();
  });

  // Clear output when a new recording starts.
  $: if (state === "recording") {
    editedText = "";
    prevTranscription = "";
    if (outputEl) outputEl.textContent = "";
  }

  // Final transcription (idle state) — overwrites any leftover streaming text.
  $: if (lastTranscription && lastTranscription !== prevTranscription && state === "idle") {
    editedText = lastTranscription;
    prevTranscription = lastTranscription;
    if (outputEl) outputEl.textContent = lastTranscription;
  }

  $: hasResult = state === "idle" && editedText !== "";

  $: hotkeyHint =
    state === "recording"
      ? "Release to stop"
      : state === "processing"
        ? "Transcribing…"
        : state === "error"
          ? "Error"
          : "Press to record";

  function barHeight(level: number): number {
    const MIN_H = 3;
    const MAX_H = 20;
    return Math.round(MIN_H + level * (MAX_H - MIN_H));
  }

  async function handleCopy() {
    const text = outputEl?.textContent ?? editedText;
    try {
      await invoke("copy_to_clipboard", { text });
      copyFeedback = true;
      setTimeout(() => (copyFeedback = false), 1500);
    } catch (e) {
      console.error("copy_to_clipboard failed", e);
    }
  }

  async function handleRelease() {
    const text = outputEl?.textContent ?? editedText;
    try {
      await invoke("release_text", { text });
      editedText = "";
      prevTranscription = "";
      if (outputEl) outputEl.textContent = "";
    } catch (e) {
      console.error("release_text failed", e);
    }
  }
</script>

<div class="main-window">
  <!-- Hotkey row -->
  <div class="row">
    <span class="row-label">Hotkey</span>
    <div class="row-center">
      {#each shortcutKeys as key}
        <span class="key-badge">{key}</span>
      {/each}
    </div>
    <span class="row-hint" class:hint-active={state === "recording" || state === "processing"}>
      {hotkeyHint}
    </span>
  </div>

  <div class="divider"></div>

  <!-- Recording row -->
  <div class="row">
    <span class="row-label">Recording</span>
    <div class="row-center recording-center">
      {#if state === "recording"}
        <div class="dot-wrap">
          <div class="ring"></div>
          <div class="dot"></div>
        </div>
        <div class="waveform" aria-hidden="true">
          {#each audioLevels as level}
            <div class="wave-bar" style="height: {barHeight(level)}px"></div>
          {/each}
        </div>
      {:else if state === "processing"}
        <div class="spinner"></div>
        <span class="processing-label">Transcribing…</span>
      {:else if state === "error"}
        <div class="err-dot"></div>
        <span class="error-label">{errorMsg || "Error"}</span>
      {/if}
    </div>
    <span class="row-hint hint-right">
      {#if state === "recording"}Release to send{/if}
    </span>
  </div>

  <div class="divider"></div>

  <!-- Output row -->
  <div class="row output-row">
    <span class="row-label">Output</span>
    <div class="output-area">
      <!-- svelte-ignore a11y-interactive-supports-focus -->
      <div
        class="output-text"
        class:output-editable={hasResult}
        contenteditable={hasResult ? "true" : "false"}
        bind:this={outputEl}
        role="textbox"
        aria-label="Transcription output"
        aria-multiline="true"
      ></div>
      {#if hasResult}
        <div class="output-actions">
          <button class="action-btn" on:click={handleCopy}>
            {copyFeedback ? "Copied!" : "Copy"}
          </button>
          <button class="action-btn primary" on:click={handleRelease}>Release</button>
        </div>
      {/if}
    </div>
  </div>
</div>

<style>
  .main-window {
    width: 100%;
    height: 100%;
    background: #1a1a1c;
    display: flex;
    flex-direction: column;
    padding: 0 20px;
    gap: 0;
    user-select: none;
    font-family: -apple-system, "SF Pro Text", BlinkMacSystemFont, sans-serif;
    -webkit-font-smoothing: antialiased;
  }

  .row {
    display: flex;
    align-items: center;
    min-height: 52px;
    gap: 12px;
  }

  .output-row {
    align-items: flex-start;
    padding-top: 10px;
    padding-bottom: 10px;
    flex: 1;
  }

  .row-label {
    font-size: 12px;
    font-weight: 500;
    color: rgba(255, 255, 255, 0.3);
    width: 72px;
    flex-shrink: 0;
    letter-spacing: 0.01em;
  }

  .row-center {
    display: flex;
    align-items: center;
    gap: 6px;
    flex: 1;
  }

  .recording-center {
    gap: 8px;
  }

  .row-hint {
    font-size: 12px;
    color: rgba(255, 255, 255, 0.28);
    white-space: nowrap;
    font-weight: 400;
    transition: color 0.2s;
  }

  .row-hint.hint-active {
    color: rgba(255, 255, 255, 0.5);
  }

  .hint-right {
    color: rgba(255, 255, 255, 0.4);
  }

  .divider {
    height: 1px;
    background: rgba(255, 255, 255, 0.07);
    flex-shrink: 0;
  }

  /* Key badges */
  .key-badge {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-width: 28px;
    height: 26px;
    padding: 0 7px;
    border-radius: 6px;
    background: rgba(255, 255, 255, 0.07);
    border: 1px solid rgba(255, 255, 255, 0.14);
    color: rgba(255, 255, 255, 0.75);
    font-size: 13px;
    font-weight: 500;
    letter-spacing: 0.01em;
    box-shadow: 0 1px 0 rgba(0, 0, 0, 0.3);
  }

  /* Red recording dot + pulse ring */
  .dot-wrap {
    position: relative;
    width: 16px;
    height: 16px;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    overflow: visible;
  }

  .dot {
    width: 9px;
    height: 9px;
    border-radius: 50%;
    background: #ff2d20;
    position: relative;
    z-index: 2;
    box-shadow: 0 0 8px 3px rgba(255, 45, 32, 0.7);
    animation: blink 1.6s ease-in-out infinite;
  }

  .ring {
    position: absolute;
    inset: -20px;
    border-radius: 50%;
    background: rgba(255, 45, 32, 0.35);
    animation: expand 1.6s ease-out infinite;
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
      transform: scale(0.15);
      opacity: 0.8;
    }
    100% {
      transform: scale(1);
      opacity: 0;
    }
  }

  /* Waveform */
  .waveform {
    display: flex;
    align-items: center;
    gap: 2px;
    height: 22px;
  }

  .wave-bar {
    width: 2.5px;
    border-radius: 1.5px;
    background: linear-gradient(
      to top,
      rgba(129, 140, 248, 0.5),
      rgba(167, 139, 250, 0.9)
    );
    flex-shrink: 0;
    transition: height 0.08s ease-out;
    min-height: 3px;
  }

  /* Processing spinner */
  .spinner {
    width: 14px;
    height: 14px;
    border-radius: 50%;
    border: 2px solid rgba(129, 140, 248, 0.15);
    border-top-color: #818cf8;
    animation: spin 0.8s linear infinite;
    flex-shrink: 0;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  .processing-label {
    font-size: 13px;
    color: rgba(129, 140, 248, 0.85);
  }

  /* Error dot */
  .err-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: #f87171;
    flex-shrink: 0;
  }

  .error-label {
    font-size: 13px;
    color: #f87171;
  }

  /* Output area */
  .output-area {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-height: 0;
  }

  .output-text {
    font-size: 14px;
    font-weight: 400;
    color: rgba(255, 255, 255, 0.82);
    line-height: 1.55;
    outline: none;
    border: none;
    background: transparent;
    width: 100%;
    word-break: break-word;
    white-space: pre-wrap;
    min-height: 22px;
    max-height: 56px;
    overflow-y: auto;
    cursor: default;
    caret-color: rgba(99, 130, 246, 0.9);
  }

  .output-text.output-editable {
    cursor: text;
    user-select: text;
  }

  .output-text:empty::before {
    content: "";
    display: block;
  }

  /* Action buttons */
  .output-actions {
    display: flex;
    gap: 6px;
    justify-content: flex-end;
  }

  .action-btn {
    padding: 4px 12px;
    border-radius: 6px;
    border: 1px solid rgba(255, 255, 255, 0.14);
    background: rgba(255, 255, 255, 0.07);
    color: rgba(255, 255, 255, 0.7);
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
    font-family: -apple-system, "SF Pro Text", BlinkMacSystemFont, sans-serif;
    transition:
      background 0.12s,
      color 0.12s;
    -webkit-font-smoothing: antialiased;
  }

  .action-btn:hover {
    background: rgba(255, 255, 255, 0.12);
    color: rgba(255, 255, 255, 0.9);
  }

  .action-btn.primary {
    background: rgba(99, 130, 246, 0.2);
    border-color: rgba(99, 130, 246, 0.45);
    color: rgba(165, 180, 252, 0.95);
  }

  .action-btn.primary:hover {
    background: rgba(99, 130, 246, 0.32);
  }
</style>
