<script lang="ts">
  import { createEventDispatcher, onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { providers, getProvider, groupFields } from "./providers";
  import { t, locale, type Locale } from "./i18n";

  const dispatch = createEventDispatcher();

  export let polishEnabled: boolean = true;
  export let audioDevices: string[] = [];
  export let autostartEnabled: boolean = false;
  export let screenshotContextEnabled: boolean = false;
  export let appState: string = "idle";
  export let shortcutConflict: string = "";

  let provider = "groq";
  let configValues: Record<string, string> = {};
  let authStatus: boolean | null = null;

  let preferredDevice: string | null = null;
  let shortcut = "Meta+Shift+Space";
  let saving = false;
  let saved = false;
  let error = "";
  let opacity = 1.0;

  $: currentProvider = getProvider(provider);
  $: fieldGroups = groupFields(currentProvider?.fields ?? []);

  onMount(async () => {
    provider = await invoke<string>("get_provider");
    await loadProviderConfig();
    shortcut = await invoke<string>("get_shortcut");
    preferredDevice = await invoke<string | null>("get_preferred_device").catch(() => null);
    const savedOpacity = localStorage.getItem("window-opacity");
    if (savedOpacity) {
      opacity = parseFloat(savedOpacity);
      await getCurrentWindow().setOpacity(opacity);
    }
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
    showSaved();
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

  async function handleOpacityChange(e: Event) {
    opacity = parseFloat((e.target as HTMLInputElement).value);
    localStorage.setItem("window-opacity", String(opacity));
    await getCurrentWindow().setOpacity(opacity);
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

  function switchLocale(loc: Locale) {
    $locale = loc;
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
  <div class="header" role="toolbar" aria-label="Settings" on:mousedown={handleHeaderMousedown}>
    <span class="title">{$t('settings.title')}</span>
    {#if appState === "recording"}
      <div class="rec-badge"><div class="rec-dot"></div><span>{$t('settings.recording')}</span></div>
    {:else if appState === "processing"}
      <div class="rec-badge processing"><div class="proc-spinner"></div><span>{$t('settings.transcribing')}</span></div>
    {/if}
    <button class="close-btn" on:click={() => dispatch("close")}>
      <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
        <path d="M1 1l10 10M11 1L1 11" stroke="rgba(255,255,255,0.5)" stroke-width="1.8" stroke-linecap="round"/>
      </svg>
    </button>
  </div>

  <div class="sections">
    <!-- Language -->
    <div class="section row-section">
      <span class="section-label">{$t('settings.language')}</span>
      <div class="lang-toggle">
        <button class:active={$locale === 'en'} on:click={() => switchLocale('en')}>EN</button>
        <button class:active={$locale === 'zh'} on:click={() => switchLocale('zh')}>中文</button>
      </div>
    </div>

    <div class="divider"></div>

    <!-- Provider selector (dynamic) -->
    <div class="section">
      <span class="section-label">{$t('settings.voice_service')}</span>
      <div class="provider-tabs" class:scrollable={providers.length > 3}>
        {#each providers as p}
          <button
            class="provider-tab"
            class:active={provider === p.id}
            on:click={() => handleProviderSwitch(p.id)}
          >
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none">{@html p.icon}</svg>
            {p.name}
          </button>
        {/each}
      </div>
    </div>

    <!-- Provider config (dynamic fields) -->
    {#if currentProvider}
      <div class="section provider-config">
        {#each fieldGroups as group}
          {#if group.length === 1}
            <div class="field">
              <span class="field-label">{group[0].label[$locale]}</span>
              {#if group[0].type === "select"}
                <select class="select-input" bind:value={configValues[group[0].key]}>
                  {#each group[0].options ?? [] as opt}
                    <option value={opt.value}>{opt.label}</option>
                  {/each}
                </select>
              {:else if group[0].type === "password"}
                <input
                  type="password"
                  class="text-input"
                  placeholder={group[0].placeholder ?? ""}
                  bind:value={configValues[group[0].key]}
                  autocomplete="off"
                  spellcheck="false"
                />
              {:else}
                <input
                  type="text"
                  class="text-input"
                  class:mono={group[0].mono}
                  placeholder={group[0].placeholder ?? ""}
                  bind:value={configValues[group[0].key]}
                  autocomplete="off"
                  spellcheck="false"
                />
              {/if}
            </div>
          {:else}
            <div class="field-row">
              {#each group as field}
                <div class="field flex1">
                  <span class="field-label">{field.label[$locale]}</span>
                  {#if field.type === "select"}
                    <select class="select-input" bind:value={configValues[field.key]}>
                      {#each field.options ?? [] as opt}
                        <option value={opt.value}>{opt.label}</option>
                      {/each}
                    </select>
                  {:else}
                    <input
                      type="text"
                      class="text-input"
                      class:mono={field.mono}
                      placeholder={field.placeholder ?? ""}
                      bind:value={configValues[field.key]}
                      autocomplete="off"
                      spellcheck="false"
                    />
                  {/if}
                </div>
              {/each}
            </div>
          {/if}
        {/each}

        <button class="action-btn full-width" on:click={handleSaveConfig} disabled={saving}>
          {saving ? $t('settings.saving') : $t('settings.save')}
        </button>

        {#if authStatus !== null && currentProvider.authOkText}
          <div class="auth-status" class:ok={authStatus}>
            <div class="auth-dot"></div>
            <span>{authStatus ? currentProvider.authOkText[$locale] : (currentProvider.authFailText ?? currentProvider.authOkText)[$locale]}</span>
          </div>
        {/if}

        {#if currentProvider.hint}
          <p class="hint">{@html currentProvider.hint[$locale]}</p>
        {/if}
      </div>
    {/if}

    <div class="divider"></div>

    <!-- Polish -->
    <div class="section row-section">
      <div class="row-label-block">
        <span class="section-label">{$t('settings.polish')}</span>
        <span class="row-desc">{$t('settings.polish_desc')}</span>
      </div>
      <button class="toggle" class:on={polishEnabled} on:click={handlePolishToggle} aria-label="Toggle polish">
        <div class="toggle-knob"></div>
      </button>
    </div>

    <div class="divider"></div>

    <!-- Microphone -->
    <div class="section">
      <span class="section-label">{$t('settings.mic')}</span>
      <select class="select-input" on:change={handleDeviceChange} value={preferredDevice ?? "__default__"}>
        <option value="__default__">{$t('settings.mic_default')}</option>
        {#each audioDevices as device}
          <option value={device}>{device}</option>
        {/each}
      </select>
    </div>

    <div class="divider"></div>

    <!-- Shortcut -->
    <div class="section">
      <span class="section-label">{$t('settings.shortcut')}</span>
      <div class="input-row">
        <input type="text" bind:value={shortcut} class="text-input mono" placeholder="Meta+Shift+Space" />
        <button class="action-btn" on:click={handleShortcutChange}>{$t('settings.shortcut_apply')}</button>
      </div>
      {#if shortcutConflict}
        <div class="conflict-banner">{$t('settings.shortcut_conflict', shortcutConflict)}</div>
      {/if}
      <p class="hint">{$t('settings.shortcut_hint')}</p>
    </div>

    <div class="divider"></div>

    <!-- Autostart -->
    <div class="section row-section">
      <div class="row-label-block">
        <span class="section-label">{$t('settings.autostart')}</span>
        <span class="row-desc">{$t('settings.autostart_desc')}</span>
      </div>
      <button class="toggle" class:on={autostartEnabled} on:click={handleAutostartToggle} aria-label="Toggle autostart">
        <div class="toggle-knob"></div>
      </button>
    </div>

    <div class="divider"></div>

    <!-- Screenshot context -->
    <div class="section row-section">
      <div class="row-label-block">
        <span class="section-label">{$t('settings.screenshot')}</span>
        <span class="row-desc">{$t('settings.screenshot_desc')}</span>
      </div>
      <button class="toggle" class:on={screenshotContextEnabled} on:click={handleScreenshotContextToggle} aria-label="Toggle screenshot context" disabled={!polishEnabled}>
        <div class="toggle-knob"></div>
      </button>
    </div>

    <div class="divider"></div>

    <!-- Opacity -->
    <div class="section">
      <div class="row-label-block">
        <span class="section-label">{$t('settings.opacity')}</span>
        <span class="row-desc">{Math.round(opacity * 100)}%</span>
      </div>
      <input type="range" min="0.2" max="1" step="0.05" value={opacity} on:input={handleOpacityChange} class="opacity-slider" />
    </div>
  </div>

  {#if error}<div class="error-banner">{error}</div>{/if}
  {#if saved}<div class="saved-banner">{$t('settings.saved')}</div>{/if}
</div>

<style>
  .settings-panel {
    width: 320px;
    background: rgba(30,30,32,0.92);
    backdrop-filter: blur(20px) saturate(180%);
    -webkit-backdrop-filter: blur(20px) saturate(180%);
    border-radius: 16px;
    box-shadow: 0 8px 40px rgba(0,0,0,0.5);
    overflow: hidden;
    font-family: -apple-system,"SF Pro Text",BlinkMacSystemFont,sans-serif;
    -webkit-font-smoothing: antialiased;
  }
  .header { display:flex; align-items:center; justify-content:space-between; padding:14px 16px 12px; border-bottom:1px solid rgba(255,255,255,0.08); cursor:grab; }
  .header:active { cursor:grabbing; }
  .title { font-size:14px; font-weight:600; color:rgba(255,255,255,0.88); }
  .rec-badge { display:flex; align-items:center; gap:5px; padding:3px 9px; border-radius:999px; background:rgba(239,68,68,0.15); border:1px solid rgba(239,68,68,0.3); font-size:11px; color:#f87171; font-weight:500; margin-left:auto; margin-right:8px; }
  .rec-badge.processing { background:rgba(99,130,246,0.12); border-color:rgba(99,130,246,0.25); color:#818cf8; }
  .rec-dot { width:6px; height:6px; border-radius:50%; background:#ef4444; animation:blink 1.4s ease-in-out infinite; }
  .proc-spinner { width:10px; height:10px; border-radius:50%; border:1.5px solid rgba(99,130,246,0.2); border-top-color:#818cf8; animation:spin .8s linear infinite; }
  @keyframes blink { 0%,100%{opacity:1} 50%{opacity:.5} }
  @keyframes spin { to { transform:rotate(360deg); } }
  .close-btn { background:rgba(255,255,255,0.08); border:none; border-radius:50%; width:22px; height:22px; display:flex; align-items:center; justify-content:center; cursor:pointer; transition:background .15s; padding:0; }
  .close-btn:hover { background:rgba(255,255,255,0.15); }
  .sections { padding:4px 0; overflow-y:auto; max-height:calc(100vh - 56px); }
  .section { padding:11px 16px; display:flex; flex-direction:column; gap:7px; }
  .section.row-section { flex-direction:row; align-items:center; justify-content:space-between; }
  .section-label { font-size:13px; font-weight:500; color:rgba(255,255,255,0.8); }
  .row-label-block { display:flex; flex-direction:column; gap:2px; }
  .row-desc { font-size:11px; color:rgba(255,255,255,0.35); }
  .hint { font-size:11px; color:rgba(255,255,255,0.35); line-height:1.5; margin:0; }
  .hint :global(a) { color:rgba(129,140,248,0.85); text-decoration:none; }
  .hint :global(code) { font-family:"SF Mono","Fira Code",monospace; font-size:10px; background:rgba(255,255,255,0.06); padding:1px 4px; border-radius:3px; color:rgba(255,255,255,0.5); }
  .divider { height:1px; background:rgba(255,255,255,0.07); margin:0 16px; }
  .input-row { display:flex; gap:8px; }
  .text-input { flex:1; padding:7px 10px; border-radius:8px; border:1px solid rgba(255,255,255,0.1); background:rgba(255,255,255,0.05); color:rgba(255,255,255,0.88); font-size:13px; font-family:-apple-system,"SF Pro Text",BlinkMacSystemFont,sans-serif; outline:none; transition:border-color .15s; min-width:0; }
  .text-input.mono { font-family:"SF Mono","Fira Code",monospace; font-size:12px; }
  .text-input:focus { border-color:rgba(129,140,248,0.5); }
  .text-input::placeholder { color:rgba(255,255,255,0.2); }
  .select-input { width:100%; padding:7px 10px; border-radius:8px; border:1px solid rgba(255,255,255,0.1); background:rgba(255,255,255,0.05); color:rgba(255,255,255,0.88); font-size:13px; font-family:-apple-system,"SF Pro Text",BlinkMacSystemFont,sans-serif; outline:none; cursor:pointer; appearance:auto; }
  .action-btn { padding:7px 14px; border-radius:8px; border:none; background:rgba(99,102,241,0.75); color:white; font-size:13px; font-weight:500; cursor:pointer; transition:background .15s; white-space:nowrap; flex-shrink:0; }
  .action-btn:hover:not(:disabled) { background:rgba(99,102,241,0.9); }
  .action-btn:disabled { opacity:0.5; cursor:not-allowed; }
  .action-btn.full-width { width:100%; }

  /* Language toggle */
  .lang-toggle { display:flex; background:rgba(255,255,255,0.04); border-radius:8px; padding:2px; border:1px solid rgba(255,255,255,0.06); }
  .lang-toggle button { padding:4px 12px; border:none; border-radius:6px; background:transparent; color:rgba(255,255,255,0.4); font-size:12px; font-weight:500; cursor:pointer; transition:all .2s; font-family:-apple-system,"SF Pro Text",BlinkMacSystemFont,sans-serif; }
  .lang-toggle button:hover { color:rgba(255,255,255,0.6); }
  .lang-toggle button.active { background:rgba(99,102,241,0.2); color:rgba(165,180,252,0.95); }

  /* Provider tabs */
  .provider-tabs { display:flex; gap:0; background:rgba(255,255,255,0.04); border-radius:10px; padding:3px; border:1px solid rgba(255,255,255,0.06); }
  .provider-tabs.scrollable { overflow-x:auto; }
  .provider-tabs.scrollable .provider-tab { flex:0 0 auto; }
  .provider-tab { flex:1; display:flex; align-items:center; justify-content:center; gap:6px; padding:7px 12px; border:none; border-radius:8px; background:transparent; color:rgba(255,255,255,0.4); font-size:12.5px; font-weight:500; cursor:pointer; transition:all .2s ease; font-family:-apple-system,"SF Pro Text",BlinkMacSystemFont,sans-serif; }
  .provider-tab:hover { color:rgba(255,255,255,0.6); }
  .provider-tab.active { background:rgba(99,102,241,0.2); color:rgba(165,180,252,0.95); box-shadow:0 1px 3px rgba(0,0,0,0.15); }
  .provider-config { padding-top:6px; }

  /* Dynamic fields */
  .field { display:flex; flex-direction:column; gap:4px; }
  .field-label { font-size:11px; color:rgba(255,255,255,0.4); font-weight:500; }
  .field-row { display:flex; gap:8px; }
  .flex1 { flex:1; min-width:0; }

  /* Auth status */
  .auth-status { display:flex; align-items:center; gap:6px; font-size:11px; color:rgba(248,113,113,0.8); padding:6px 10px; border-radius:8px; background:rgba(248,113,113,0.06); border:1px solid rgba(248,113,113,0.12); }
  .auth-status.ok { color:rgba(134,239,172,0.85); background:rgba(74,222,128,0.06); border-color:rgba(74,222,128,0.15); }
  .auth-dot { width:6px; height:6px; border-radius:50%; background:rgba(248,113,113,0.7); flex-shrink:0; }
  .auth-status.ok .auth-dot { background:rgba(74,222,128,0.7); }

  /* Toggle */
  .toggle { position:relative; width:40px; height:24px; border-radius:12px; border:none; background:rgba(255,255,255,0.1); cursor:pointer; transition:background .2s; flex-shrink:0; padding:0; }
  .toggle.on { background:rgba(99,102,241,0.85); }
  .toggle-knob { position:absolute; top:3px; left:3px; width:18px; height:18px; border-radius:50%; background:white; box-shadow:0 1px 3px rgba(0,0,0,0.3); transition:transform .2s cubic-bezier(.4,0,.2,1); }
  .toggle.on .toggle-knob { transform:translateX(16px); }

  /* Banners */
  .conflict-banner { padding:6px 10px; border-radius:8px; background:rgba(251,191,36,0.08); border:1px solid rgba(251,191,36,0.2); font-size:11px; color:rgba(251,191,36,0.85); line-height:1.5; }
  .error-banner { margin:0 16px 12px; padding:7px 10px; border-radius:8px; background:rgba(248,113,113,0.12); border:1px solid rgba(248,113,113,0.25); font-size:12px; color:#f87171; }
  .saved-banner { margin:0 16px 12px; padding:7px 10px; border-radius:8px; background:rgba(74,222,128,0.1); border:1px solid rgba(74,222,128,0.25); font-size:12px; color:rgba(134,239,172,0.9); text-align:center; }
  .opacity-slider { width:100%; accent-color:rgba(99,102,241,0.85); cursor:pointer; }
</style>
