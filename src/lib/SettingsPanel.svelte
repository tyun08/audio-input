<script lang="ts">
  import { createEventDispatcher, onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { providers, getProvider } from "./providers";
  import { t, locale } from "./i18n";
  import FieldSelect from "./FieldSelect.svelte";

  const dispatch = createEventDispatcher();

  export let polishEnabled: boolean = true;
  export let audioDevices: string[] = [];
  export let autostartEnabled: boolean = false;
  export let screenshotContextEnabled: boolean = false;
  export let appState: string = "idle";
  export let shortcutConflict: string = "";

  let activeSection: "general" | "transcription" | "advanced" = "transcription";
  let provider = "openai";
  let configValues: Record<string, string> = {};
  let authStatus: boolean | null = null;

  let preferredDevice: string | null = null;
  let shortcut = "Meta+Shift+Space";
  let saving = false;
  let saved = false;
  let error = "";

  $: currentProvider = getProvider(provider);

  onMount(async () => {
    provider = await invoke<string>("get_provider");
    await loadProviderConfig();
    shortcut = await invoke<string>("get_shortcut");
    preferredDevice = await invoke<string | null>("get_preferred_device").catch(() => null);
  });

  async function loadProviderConfig() {
    const raw = await invoke<Record<string, string>>("get_provider_config", { provider });
    configValues = raw ?? {};
    const cp = getProvider(provider);
    if (cp?.authCheck) {
      authStatus = await invoke<boolean>(cp.authCheck, { provider }).catch(() => false);
    } else {
      authStatus = null;
    }
  }

  async function handleProviderSwitch(id: string) {
    provider = id;
    await invoke("save_provider", { provider: id });
    await loadProviderConfig();
  }

  async function handleSaveConfig() {
    saving = true;
    error = "";
    try {
      await invoke("save_provider_config", { provider, configValues });
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
    error = "";
    try {
      await invoke("save_shortcut", { shortcut });
      shortcutConflict = "";
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

  function handleProviderSelectChange(e: Event) {
    handleProviderSwitch((e.target as HTMLSelectElement).value);
  }

  function showSaved() {
    saved = true;
    setTimeout(() => { saved = false; }, 1800);
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") dispatch("close");
  }

  async function handleTitlebarMousedown(e: MouseEvent) {
    if ((e.target as HTMLElement).closest("button")) return;
    await getCurrentWindow().startDragging();
  }
</script>

<svelte:window on:keydown={handleKeydown} />

<div class="settings-root">
  <!-- Title bar -->
  <!-- svelte-ignore a11y-no-noninteractive-element-interactions -->
  <div class="titlebar" role="toolbar" on:mousedown={handleTitlebarMousedown}>
    <button class="close-btn" on:click={() => dispatch("close")} aria-label="Close">
      <svg width="8" height="8" viewBox="0 0 8 8"><path d="M1 1l6 6M7 1L1 7" stroke="rgba(0,0,0,0.5)" stroke-width="1.5" stroke-linecap="round"/></svg>
    </button>
    <span class="titlebar-label">{$t('settings.title')}</span>
    {#if appState === "recording"}
      <span class="status-pill recording">{$t('settings.recording')}</span>
    {:else if appState === "processing"}
      <span class="status-pill processing">{$t('settings.transcribing')}</span>
    {/if}
  </div>

  <div class="layout">
    <!-- Sidebar -->
    <nav class="sidebar">
      <button
        class="nav-item"
        class:active={activeSection === "transcription"}
        on:click={() => activeSection = "transcription"}
      >
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none">
          <rect x="3" y="3" width="18" height="18" rx="3" stroke="currentColor" stroke-width="1.8"/>
          <path d="M8 12h8M8 8h5M8 16h6" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"/>
        </svg>
        {$t('settings.nav.transcription')}
      </button>
      <button
        class="nav-item"
        class:active={activeSection === "general"}
        on:click={() => activeSection = "general"}
      >
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none">
          <circle cx="12" cy="12" r="3" stroke="currentColor" stroke-width="1.8"/>
          <path d="M12 2v2M12 20v2M4.22 4.22l1.42 1.42M18.36 18.36l1.42 1.42M2 12h2M20 12h2M4.22 19.78l1.42-1.42M18.36 5.64l1.42-1.42" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"/>
        </svg>
        {$t('settings.nav.general')}
      </button>
      <button
        class="nav-item"
        class:active={activeSection === "advanced"}
        on:click={() => activeSection = "advanced"}
      >
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none">
          <path d="M12 2L2 7l10 5 10-5-10-5zM2 17l10 5 10-5M2 12l10 5 10-5" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/>
        </svg>
        {$t('settings.nav.advanced')}
      </button>
    </nav>

    <!-- Content -->
    <main class="content">

      <!-- ── Transcription ── -->
      {#if activeSection === "transcription"}
        <h2>{$t('settings.nav.transcription')}</h2>

        <div class="group">
          <div class="row">
            <span class="row-label">{$t('settings.voice_service')}</span>
            <select
              class="row-select"
              value={provider}
              on:change={handleProviderSelectChange}
            >
              {#each providers as p}
                <option value={p.id}>{p.name}</option>
              {/each}
            </select>
          </div>

          {#if currentProvider}
            {#each currentProvider.fields as field}
              <div class="row-sep"></div>
              <div class="row">
                <span class="row-label">{field.label[$locale]}</span>
                {#if field.type === "select"}
                  <FieldSelect options={field.options ?? []} bind:value={configValues[field.key]} />
                {:else if field.type === "password"}
                  <input
                    type="password"
                    class="row-input"
                    placeholder={field.placeholder ?? ""}
                    bind:value={configValues[field.key]}
                    on:keydown={(e) => e.key === "Enter" && handleSaveConfig()}
                    autocomplete="off"
                    spellcheck="false"
                  />
                {:else}
                  <input
                    type="text"
                    class="row-input"
                    class:mono={field.mono}
                    placeholder={field.placeholder ?? ""}
                    bind:value={configValues[field.key]}
                    on:keydown={(e) => e.key === "Enter" && handleSaveConfig()}
                    autocomplete="off"
                    spellcheck="false"
                  />
                {/if}
              </div>
            {/each}
          {/if}
        </div>

        <div class="action-row">
          <button class="save-btn" on:click={handleSaveConfig} disabled={saving}>
            {saving ? $t('settings.saving') : saved ? $t('settings.saved') : $t('settings.save')}
          </button>
          {#if error}
            <span class="inline-error">{error}</span>
          {/if}
        </div>

        {#if authStatus !== null && currentProvider?.authOkText}
          <div class="auth-badge" class:ok={authStatus}>
            <span class="auth-dot"></span>
            {authStatus ? currentProvider.authOkText[$locale] : (currentProvider.authFailText ?? currentProvider.authOkText)[$locale]}
          </div>
        {/if}

        {#if currentProvider?.hint}
          <p class="hint">{@html currentProvider.hint[$locale]}</p>
        {/if}

      <!-- ── General ── -->
      {:else if activeSection === "general"}
        <h2>{$t('settings.nav.general')}</h2>

        <h3>{$t('settings.section.startup')}</h3>
        <div class="group">
          <div class="row">
            <span class="row-label">{$t('settings.autostart')}</span>
            <button class="toggle" class:on={autostartEnabled} on:click={handleAutostartToggle} aria-label="Toggle autostart">
              <span class="toggle-knob"></span>
            </button>
          </div>
        </div>

        <h3>{$t('settings.section.input')}</h3>
        <div class="group">
          <div class="row">
            <span class="row-label">{$t('settings.mic')}</span>
            <select class="row-select" on:change={handleDeviceChange} value={preferredDevice ?? "__default__"}>
              <option value="__default__">{$t('settings.mic_default')}</option>
              {#each audioDevices as device}
                <option value={device}>{device}</option>
              {/each}
            </select>
          </div>
          <div class="row-sep"></div>
          <div class="row">
            <span class="row-label">{$t('settings.shortcut')}</span>
            <div class="row-input-group">
              <input type="text" bind:value={shortcut} class="row-input mono" placeholder="Meta+Shift+Space" />
              <button class="apply-btn" on:click={handleShortcutChange}>{$t('settings.shortcut_apply')}</button>
            </div>
          </div>
        </div>
        {#if shortcutConflict}
          <p class="warn">{$t('settings.shortcut_conflict', shortcutConflict)}</p>
        {/if}
        <p class="hint">{$t('settings.shortcut_hint')}</p>

        <h3>{$t('settings.section.language')}</h3>
        <div class="group">
          <div class="row">
            <span class="row-label">{$t('settings.language')}</span>
            <FieldSelect
              options={[{ value: 'en', label: 'EN' }, { value: 'zh', label: '中文' }]}
              bind:value={$locale}
            />
          </div>
        </div>

        {#if saved}
          <p class="saved-note">{$t('settings.saved')}</p>
        {/if}
        {#if error}
          <p class="inline-error">{error}</p>
        {/if}

      <!-- ── Advanced ── -->
      {:else if activeSection === "advanced"}
        <h2>{$t('settings.nav.advanced')}</h2>

        <div class="group">
          <div class="row">
            <div class="row-label-stack">
              <span class="row-label">{$t('settings.screenshot')}</span>
              <span class="row-sub">{$t('settings.screenshot_desc')}</span>
            </div>
            <button class="toggle" class:on={screenshotContextEnabled} on:click={handleScreenshotContextToggle} aria-label="Toggle screenshot context">
              <span class="toggle-knob"></span>
            </button>
          </div>
          <div class="row-sep"></div>
          <div class="row">
            <div class="row-label-stack">
              <span class="row-label">{$t('settings.polish')}</span>
              <span class="row-sub">{$t('settings.polish_desc')}</span>
            </div>
            <button class="toggle" class:on={polishEnabled} on:click={handlePolishToggle} aria-label="Toggle polish">
              <span class="toggle-knob"></span>
            </button>
          </div>
        </div>
      {/if}

    </main>
  </div>
</div>

<style>
  /* ── Root ── */
  .settings-root {
    width: 100%;
    height: 100%;
    display: flex;
    flex-direction: column;
    background: #f0f1f3;
    overflow: hidden;
    font-family: -apple-system, "SF Pro Text", BlinkMacSystemFont, sans-serif;
    -webkit-font-smoothing: antialiased;
    color: #1c1c1e;
  }

  /* ── Title bar ── */
  .titlebar {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 44px;
    padding: 0 14px;
    background: #e8e9eb;
    border-bottom: 1px solid rgba(0,0,0,0.12);
    cursor: grab;
    flex-shrink: 0;
  }
  .titlebar:active { cursor: grabbing; }
  .titlebar-label {
    font-size: 13px;
    font-weight: 600;
    color: #3c3c3e;
    flex: 1;
    text-align: center;
    /* offset to visually center given close button on left */
    padding-right: 22px;
  }

  .close-btn {
    width: 14px;
    height: 14px;
    border-radius: 50%;
    border: none;
    background: #ff5f57;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    flex-shrink: 0;
    padding: 0;
    transition: filter 0.1s;
  }
  .close-btn:hover { filter: brightness(0.88); }
  .close-btn svg { opacity: 0; transition: opacity 0.1s; }
  .close-btn:hover svg { opacity: 1; }

  .status-pill {
    font-size: 10px;
    font-weight: 500;
    padding: 2px 8px;
    border-radius: 999px;
    flex-shrink: 0;
  }
  .status-pill.recording { background: rgba(255,59,48,0.12); color: #ff3b30; }
  .status-pill.processing { background: rgba(0,122,255,0.12); color: #007aff; }

  /* ── Layout ── */
  .layout {
    display: flex;
    flex: 1;
    min-height: 0;
  }

  /* ── Sidebar ── */
  .sidebar {
    width: 168px;
    flex-shrink: 0;
    background: #e4e5e7;
    border-right: 1px solid rgba(0,0,0,0.1);
    padding: 10px 8px;
    display: flex;
    flex-direction: column;
    gap: 2px;
    overflow-y: auto;
  }

  .nav-item {
    display: flex;
    align-items: center;
    gap: 9px;
    width: 100%;
    padding: 8px 10px;
    border: none;
    border-radius: 7px;
    background: transparent;
    color: #3c3c3e;
    font-size: 13.5px;
    font-weight: 400;
    text-align: left;
    cursor: pointer;
    transition: background 0.12s;
    font-family: -apple-system, "SF Pro Text", BlinkMacSystemFont, sans-serif;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  .nav-item:hover { background: rgba(0,0,0,0.07); }
  .nav-item.active {
    background: rgba(0,0,0,0.14);
    color: #1c1c1e;
    font-weight: 500;
  }

  /* ── Content ── */
  .content {
    flex: 1;
    min-width: 0;
    padding: 20px 20px 24px;
    overflow-y: auto;
    background: #f0f1f3;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .content h2 {
    font-size: 20px;
    font-weight: 700;
    color: #1c1c1e;
    margin: 0 0 10px;
    letter-spacing: -0.02em;
  }

  .content h3 {
    font-size: 13px;
    font-weight: 600;
    color: #1c1c1e;
    margin: 10px 0 4px;
    letter-spacing: 0;
  }

  /* ── Groups (white card with rows) ── */
  .group {
    background: white;
    border-radius: 10px;
    border: 1px solid rgba(0,0,0,0.09);
    overflow: hidden;
  }

  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 0 16px;
    min-height: 48px;
  }

  .row-sep {
    height: 1px;
    background: rgba(0,0,0,0.07);
    margin: 0 16px;
  }

  .row-label {
    font-size: 14px;
    color: #1c1c1e;
    flex-shrink: 0;
  }

  .row-label-stack {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .row-sub {
    font-size: 12px;
    color: #8e8e93;
  }

  /* ── Controls ── */
  .row-select {
    font-size: 13px;
    color: #1c1c1e;
    background: #f2f2f2;
    border: 1px solid rgba(0,0,0,0.12);
    border-radius: 7px;
    padding: 5px 7px;
    outline: none;
    cursor: pointer;
    max-width: 200px;
    font-family: -apple-system, "SF Pro Text", BlinkMacSystemFont, sans-serif;
  }

  .row-input {
    font-size: 13px;
    color: #1c1c1e;
    background: #f2f2f2;
    border: 1px solid rgba(0,0,0,0.12);
    border-radius: 7px;
    padding: 6px 9px;
    outline: none;
    width: 100%;
    max-width: 200px;
    font-family: -apple-system, "SF Pro Text", BlinkMacSystemFont, sans-serif;
    transition: border-color 0.15s, background 0.15s;
  }
  .row-input:focus { border-color: #007aff; background: white; }
  .row-input::placeholder { color: #aeaeb2; }
  .row-input.mono { font-family: "SF Mono", "Fira Code", monospace; font-size: 12px; }

  .row-input-group {
    display: flex;
    gap: 6px;
    align-items: center;
  }

  .apply-btn {
    font-size: 12px;
    font-weight: 500;
    padding: 5px 10px;
    border-radius: 6px;
    border: 1px solid rgba(0,0,0,0.15);
    background: #f0f0f0;
    color: #1c1c1e;
    cursor: pointer;
    white-space: nowrap;
    flex-shrink: 0;
    font-family: -apple-system, "SF Pro Text", BlinkMacSystemFont, sans-serif;
    transition: background 0.1s;
  }
  .apply-btn:hover { background: #e0e0e0; }

  /* macOS-style toggle */
  .toggle {
    position: relative;
    width: 44px;
    height: 26px;
    border-radius: 13px;
    border: none;
    background: rgba(0,0,0,0.15);
    cursor: pointer;
    flex-shrink: 0;
    padding: 0;
    transition: background 0.2s;
  }
  .toggle.on { background: #007aff; }
  .toggle-knob {
    position: absolute;
    top: 3px;
    left: 3px;
    width: 20px;
    height: 20px;
    border-radius: 50%;
    background: white;
    box-shadow: 0 1px 3px rgba(0,0,0,0.25);
    transition: transform 0.2s cubic-bezier(0.4,0,0.2,1);
  }
  .toggle.on .toggle-knob { transform: translateX(18px); }

  /* Save / action row */
  .action-row {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .save-btn {
    padding: 8px 20px;
    border-radius: 8px;
    border: none;
    background: #007aff;
    color: white;
    font-size: 13.5px;
    font-weight: 600;
    cursor: pointer;
    transition: filter 0.1s;
    font-family: -apple-system, "SF Pro Text", BlinkMacSystemFont, sans-serif;
    letter-spacing: -0.01em;
  }
  .save-btn:hover:not(:disabled) { filter: brightness(0.92); }
  .save-btn:disabled { opacity: 0.5; cursor: not-allowed; }

  /* Auth badge */
  .auth-badge {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    color: #ff3b30;
    padding: 6px 10px;
    border-radius: 7px;
    background: rgba(255,59,48,0.06);
    border: 1px solid rgba(255,59,48,0.15);
  }
  .auth-badge.ok { color: #34c759; background: rgba(52,199,89,0.06); border-color: rgba(52,199,89,0.2); }
  .auth-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: #ff3b30;
    flex-shrink: 0;
  }
  .auth-badge.ok .auth-dot { background: #34c759; }

  /* Misc text */
  .hint {
    font-size: 12px;
    color: #8e8e93;
    line-height: 1.5;
    margin: 0;
  }
  .hint :global(a) { color: #007aff; text-decoration: none; }
  .hint :global(code) {
    font-family: "SF Mono", "Fira Code", monospace;
    font-size: 10px;
    background: rgba(0,0,0,0.06);
    padding: 1px 4px;
    border-radius: 3px;
  }

  .warn {
    font-size: 12px;
    color: #ff9500;
    padding: 6px 10px;
    border-radius: 7px;
    background: rgba(255,149,0,0.08);
    border: 1px solid rgba(255,149,0,0.2);
    margin: 0;
  }

  .inline-error {
    font-size: 12px;
    color: #ff3b30;
  }

  .saved-note {
    font-size: 12px;
    color: #34c759;
    margin: 0;
  }
</style>
