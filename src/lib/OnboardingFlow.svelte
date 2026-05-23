<script lang="ts">
  import { createEventDispatcher, onMount, onDestroy } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { providers, getDefaultConfig, getProvider, groupFields } from "./providers";
  import { t, locale, type Locale } from "./i18n";

  const dispatch = createEventDispatcher();

  const isWindows = navigator.userAgent.includes("Windows");
  const totalSteps = isWindows ? 3 : 4;

  let step = 1;
  let provider = providers[0]?.id ?? "groq";
  let configValues: Record<string, string> = getDefaultConfig(providers[0]?.fields ?? []);
  let authStatus: boolean | null = null;

  let configSaving = false;
  let configSaved = false;
  let configError = "";

  let micStatus = "not_determined";
  let axGranted = false;
  let pollTimer: ReturnType<typeof setInterval> | null = null;

  $: {
    stopPolling();
    if (!isWindows && step === 2) {
      startPermissionsStep();
    }
  }

  async function startPermissionsStep() {
    await refreshPermissions();
    pollTimer = setInterval(refreshPermissions, 1000);
    // Permission requests are user-initiated only — users click "Grant Access"
    // to trigger the macOS prompt. Auto-triggering surprises the user with a
    // system dialog they didn't ask for.
  }

  function stopPolling() {
    if (pollTimer !== null) {
      clearInterval(pollTimer);
      pollTimer = null;
    }
  }

  async function refreshPermissions() {
    micStatus = await invoke<string>("get_microphone_status").catch(() => "not_determined");
    axGranted = await invoke<boolean>("get_accessibility_status").catch(() => false);
  }

  onDestroy(stopPolling);

  $: currentProvider = getProvider(provider);
  $: fieldGroups = groupFields(currentProvider?.fields ?? []);

  onMount(async () => {
    for (const p of providers) {
      if (p.authCheck) {
        const ok = await invoke<boolean>(p.authCheck, { provider: p.id }).catch(() => false);
        if (p.id === provider) authStatus = ok;
      }
    }
  });

  async function handleProviderSwitch(id: string) {
    provider = id;
    configValues = getDefaultConfig(getProvider(id)?.fields ?? []);
    configError = "";
    const cp = getProvider(id);
    if (cp?.authCheck) {
      authStatus = await invoke<boolean>(cp.authCheck, { provider: id }).catch(() => false);
    } else {
      authStatus = null;
    }
  }

  async function saveConfig() {
    configSaving = true;
    configError = "";
    try {
      await invoke("save_provider", { provider });
      await invoke("save_provider_config", { provider, configValues });
      configSaved = true;
      setTimeout(() => {
        step = totalSteps;
      }, 500);
    } catch (e) {
      configError = String(e);
    } finally {
      configSaving = false;
    }
  }

  async function skipConfig() {
    await invoke("save_provider", { provider });
    step = totalSteps;
  }

  async function finishOnboarding() {
    await invoke("save_onboarding_completed");
    dispatch("done");
  }

  function next() {
    if (step < totalSteps) step++;
  }

  function switchLocale(loc: Locale) {
    $locale = loc;
  }
</script>

