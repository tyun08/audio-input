<script lang="ts">
  import { createEventDispatcher, onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";

  const dispatch = createEventDispatcher();

  export let polishEnabled: boolean = true;
  export let audioDevices: string[] = [];
  export let autostartEnabled: boolean = false;

  let apiKey = "";
  let preferredDevice: string | null = null;
  let shortcut = "Meta+Shift+Space";
  let saving = false;
  let saved = false;
  let error = "";

  onMount(async () => {
    apiKey = await invoke<string>("get_saved_api_key");
    shortcut = await invoke<string>("get_shortcut");
    const cfg = await invoke<string | null>("get_preferred_device").catch(() => null);
    preferredDevice = cfg;
  });

  async function handleSaveApiKey() {
    if (!apiKey.trim()) {
      error = "API Key 不能为空";
      return;
    }
    saving = true;
    error = "";
    try {
      await invoke("save_api_key", { key: apiKey.trim() });
      showSaved();
    } catch (e) {
      error = String(e);
    } finally {
      saving = false;
    }
  }

  async function handlePolishToggle() {
    polishEnabled = !polishEnabled;
    await invoke("save_polish_enabled", { enabled: polishEnabled });
  }

  async function handleDeviceChange(e: Event) {
    const val = (e.target as HTMLSelectElement).value;
    preferredDevice = val === "__default__" ? null : val;
    await invoke("save_preferred_device", { device: preferredDevice });
  }

  async function handleShortcutChange() {
    try {
      await invoke("save_shortcut", { shortcut });
      showSaved();
    } catch (e) {
      error = String(e);
    }
  }

  async function handleAutostartToggle() {
    autostartEnabled = !autostartEnabled;
    await invoke("save_autostart_enabled", { enabled: autostartEnabled });
  }

  function showSaved() {
    saved = true;
    setTimeout(() => { saved = false; }, 1800);
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") dispatch("close");
  }

  // Expose get_preferred_device as a stub — it's done via onMount
  async function getPreferredDevice(): Promise<string | null> {
    try {
      const cfg = await invoke<{preferred_device: string | null}>("get_config");
      return cfg.preferred_device;
    } catch {
      return null;
    }
  }
</script>

<svelte:window on:keydown={handleKeydown} />

<div class="settings-panel">
  <!-- Header -->
  <div class="header">
    <span class="title">设置</span>
    <button class="close-btn" on:click={() => dispatch("close")}>
      <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
        <path d="M1 1l10 10M11 1L1 11" stroke="rgba(255,255,255,0.5)" stroke-width="1.8" stroke-linecap="round"/>
      </svg>
    </button>
  </div>

  <div class="sections">
    <!-- API Key -->
    <div class="section">
      <label class="section-label">Groq API Key</label>
      <div class="input-row">
        <input
          type="password"
          placeholder="gsk_..."
          bind:value={apiKey}
          on:keydown={(e) => e.key === "Enter" && handleSaveApiKey()}
          autocomplete="off"
          spellcheck="false"
          class="text-input"
        />
        <button class="action-btn" on:click={handleSaveApiKey} disabled={saving}>
          {saving ? "..." : "保存"}
        </button>
      </div>
      <p class="hint">
        在 <a href="https://console.groq.com" target="_blank" rel="noopener">console.groq.com</a> 免费获取
      </p>
    </div>

    <div class="divider"></div>

    <!-- Polish toggle -->
    <div class="section row-section">
      <div class="row-label-block">
        <span class="section-label">AI 润色</span>
        <span class="row-desc">自动添加标点、修正错字</span>
      </div>
      <button
        class="toggle"
        class:on={polishEnabled}
        on:click={handlePolishToggle}
        aria-label="切换润色"
      >
        <div class="toggle-knob"></div>
      </button>
    </div>

    <div class="divider"></div>

    <!-- Microphone selection -->
    <div class="section">
      <label class="section-label">麦克风</label>
      <select class="select-input" on:change={handleDeviceChange} value={preferredDevice ?? "__default__"}>
        <option value="__default__">系统默认</option>
        {#each audioDevices as device}
          <option value={device}>{device}</option>
        {/each}
      </select>
    </div>

    <div class="divider"></div>

    <!-- Shortcut -->
    <div class="section">
      <label class="section-label">全局快捷键</label>
      <div class="input-row">
        <input
          type="text"
          bind:value={shortcut}
          class="text-input mono"
          placeholder="Meta+Shift+Space"
        />
        <button class="action-btn" on:click={handleShortcutChange}>应用</button>
      </div>
      <p class="hint">Meta = ⌘，Ctrl，Alt，Shift</p>
    </div>

    <div class="divider"></div>

    <!-- Autostart toggle -->
    <div class="section row-section">
      <div class="row-label-block">
        <span class="section-label">开机自启</span>
        <span class="row-desc">登录时自动启动</span>
      </div>
      <button
        class="toggle"
        class:on={autostartEnabled}
        on:click={handleAutostartToggle}
        aria-label="切换开机自启"
      >
        <div class="toggle-knob"></div>
      </button>
    </div>
  </div>

  {#if error}
    <div class="error-banner">{error}</div>
  {/if}

  {#if saved}
    <div class="saved-banner">已保存</div>
  {/if}
</div>

<style>
  .settings-panel {
    width: 320px;
    background: rgba(30, 30, 32, 0.92);
    backdrop-filter: blur(20px) saturate(180%);
    -webkit-backdrop-filter: blur(20px) saturate(180%);
    border: 1px solid rgba(255, 255, 255, 0.12);
    border-radius: 16px;
    box-shadow: 0 8px 40px rgba(0, 0, 0, 0.5), 0 1px 0 rgba(255,255,255,0.06) inset;
    overflow: hidden;
    font-family: -apple-system, "SF Pro Text", BlinkMacSystemFont, sans-serif;
    -webkit-font-smoothing: antialiased;
  }

  .header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 14px 16px 12px;
    border-bottom: 1px solid rgba(255,255,255,0.08);
  }

  .title {
    font-size: 14px;
    font-weight: 600;
    color: rgba(255,255,255,0.88);
    letter-spacing: 0.01em;
  }

  .close-btn {
    background: rgba(255,255,255,0.08);
    border: none;
    border-radius: 50%;
    width: 22px;
    height: 22px;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    transition: background 0.15s;
    padding: 0;
  }

  .close-btn:hover {
    background: rgba(255,255,255,0.15);
  }

  .sections {
    padding: 4px 0;
  }

  .section {
    padding: 11px 16px;
    display: flex;
    flex-direction: column;
    gap: 7px;
  }

  .section.row-section {
    flex-direction: row;
    align-items: center;
    justify-content: space-between;
  }

  .section-label {
    font-size: 13px;
    font-weight: 500;
    color: rgba(255,255,255,0.8);
    letter-spacing: 0.01em;
  }

  .row-label-block {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .row-desc {
    font-size: 11px;
    color: rgba(255,255,255,0.35);
  }

  .hint {
    font-size: 11px;
    color: rgba(255,255,255,0.35);
    line-height: 1.5;
    margin: 0;
  }

  .hint a {
    color: rgba(129, 140, 248, 0.85);
    text-decoration: none;
  }

  .divider {
    height: 1px;
    background: rgba(255,255,255,0.07);
    margin: 0 16px;
  }

  .input-row {
    display: flex;
    gap: 8px;
  }

  .text-input {
    flex: 1;
    padding: 7px 10px;
    border-radius: 8px;
    border: 1px solid rgba(255,255,255,0.1);
    background: rgba(255,255,255,0.05);
    color: rgba(255,255,255,0.88);
    font-size: 13px;
    font-family: -apple-system, "SF Pro Text", BlinkMacSystemFont, sans-serif;
    outline: none;
    transition: border-color 0.15s;
    min-width: 0;
  }

  .text-input.mono {
    font-family: "SF Mono", "Fira Code", monospace;
    font-size: 12px;
  }

  .text-input:focus {
    border-color: rgba(129, 140, 248, 0.5);
  }

  .text-input::placeholder {
    color: rgba(255,255,255,0.2);
  }

  .select-input {
    width: 100%;
    padding: 7px 10px;
    border-radius: 8px;
    border: 1px solid rgba(255,255,255,0.1);
    background: rgba(255,255,255,0.05);
    color: rgba(255,255,255,0.88);
    font-size: 13px;
    font-family: -apple-system, "SF Pro Text", BlinkMacSystemFont, sans-serif;
    outline: none;
    cursor: pointer;
    appearance: auto;
  }

  .action-btn {
    padding: 7px 14px;
    border-radius: 8px;
    border: none;
    background: rgba(99, 102, 241, 0.75);
    color: white;
    font-size: 13px;
    font-weight: 500;
    cursor: pointer;
    transition: background 0.15s;
    white-space: nowrap;
    flex-shrink: 0;
  }

  .action-btn:hover:not(:disabled) {
    background: rgba(99, 102, 241, 0.9);
  }

  .action-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  /* Toggle switch */
  .toggle {
    position: relative;
    width: 40px;
    height: 24px;
    border-radius: 12px;
    border: none;
    background: rgba(255,255,255,0.1);
    cursor: pointer;
    transition: background 0.2s;
    flex-shrink: 0;
    padding: 0;
  }

  .toggle.on {
    background: rgba(99, 102, 241, 0.85);
  }

  .toggle-knob {
    position: absolute;
    top: 3px;
    left: 3px;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    background: white;
    box-shadow: 0 1px 3px rgba(0,0,0,0.3);
    transition: transform 0.2s cubic-bezier(0.4, 0, 0.2, 1);
  }

  .toggle.on .toggle-knob {
    transform: translateX(16px);
  }

  /* Status banners */
  .error-banner {
    margin: 0 16px 12px;
    padding: 7px 10px;
    border-radius: 8px;
    background: rgba(248, 113, 113, 0.12);
    border: 1px solid rgba(248, 113, 113, 0.25);
    font-size: 12px;
    color: #f87171;
  }

  .saved-banner {
    margin: 0 16px 12px;
    padding: 7px 10px;
    border-radius: 8px;
    background: rgba(74, 222, 128, 0.1);
    border: 1px solid rgba(74, 222, 128, 0.25);
    font-size: 12px;
    color: rgba(134, 239, 172, 0.9);
    text-align: center;
  }
</style>
