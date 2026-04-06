<script lang="ts">
  import { createEventDispatcher, onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { getCurrentWindow } from "@tauri-apps/api/window";

  const isWindows = navigator.userAgent.includes("Windows");

  const dispatch = createEventDispatcher();

  export let polishEnabled: boolean = true;
  export let audioDevices: string[] = [];
  export let autostartEnabled: boolean = false;
  export let screenshotContextEnabled: boolean = false;
  export let appState: string = "idle";

  let provider: "groq" | "vertex_ai" = "groq";
  let apiKey = "";
  let gcpProjectId = "";
  let gcpLocation = "us-central1";
  let vertexModel = "gemini-2.5-flash";
  let adcAvailable = false;

  let preferredDevice: string | null = null;
  let shortcut = "Meta+Shift+Space";
  let saving = false;
  let saved = false;
  let error = "";
  let opacity = 1.0;

  const vertexModels = [
    { value: "gemini-2.5-flash", label: "Gemini 2.5 Flash" },
    { value: "gemini-2.5-pro", label: "Gemini 2.5 Pro" },
    { value: "gemini-2.0-flash", label: "Gemini 2.0 Flash" },
  ];

  onMount(async () => {
    provider = (await invoke<string>("get_provider")) as typeof provider;
    apiKey = await invoke<string>("get_saved_api_key");
    shortcut = await invoke<string>("get_shortcut");
    const cfg = await invoke<string | null>("get_preferred_device").catch(() => null);
    preferredDevice = cfg;
    const savedOpacity = localStorage.getItem("window-opacity");
    if (savedOpacity) {
      opacity = parseFloat(savedOpacity);
      await getCurrentWindow().setOpacity(opacity);
    }
    const vc = await invoke<{ project_id: string; location: string; model: string }>("get_vertex_config");
    gcpProjectId = vc.project_id;
    gcpLocation = vc.location || "us-central1";
    vertexModel = vc.model || "gemini-2.5-flash";
    adcAvailable = await invoke<boolean>("check_vertex_auth").catch(() => false);
  });

  async function handleOpacityChange(e: Event) {
    opacity = parseFloat((e.target as HTMLInputElement).value);
    localStorage.setItem("window-opacity", String(opacity));
    await getCurrentWindow().setOpacity(opacity);
  }

  async function handleProviderSwitch(p: typeof provider) {
    provider = p;
    await invoke("save_provider", { provider: p });
    showSaved();
  }

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

  async function handleSaveVertexConfig() {
    if (!gcpProjectId.trim()) {
      error = "项目 ID 不能为空";
      return;
    }
    saving = true;
    error = "";
    try {
      await invoke("save_vertex_config", {
        projectId: gcpProjectId.trim(),
        location: gcpLocation.trim() || "us-central1",
        model: vertexModel,
      });
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

  async function handleScreenshotContextToggle() {
    screenshotContextEnabled = !screenshotContextEnabled;
    await invoke("save_screenshot_context_enabled", { enabled: screenshotContextEnabled });
  }

  function showSaved() {
    saved = true;
    setTimeout(() => { saved = false; }, 1800);
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") dispatch("close");
  }

  async function handleHeaderMousedown(e: MouseEvent) {
    if ((e.target as HTMLElement).closest("button")) return;
    await getCurrentWindow().startDragging();
  }
</script>

<svelte:window on:keydown={handleKeydown} />

<div class="settings-panel">
  <!-- Header -->
  <!-- svelte-ignore a11y-no-noninteractive-element-interactions -->
  <div class="header" role="toolbar" aria-label="设置标题栏" on:mousedown={handleHeaderMousedown}>
    <span class="title">设置</span>
    {#if appState === "recording"}
      <div class="rec-badge">
        <div class="rec-dot"></div>
        <span>录音中</span>
      </div>
    {:else if appState === "processing"}
      <div class="rec-badge processing">
        <div class="proc-spinner"></div>
        <span>转录中</span>
      </div>
    {/if}
    <button class="close-btn" on:click={() => dispatch("close")}>
      <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
        <path d="M1 1l10 10M11 1L1 11" stroke="rgba(255,255,255,0.5)" stroke-width="1.8" stroke-linecap="round"/>
      </svg>
    </button>
  </div>

  <div class="sections">
    <!-- Provider selector -->
    <div class="section">
      <label class="section-label">语音服务</label>
      <div class="provider-tabs">
        <button
          class="provider-tab"
          class:active={provider === "groq"}
          on:click={() => handleProviderSwitch("groq")}
        >
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none">
            <path d="M13 2L3 14h9l-1 8 10-12h-9l1-8z" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
          </svg>
          Groq
        </button>
        <button
          class="provider-tab"
          class:active={provider === "vertex_ai"}
          on:click={() => handleProviderSwitch("vertex_ai")}
        >
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none">
            <path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z" stroke="currentColor" stroke-width="1.8" stroke-linejoin="round"/>
            <polyline points="3.27 6.96 12 12.01 20.73 6.96" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/>
            <line x1="12" y1="22.08" x2="12" y2="12" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"/>
          </svg>
          Vertex AI
        </button>
      </div>
    </div>

    <!-- Groq config -->
    {#if provider === "groq"}
      <div class="section provider-config">
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
    {/if}

    <!-- Vertex AI config -->
    {#if provider === "vertex_ai"}
      <div class="section provider-config">
        <div class="vertex-fields">
          <div class="field">
            <label class="field-label">GCP 项目 ID</label>
            <input
              type="text"
              placeholder="my-project-id"
              bind:value={gcpProjectId}
              class="text-input"
              autocomplete="off"
              spellcheck="false"
            />
          </div>
          <div class="field-row">
            <div class="field flex1">
              <label class="field-label">区域</label>
              <input
                type="text"
                placeholder="us-central1"
                bind:value={gcpLocation}
                class="text-input mono"
                autocomplete="off"
                spellcheck="false"
              />
            </div>
            <div class="field flex1">
              <label class="field-label">模型</label>
              <select class="select-input" bind:value={vertexModel}>
                {#each vertexModels as m}
                  <option value={m.value}>{m.label}</option>
                {/each}
              </select>
            </div>
          </div>
          <button class="action-btn full-width" on:click={handleSaveVertexConfig} disabled={saving}>
            {saving ? "保存中..." : "保存配置"}
          </button>
        </div>
        <div class="adc-status" class:ok={adcAvailable}>
          <div class="adc-dot"></div>
          <span>
            {#if adcAvailable}
              gcloud 凭证已就绪
            {:else}
              未检测到 gcloud 凭证
            {/if}
          </span>
        </div>
        {#if !adcAvailable}
          <p class="hint">请运行 <code>gcloud auth application-default login</code></p>
        {/if}
      </div>
    {/if}

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

    <div class="divider"></div>

    <!-- Screenshot context toggle -->
    <div class="section row-section">
      <div class="row-label-block">
        <span class="section-label">截图上下文</span>
        <span class="row-desc">录音时截屏，提升润色准确度</span>
      </div>
      <button
        class="toggle"
        class:on={screenshotContextEnabled}
        on:click={handleScreenshotContextToggle}
        aria-label="切换截图上下文"
        disabled={!polishEnabled}
      >
        <div class="toggle-knob"></div>
      </button>
    </div>

    <div class="divider"></div>

    <!-- Opacity -->
    <div class="section">
      <div class="row-label-block">
        <span class="section-label">窗口不透明度</span>
        <span class="row-desc">{Math.round(opacity * 100)}%</span>
      </div>
      <input
        type="range"
        min="0.2"
        max="1"
        step="0.05"
        value={opacity}
        on:input={handleOpacityChange}
        class="opacity-slider"
      />
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
    border-radius: 16px;
    box-shadow: 0 8px 40px rgba(0, 0, 0, 0.5);
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
    cursor: grab;
  }

  .header:active {
    cursor: grabbing;
  }

  .title {
    font-size: 14px;
    font-weight: 600;
    color: rgba(255,255,255,0.88);
    letter-spacing: 0.01em;
  }

  .rec-badge {
    display: flex;
    align-items: center;
    gap: 5px;
    padding: 3px 9px;
    border-radius: 999px;
    background: rgba(239, 68, 68, 0.15);
    border: 1px solid rgba(239, 68, 68, 0.3);
    font-size: 11px;
    color: #f87171;
    font-weight: 500;
    margin-left: auto;
    margin-right: 8px;
  }

  .rec-badge.processing {
    background: rgba(99, 130, 246, 0.12);
    border-color: rgba(99, 130, 246, 0.25);
    color: #818cf8;
  }

  .rec-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: #ef4444;
    animation: blink 1.4s ease-in-out infinite;
  }

  .proc-spinner {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    border: 1.5px solid rgba(99, 130, 246, 0.2);
    border-top-color: #818cf8;
    animation: spin 0.8s linear infinite;
  }

  @keyframes blink { 0%,100%{opacity:1} 50%{opacity:.5} }
  @keyframes spin { to { transform: rotate(360deg); } }

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
    overflow-y: auto;
    max-height: calc(100vh - 56px);
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

  .hint code {
    font-family: "SF Mono", "Fira Code", monospace;
    font-size: 10px;
    background: rgba(255,255,255,0.06);
    padding: 1px 4px;
    border-radius: 3px;
    color: rgba(255,255,255,0.5);
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

  .action-btn.full-width {
    width: 100%;
  }

  /* Provider tabs */
  .provider-tabs {
    display: flex;
    gap: 0;
    background: rgba(255,255,255,0.04);
    border-radius: 10px;
    padding: 3px;
    border: 1px solid rgba(255,255,255,0.06);
  }

  .provider-tab {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    padding: 7px 12px;
    border: none;
    border-radius: 8px;
    background: transparent;
    color: rgba(255,255,255,0.4);
    font-size: 12.5px;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.2s ease;
    font-family: -apple-system, "SF Pro Text", BlinkMacSystemFont, sans-serif;
  }

  .provider-tab:hover {
    color: rgba(255,255,255,0.6);
  }

  .provider-tab.active {
    background: rgba(99, 102, 241, 0.2);
    color: rgba(165, 180, 252, 0.95);
    box-shadow: 0 1px 3px rgba(0,0,0,0.15);
  }

  /* Provider config area */
  .provider-config {
    padding-top: 6px;
  }

  /* Vertex AI specific */
  .vertex-fields {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .field-label {
    font-size: 11px;
    color: rgba(255,255,255,0.4);
    font-weight: 500;
  }

  .field-row {
    display: flex;
    gap: 8px;
  }

  .flex1 {
    flex: 1;
    min-width: 0;
  }

  .adc-status {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 11px;
    color: rgba(248, 113, 113, 0.8);
    padding: 6px 10px;
    border-radius: 8px;
    background: rgba(248, 113, 113, 0.06);
    border: 1px solid rgba(248, 113, 113, 0.12);
  }

  .adc-status.ok {
    color: rgba(134, 239, 172, 0.85);
    background: rgba(74, 222, 128, 0.06);
    border-color: rgba(74, 222, 128, 0.15);
  }

  .adc-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: rgba(248, 113, 113, 0.7);
    flex-shrink: 0;
  }

  .adc-status.ok .adc-dot {
    background: rgba(74, 222, 128, 0.7);
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

  .opacity-slider {
    width: 100%;
    accent-color: rgba(99, 102, 241, 0.85);
    cursor: pointer;
  }
</style>