<div class="onboarding">
  <!-- Draggable title bar with close button -->
  <div class="win-bar" data-tauri-drag-region>
    <button class="win-close" on:click={finishOnboarding} aria-label="Close">
      <svg width="8" height="8" viewBox="0 0 8 8">
        <path d="M1 1l6 6M7 1L1 7" stroke="rgba(0,0,0,0.5)" stroke-width="1.5" stroke-linecap="round"/>
      </svg>
    </button>
  </div>

  <div class="dots">
    {#each Array(totalSteps) as _, i}
      <div class="dot" class:active={i + 1 === step} class:done={i + 1 < step}></div>
    {/each}
  </div>

  <!-- Step 1: Welcome -->
  {#if step === 1}
    <div class="step">
      <div class="lang-pick">
        <button class:active={$locale === "en"} on:click={() => switchLocale("en")}>EN</button>
        <button class:active={$locale === "zh"} on:click={() => switchLocale("zh")}>中文</button>
      </div>
      <div class="app-icon">
        <svg width="44" height="44" viewBox="0 0 48 48" fill="none">
          <rect width="48" height="48" rx="12" fill="rgba(99,102,241,0.18)" />
          <rect x="19" y="8" width="10" height="22" rx="5" fill="rgba(129,140,248,0.9)" />
          <path
            d="M10 22a14 14 0 0 0 28 0"
            stroke="rgba(129,140,248,0.9)"
            stroke-width="3"
            stroke-linecap="round"
          />
          <line
            x1="24"
            y1="36"
            x2="24"
            y2="42"
            stroke="rgba(129,140,248,0.9)"
            stroke-width="3"
            stroke-linecap="round"
          />
          <line
            x1="18"
            y1="42"
            x2="30"
            y2="42"
            stroke="rgba(129,140,248,0.9)"
            stroke-width="3"
            stroke-linecap="round"
          />
        </svg>
      </div>
      <h1 class="app-name">{$t("app.name")}</h1>
      <p class="app-desc">{$t("app.desc")}</p>
      <button class="primary-btn" on:click={next}>{$t("onboarding.start")}</button>
    </div>

    <!-- Step 3 (macOS) / Step 2 (Windows): Provider + Config -->
  {:else if (isWindows && step === 2) || (!isWindows && step === 3)}
    <div class="step">
      <div class="step-num">{step} / {totalSteps}</div>
      <h2>{$t("onboarding.configure")}</h2>

      <div class="provider-tabs">
        {#each providers as p}
          <button
            class="provider-tab"
            class:active={provider === p.id}
            on:click={() => handleProviderSwitch(p.id)}
          >
            <div class="tab-text">
              <span class="tab-name">{p.name}</span>
              <span class="tab-desc">{p.tagline[$locale]}</span>
            </div>
          </button>
        {/each}
      </div>

      {#if currentProvider?.hint}
        <p class="desc">{@html currentProvider.hint[$locale]}</p>
      {/if}

      <!-- Dynamic config fields -->
      {#each fieldGroups as group}
        {#if group.length === 1}
          {#if group[0].type === "select"}
            <select class="config-select" bind:value={configValues[group[0].key]}>
              {#each group[0].options ?? [] as opt}
                <option value={opt.value}>{opt.label}</option>
              {/each}
            </select>
          {:else if group[0].type === "password"}
            <input
              type="password"
              class="config-input"
              placeholder={group[0].placeholder ?? group[0].label[$locale]}
              bind:value={configValues[group[0].key]}
              on:keydown={(e) => e.key === "Enter" && saveConfig()}
              autocomplete="off"
              spellcheck="false"
            />
          {:else}
            <input
              type="text"
              class="config-input"
              class:mono={group[0].mono}
              placeholder={group[0].placeholder ?? group[0].label[$locale]}
              bind:value={configValues[group[0].key]}
              on:keydown={(e) => e.key === "Enter" && saveConfig()}
              autocomplete="off"
              spellcheck="false"
            />
          {/if}
        {:else}
          <div class="mini-row">
            {#each group as field}
              {#if field.type === "select"}
                <select class="config-select" bind:value={configValues[field.key]}>
                  {#each field.options ?? [] as opt}
                    <option value={opt.value}>{opt.label}</option>
                  {/each}
                </select>
              {:else}
                <input
                  type="text"
                  class="config-input mini"
                  class:mono={field.mono}
                  placeholder={field.placeholder ?? field.label[$locale]}
                  bind:value={configValues[field.key]}
                  autocomplete="off"
                  spellcheck="false"
                />
              {/if}
            {/each}
          </div>
        {/if}
      {/each}

      {#if authStatus !== null && currentProvider?.authOkText}
        <div class="auth-badge" class:ok={authStatus}>
          <div class="auth-dot"></div>
          <span
            >{authStatus
              ? currentProvider.authOkText[$locale]
              : (currentProvider.authFailText ?? currentProvider.authOkText)[$locale]}</span
          >
        </div>
      {/if}

      {#if configError}
        <div class="err">{configError}</div>
      {/if}
      <div class="btn-row">
        <button class="ghost-btn" on:click={skipConfig}>{$t("onboarding.skip")}</button>
        <button class="primary-btn" on:click={saveConfig} disabled={configSaving}>
          {#if configSaved}{$t("onboarding.saved")}{:else if configSaving}{$t(
              "settings.saving"
            )}{:else}{$t("onboarding.save_continue")}{/if}
        </button>
      </div>
    </div>

    <!-- Step 2: Permissions (macOS only) -->
  {:else if step === 2 && !isWindows}
    <div class="step perm-step">
      <h2>{$t("onboarding.perms_title")}</h2>
      <p class="desc" style="text-align:center">{$t("onboarding.perms_subtitle")}</p>

      <!-- Accessibility -->
      <div class="perm-section">
        <div class="perm-header">
          <span class="perm-name">{$t("onboarding.perms_ax")}</span>
          <span class="perm-sdesc">{$t("onboarding.perms_ax_desc")}</span>
        </div>
        {#if axGranted}
          <button class="perm-btn granted" disabled>{$t("onboarding.perms_granted")}</button>
        {:else}
          <button class="perm-btn" on:click={() => invoke("open_accessibility_prefs")}
            >{$t("onboarding.ax_open")}</button
          >
        {/if}
      </div>

      <div class="perm-divider"></div>

      <!-- Microphone -->
      <div class="perm-section">
        <div class="perm-header">
          <span class="perm-name">{$t("onboarding.perms_mic")}</span>
          <span class="perm-sdesc">{$t("onboarding.perms_mic_desc")}</span>
        </div>
        {#if micStatus === "authorized"}
          <button class="perm-btn granted" disabled>{$t("onboarding.perms_granted")}</button>
        {:else if micStatus === "denied" || micStatus === "restricted"}
          <button class="perm-btn" on:click={() => invoke("open_microphone_prefs")}
            >{$t("onboarding.mic_open")}</button
          >
        {:else}
          <button class="perm-btn" on:click={() => invoke("request_microphone_permission")}
            >{$t("onboarding.perms_request")}</button
          >
        {/if}
      </div>

      <button class="primary-btn" style="width:100%;margin-top:4px" on:click={next}
        >{$t("onboarding.perms_continue")}</button
      >
    </div>

    <!-- Final step: Done -->
  {:else if step === totalSteps}
    <div class="step">
      <div class="done-check">
        <svg width="28" height="28" viewBox="0 0 28 28" fill="none">
          <circle cx="14" cy="14" r="13" stroke="rgba(74,222,128,0.7)" stroke-width="2" />
          <path
            d="M8 14l4 4 8-8"
            stroke="rgba(74,222,128,0.9)"
            stroke-width="2.5"
            stroke-linecap="round"
            stroke-linejoin="round"
          />
        </svg>
      </div>
      <h2>{$t("onboarding.ready")}</h2>

      <div class="shortcut-display">
        <span class="shortcut-label">{$t("onboarding.shortcut_label")}</span>
        <div class="key-row">
          {#if isWindows}
            <kbd class="key">Ctrl</kbd>
            <span class="plus">+</span>
            <kbd class="key">Shift</kbd>
            <span class="plus">+</span>
            <kbd class="key key-wide">Space</kbd>
          {:else}
            <kbd class="key">⌘</kbd>
            <kbd class="key">⇧</kbd>
            <kbd class="key key-wide">Space</kbd>
          {/if}
        </div>
        <p class="shortcut-hint">{$t("onboarding.shortcut_hint")}</p>
      </div>

      <p class="settings-hint">
        {$t(isWindows ? "onboarding.settings_hint_win" : "onboarding.settings_hint_mac")}
      </p>

      <button class="primary-btn" on:click={finishOnboarding}>{$t("onboarding.finish")}</button>
    </div>
  {/if}
</div>

<style>
  .onboarding {
    position: relative;
    width: 100%;
    height: 100%;
    background: rgba(30, 30, 32, 1);
    border-radius: 0;
    box-shadow: none;
    border: none;
    padding: 36px 24px 20px;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 16px;
    overflow-y: auto;
    font-family: -apple-system, "SF Pro Text", BlinkMacSystemFont, sans-serif;
    -webkit-font-smoothing: antialiased;
  }
  .win-bar {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    height: 36px;
    display: flex;
    align-items: center;
    justify-content: flex-start;
    padding: 0 12px;
    -webkit-app-region: drag;
  }
  .win-close {
    -webkit-app-region: no-drag;
    width: 14px;
    height: 14px;
    border-radius: 50%;
    background: #ff5f57;
    border: none;
    color: transparent;
    font-size: 8px;
    line-height: 1;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0;
    transition: filter 0.1s;
  }
  .win-close svg {
    opacity: 0;
    transition: opacity 0.1s;
  }
  .win-close:hover {
    filter: brightness(0.88);
  }
  .win-close:hover svg {
    opacity: 1;
  }
  .dots {
    display: flex;
    gap: 6px;
    justify-content: center;
  }
  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.15);
    transition:
      background 0.2s,
      transform 0.2s;
  }
  .dot.active {
    background: rgba(129, 140, 248, 0.9);
    transform: scale(1.3);
  }
  .dot.done {
    background: rgba(129, 140, 248, 0.4);
  }
  .step {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
    text-align: center;
  }
  .step-num {
    font-size: 11px;
    color: rgba(255, 255, 255, 0.3);
    letter-spacing: 0.05em;
  }
  .app-icon {
    margin-top: 4px;
  }
  .app-name {
    font-size: 22px;
    font-weight: 700;
    color: rgba(255, 255, 255, 0.92);
    letter-spacing: -0.02em;
  }
  h2 {
    font-size: 17px;
    font-weight: 600;
    color: rgba(255, 255, 255, 0.9);
    letter-spacing: -0.01em;
  }
  .app-desc,
  .desc {
    font-size: 13px;
    color: rgba(255, 255, 255, 0.5);
    line-height: 1.6;
    max-width: 260px;
    white-space: pre-line;
  }
  .desc :global(a) {
    color: rgba(129, 140, 248, 0.85);
    text-decoration: none;
  }
  .desc :global(code) {
    font-family: "SF Mono", "Fira Code", monospace;
    font-size: 10px;
    background: rgba(255, 255, 255, 0.06);
    padding: 1px 4px;
    border-radius: 3px;
    color: rgba(255, 255, 255, 0.5);
  }

  /* Language picker on welcome step */
  .lang-pick {
    display: flex;
    gap: 0;
    background: rgba(255, 255, 255, 0.04);
    border-radius: 8px;
    padding: 2px;
    border: 1px solid rgba(255, 255, 255, 0.06);
  }
  .lang-pick button {
    padding: 4px 14px;
    border: none;
    border-radius: 6px;
    background: transparent;
    color: rgba(255, 255, 255, 0.35);
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.2s;
    font-family: -apple-system, "SF Pro Text", BlinkMacSystemFont, sans-serif;
  }
  .lang-pick button:hover {
    color: rgba(255, 255, 255, 0.55);
  }
  .lang-pick button.active {
    background: rgba(99, 102, 241, 0.2);
    color: rgba(165, 180, 252, 0.95);
  }

  /* Provider tabs */
  .provider-tabs {
    display: flex;
    gap: 8px;
    width: 100%;
    flex-wrap: wrap;
  }
  .provider-tab {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 0;
    padding: 10px 12px;
    border-radius: 12px;
    border: 1.5px solid rgba(255, 255, 255, 0.08);
    background: rgba(255, 255, 255, 0.03);
    color: rgba(255, 255, 255, 0.4);
    cursor: pointer;
    transition: all 0.2s ease;
    font-family: -apple-system, "SF Pro Text", BlinkMacSystemFont, sans-serif;
    text-align: left;
  }
  .provider-tab:hover {
    border-color: rgba(255, 255, 255, 0.15);
    color: rgba(255, 255, 255, 0.6);
  }
  .provider-tab.active {
    border-color: rgba(129, 140, 248, 0.45);
    background: rgba(99, 102, 241, 0.08);
    color: rgba(165, 180, 252, 0.95);
  }
  .tab-text {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
    width: 100%;
  }
  .tab-name {
    font-size: 13px;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .tab-desc {
    font-size: 10px;
    opacity: 0.55;
    font-weight: 400;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  /* Config inputs */
  .config-input {
    width: 100%;
    padding: 9px 12px;
    border-radius: 10px;
    border: 1px solid rgba(255, 255, 255, 0.1);
    background: rgba(255, 255, 255, 0.05);
    color: rgba(255, 255, 255, 0.9);
    font-size: 13px;
    font-family: -apple-system, "SF Pro Text", BlinkMacSystemFont, sans-serif;
    outline: none;
    text-align: left;
    transition: border-color 0.15s;
  }
  .config-input.mono {
    font-family: "SF Mono", "Fira Code", monospace;
    font-size: 12px;
  }
  .config-input:focus {
    border-color: rgba(129, 140, 248, 0.5);
  }
  .config-input::placeholder {
    color: rgba(255, 255, 255, 0.2);
  }
  .mini-row {
    display: flex;
    gap: 8px;
    width: 100%;
  }
  .config-input.mini {
    flex: 1;
    min-width: 0;
  }
  .config-select {
    flex: 1;
    min-width: 0;
    padding: 9px 8px;
    border-radius: 10px;
    border: 1px solid rgba(255, 255, 255, 0.1);
    background: rgba(255, 255, 255, 0.05);
    color: rgba(255, 255, 255, 0.9);
    font-size: 12px;
    font-family: -apple-system, "SF Pro Text", BlinkMacSystemFont, sans-serif;
    outline: none;
    cursor: pointer;
    appearance: auto;
  }
  .err {
    font-size: 12px;
    color: #f87171;
    align-self: flex-start;
  }

  /* Auth badge */
  .auth-badge {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 11px;
    color: rgba(248, 113, 113, 0.75);
    width: 100%;
    padding: 6px 10px;
    border-radius: 8px;
    background: rgba(248, 113, 113, 0.05);
    border: 1px solid rgba(248, 113, 113, 0.1);
    text-align: left;
  }
  .auth-badge.ok {
    color: rgba(134, 239, 172, 0.8);
    background: rgba(74, 222, 128, 0.05);
    border-color: rgba(74, 222, 128, 0.12);
  }
  .auth-dot {
    width: 5px;
    height: 5px;
    border-radius: 50%;
    background: rgba(248, 113, 113, 0.65);
    flex-shrink: 0;
  }
  .auth-badge.ok .auth-dot {
    background: rgba(74, 222, 128, 0.65);
  }

  /* OBS-style permissions page */
  .perm-step {
    align-items: flex-start;
    text-align: left;
  }
  .perm-step h2 {
    align-self: center;
  }
  .perm-section {
    width: 100%;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .perm-header {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .perm-name {
    font-size: 13px;
    font-weight: 600;
    color: rgba(255, 255, 255, 0.85);
  }
  .perm-sdesc {
    font-size: 12px;
    color: rgba(255, 255, 255, 0.45);
    line-height: 1.4;
  }
  .perm-btn {
    width: 100%;
    padding: 9px 16px;
    border-radius: 10px;
    border: 1px solid rgba(255, 255, 255, 0.15);
    background: rgba(255, 255, 255, 0.06);
    color: rgba(255, 255, 255, 0.7);
    font-size: 13px;
    font-weight: 500;
    cursor: pointer;
    transition: background 0.15s;
    font-family: -apple-system, "SF Pro Text", BlinkMacSystemFont, sans-serif;
  }
  .perm-btn:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.1);
    border-color: rgba(255, 255, 255, 0.22);
  }
  .perm-btn.granted {
    color: rgba(134, 239, 172, 0.85);
    background: rgba(74, 222, 128, 0.06);
    border-color: rgba(74, 222, 128, 0.18);
    cursor: default;
  }
  .perm-btn.muted {
    color: rgba(255, 255, 255, 0.25);
    cursor: default;
  }
  .perm-divider {
    width: 100%;
    height: 1px;
    background: rgba(255, 255, 255, 0.06);
  }
  .done-check {
    margin-top: 8px;
  }

  /* Final "All Set" — prominent keyboard shortcut display */
  .shortcut-display {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    padding: 16px 20px;
    margin: 4px 0 8px;
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 12px;
    width: 100%;
    max-width: 320px;
  }
  .shortcut-label {
    font-size: 11px;
    font-weight: 500;
    color: rgba(255, 255, 255, 0.45);
    text-transform: uppercase;
    letter-spacing: 0.08em;
  }
  .key-row {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .key {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-width: 36px;
    height: 36px;
    padding: 0 10px;
    background: linear-gradient(180deg, rgba(255, 255, 255, 0.12), rgba(255, 255, 255, 0.06));
    border: 1px solid rgba(255, 255, 255, 0.18);
    border-bottom-color: rgba(255, 255, 255, 0.06);
    border-radius: 7px;
    font-family: -apple-system, "SF Pro Text", BlinkMacSystemFont, sans-serif;
    font-size: 15px;
    font-weight: 600;
    color: rgba(255, 255, 255, 0.92);
    box-shadow:
      0 1px 0 rgba(0, 0, 0, 0.35),
      inset 0 1px 0 rgba(255, 255, 255, 0.08);
  }
  .key-wide {
    min-width: 64px;
    padding: 0 14px;
  }
  .plus {
    font-size: 13px;
    color: rgba(255, 255, 255, 0.35);
    font-weight: 400;
  }
  .shortcut-hint {
    font-size: 12px;
    color: rgba(255, 255, 255, 0.55);
    margin: 0;
    text-align: center;
    line-height: 1.5;
  }
  .settings-hint {
    font-size: 11px;
    color: rgba(255, 255, 255, 0.4);
    margin: 0;
    text-align: center;
  }

  /* Buttons */
  .btn-row {
    display: flex;
    gap: 10px;
    width: 100%;
    justify-content: center;
    margin-top: 4px;
  }
  .primary-btn {
    padding: 9px 22px;
    border-radius: 10px;
    border: none;
    background: rgba(99, 102, 241, 0.85);
    color: white;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
    transition: background 0.15s;
    font-family: -apple-system, "SF Pro Text", BlinkMacSystemFont, sans-serif;
  }
  .primary-btn:hover:not(:disabled) {
    background: rgba(99, 102, 241, 1);
  }
  .primary-btn:disabled {
    opacity: 0.55;
    cursor: not-allowed;
  }
  .ghost-btn {
    padding: 9px 18px;
    border-radius: 10px;
    border: 1px solid rgba(255, 255, 255, 0.12);
    background: rgba(255, 255, 255, 0.06);
    color: rgba(255, 255, 255, 0.55);
    font-size: 13px;
    font-weight: 500;
    cursor: pointer;
    transition: background 0.15s;
    font-family: -apple-system, "SF Pro Text", BlinkMacSystemFont, sans-serif;
  }
  .ghost-btn:hover {
    background: rgba(255, 255, 255, 0.1);
  }
</style>
