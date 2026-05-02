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
  export let showIdleHud: boolean = false;
  export let appState: string = "idle";
  export let shortcutConflict: string = "";

  let activeSection: "general" | "transcription" | "advanced" | "history" = "transcription";
  let provider = "openai";
  let configValues: Record<string, string> = {};
  let authStatus: boolean | null = null;

  let preferredDevice: string | null = null;
  let shortcut = "Meta+Shift+Space";
  let saving = false;
  let saved = false;
  let error = "";

  type HistoryEntry = {
    id: string;
    createdAtMs: number;
    durationS: number;
    provider: string;
    rawText: string;
    polishedText: string;
    status: "pending" | "completed" | "failed";
    error: string | null;
    polishFailed: boolean;
  };

  let history: HistoryEntry[] = [];
  let maxHistory = 100;
  let retryingId: string | null = null;

  $: currentProvider = getProvider(provider);

  onMount(async () => {
    provider = await invoke<string>("get_provider");
    await loadProviderConfig();
    shortcut = await invoke<string>("get_shortcut");
    preferredDevice = await invoke<string | null>("get_preferred_device").catch(() => null);
    maxHistory = await invoke<number>("get_max_history").catch(() => 100);
    await refreshHistory();
  });

  async function refreshHistory() {
    history = await invoke<HistoryEntry[]>("list_history").catch(() => []);
  }

  async function handleHistoryRetry(id: string) {
    if (retryingId) return;
    retryingId = id;
    try {
      await invoke("retry_transcription", { sessionId: id });
    } catch (e) {
      error = String(e);
    } finally {
      retryingId = null;
      await refreshHistory();
    }
  }

  async function handleHistoryDelete(id: string) {
    try {
      await invoke("delete_history_entry", { sessionId: id });
      await refreshHistory();
    } catch (e) {
      error = String(e);
    }
  }

  async function handleMaxHistoryChange(e: Event) {
    const raw = parseInt((e.target as HTMLInputElement).value, 10);
    const next = Number.isFinite(raw) && raw > 0 ? Math.min(raw, 1000) : 1;
    maxHistory = next;
    await invoke("save_max_history", { max: next });
    await refreshHistory();
  }

  function formatTime(ms: number): string {
    const d = new Date(ms);
    const pad = (n: number) => String(n).padStart(2, "0");
    const today = new Date();
    const sameDay =
      d.getFullYear() === today.getFullYear() &&
      d.getMonth() === today.getMonth() &&
      d.getDate() === today.getDate();
    const time = `${pad(d.getHours())}:${pad(d.getMinutes())}`;
    if (sameDay) return time;
    return `${d.getMonth() + 1}/${d.getDate()} ${time}`;
  }

  function entryText(e: HistoryEntry): string {
    return e.polishedText || e.rawText || "";
  }

  async function loadProviderConfig() {
    const raw = await invoke<Record<string, string>>("get_provider_config", { provider });
    const loaded = raw ?? {};
    const cp = getProvider(provider);
    // Apply field defaults for any unset values
    if (cp) {
      for (const field of cp.fields) {
        if (field.default !== undefined && !(field.key in loaded)) {
          loaded[field.key] = field.default;
        }
      }
    }
    configValues = loaded;
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

  async function handleShowIdleHudToggle() {
    showIdleHud = !showIdleHud;
    await invoke("save_show_idle_hud", { enabled: showIdleHud });
  }

  function handleProviderSelectChange(e: Event) {
    handleProviderSwitch((e.target as HTMLSelectElement).value);
  }

  function showSaved() {
    saved = true;
    setTimeout(() => {
      saved = false;
    }, 1800);
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
  <div class="titlebar" role="toolbar" tabindex="0" on:mousedown={handleTitlebarMousedown}>
    <button class="close-btn" on:click={() => dispatch("close")} aria-label="Close">
      <svg width="8" height="8" viewBox="0 0 8 8"
        ><path
          d="M1 1l6 6M7 1L1 7"
          stroke="rgba(0,0,0,0.5)"
          stroke-width="1.5"
          stroke-linecap="round"
        /></svg
      >
    </button>
    <span class="titlebar-label">{$t("settings.title")}</span>
    {#if appState === "recording"}
      <span class="status-pill recording">{$t("settings.recording")}</span>
    {:else if appState === "processing"}
      <span class="status-pill processing">{$t("settings.transcribing")}</span>
    {/if}
  </div>

  <div class="layout">
    <!-- Sidebar -->
    <nav class="sidebar">
      <button
        class="nav-item"
        class:active={activeSection === "transcription"}
        on:click={() => (activeSection = "transcription")}
      >
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none">
          <rect
            x="3"
            y="3"
            width="18"
            height="18"
            rx="3"
            stroke="currentColor"
            stroke-width="1.8"
          />
          <path
            d="M8 12h8M8 8h5M8 16h6"
            stroke="currentColor"
            stroke-width="1.8"
            stroke-linecap="round"
          />
        </svg>
        {$t("settings.nav.transcription")}
      </button>
      <button
        class="nav-item"
        class:active={activeSection === "general"}
        on:click={() => (activeSection = "general")}
      >
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none">
          <circle cx="12" cy="12" r="3" stroke="currentColor" stroke-width="1.8" />
          <path
            d="M12 2v2M12 20v2M4.22 4.22l1.42 1.42M18.36 18.36l1.42 1.42M2 12h2M20 12h2M4.22 19.78l1.42-1.42M18.36 5.64l1.42-1.42"
            stroke="currentColor"
            stroke-width="1.8"
            stroke-linecap="round"
          />
        </svg>
        {$t("settings.nav.general")}
      </button>
      <button
        class="nav-item"
        class:active={activeSection === "history"}
        on:click={() => {
          activeSection = "history";
          void refreshHistory();
        }}
      >
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none">
          <circle cx="12" cy="12" r="9" stroke="currentColor" stroke-width="1.8" />
          <path
            d="M12 7v5l3 2"
            stroke="currentColor"
            stroke-width="1.8"
            stroke-linecap="round"
            stroke-linejoin="round"
          />
        </svg>
        {$t("settings.nav.history")}
      </button>
      <button
        class="nav-item"
        class:active={activeSection === "advanced"}
        on:click={() => (activeSection = "advanced")}
      >
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none">
          <path
            d="M12 2L2 7l10 5 10-5-10-5zM2 17l10 5 10-5M2 12l10 5 10-5"
            stroke="currentColor"
            stroke-width="1.8"
            stroke-linecap="round"
            stroke-linejoin="round"
          />
        </svg>
        {$t("settings.nav.advanced")}
      </button>
    </nav>

    <!-- Content -->
    <main class="content">
      <!-- ── Transcription ── -->
      {#if activeSection === "transcription"}
        <h2>{$t("settings.nav.transcription")}</h2>

        <div class="group">
          <div class="row">
            <span class="row-label">{$t("settings.voice_service")}</span>
            <select class="row-select" value={provider} on:change={handleProviderSelectChange}>
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
            {saving ? $t("settings.saving") : saved ? $t("settings.saved") : $t("settings.save")}
          </button>
          {#if error}
            <span class="inline-error">{error}</span>
          {/if}
        </div>

        {#if authStatus !== null && currentProvider?.authOkText}
          <div class="auth-badge" class:ok={authStatus}>
            <span class="auth-dot"></span>
            {authStatus
              ? currentProvider.authOkText[$locale]
              : (currentProvider.authFailText ?? currentProvider.authOkText)[$locale]}
          </div>
        {/if}

        {#if currentProvider?.hint}
          <p class="hint">{@html currentProvider.hint[$locale]}</p>
        {/if}

        <!-- ── General ── -->
      {:else if activeSection === "general"}
        <h2>{$t("settings.nav.general")}</h2>

        <h3>{$t("settings.section.startup")}</h3>
        <div class="group">
          <div class="row">
            <span class="row-label">{$t("settings.autostart")}</span>
            <button
              class="toggle"
              class:on={autostartEnabled}
              on:click={handleAutostartToggle}
              aria-label="Toggle autostart"
            >
              <span class="toggle-knob"></span>
            </button>
          </div>
        </div>

        <h3>{$t("settings.section.input")}</h3>
        <div class="group">
          <div class="row">
            <span class="row-label">{$t("settings.mic")}</span>
            <select
              class="row-select"
              on:change={handleDeviceChange}
              value={preferredDevice ?? "__default__"}
            >
              <option value="__default__">{$t("settings.mic_default")}</option>
              {#each audioDevices as device}
                <option value={device}>{device}</option>
              {/each}
            </select>
          </div>
          <div class="row-sep"></div>
          <div class="row">
            <span class="row-label">{$t("settings.shortcut")}</span>
            <div class="row-input-group">
              <input
                type="text"
                bind:value={shortcut}
                class="row-input mono"
                placeholder="Meta+Shift+Space"
              />
              <button class="apply-btn" on:click={handleShortcutChange}
                >{$t("settings.shortcut_apply")}</button
              >
            </div>
          </div>
        </div>
        {#if shortcutConflict}
          <p class="warn">{$t("settings.shortcut_conflict", shortcutConflict)}</p>
        {/if}
        <p class="hint">{$t("settings.shortcut_hint")}</p>

        <h3>{$t("settings.section.language")}</h3>
        <div class="group">
          <div class="row">
            <span class="row-label">{$t("settings.language")}</span>
            <FieldSelect
              options={[
                { value: "en", label: "EN" },
                { value: "zh", label: "中文" },
              ]}
              bind:value={$locale}
            />
          </div>
        </div>

        {#if saved}
          <p class="saved-note">{$t("settings.saved")}</p>
        {/if}
        {#if error}
          <p class="inline-error">{error}</p>
        {/if}

        <!-- ── History ── -->
      {:else if activeSection === "history"}
        <h2>{$t("history.title")}</h2>
        <p class="history-section-hint">{$t("history.failed_hint")}</p>

        <div class="group">
          <div class="row">
            <div class="row-label-stack">
              <span class="row-label">{$t("history.max_label")}</span>
              <span class="row-sub">{$t("history.max_desc")}</span>
            </div>
            <input
              type="number"
              min="1"
              max="1000"
              class="row-input mono"
              style="max-width: 90px;"
              value={maxHistory}
              on:change={handleMaxHistoryChange}
            />
          </div>
        </div>

        {#if history.length === 0}
          <div class="history-empty">
            <p class="history-empty-title">{$t("history.empty")}</p>
            <p class="history-empty-hint">{$t("history.empty_hint")}</p>
          </div>
        {:else}
          <ul class="history-list">
            {#each history as entry (entry.id)}
              <li class="history-item" class:failed={entry.status === "failed"}>
                <div class="history-head">
                  <span class="history-time">{formatTime(entry.createdAtMs)}</span>
                  <span class="history-meta">
                    {entry.provider} · {$t("history.duration", entry.durationS.toFixed(1))}
                  </span>
                  <span class="history-status status-{entry.status}">
                    {#if entry.status === "completed"}
                      {$t("history.status.completed")}
                    {:else if entry.status === "failed"}
                      {$t("history.status.failed")}
                    {:else}
                      {$t("history.status.pending")}
                    {/if}
                  </span>
                </div>
                {#if entry.status === "failed"}
                  <p class="history-text history-failed-preview" title={entry.error ?? ""}>
                    {entry.error?.trim() ? entry.error : $t("history.failed_unknown")}
                  </p>
                {:else if entryText(entry)}
                  <p class="history-text">{entryText(entry)}</p>
                {:else if entry.error}
                  <p class="history-error" title={entry.error}>{entry.error}</p>
                {/if}
                <div class="history-actions">
                  <button
                    class="history-btn primary"
                    on:click={() => handleHistoryRetry(entry.id)}
                    disabled={retryingId !== null}
                  >
                    {retryingId === entry.id ? $t("hud.retrying") : $t("history.retry")}
                  </button>
                  {#if entryText(entry)}
                    <button
                      class="history-btn"
                      on:click={() =>
                        navigator.clipboard.writeText(entryText(entry)).catch(() => {})}
                    >
                      {$t("history.copy")}
                    </button>
                  {/if}
                  <button class="history-btn" on:click={() => handleHistoryDelete(entry.id)}>
                    {$t("history.delete")}
                  </button>
                </div>
              </li>
            {/each}
          </ul>
        {/if}

        <!-- ── Advanced ── -->
      {:else if activeSection === "advanced"}
        <h2>{$t("settings.nav.advanced")}</h2>

        <div class="group">
          <div class="row">
            <div class="row-label-stack">
              <span class="row-label">{$t("settings.screenshot")}</span>
              <span class="row-sub">{$t("settings.screenshot_desc")}</span>
            </div>
            <button
              class="toggle"
              class:on={screenshotContextEnabled}
              on:click={handleScreenshotContextToggle}
              aria-label="Toggle screenshot context"
            >
              <span class="toggle-knob"></span>
            </button>
          </div>
          <div class="row-sep"></div>
          <div class="row">
            <div class="row-label-stack">
              <span class="row-label">{$t("settings.polish")}</span>
              <span class="row-sub">{$t("settings.polish_desc")}</span>
            </div>
            <button
              class="toggle"
              class:on={polishEnabled}
              on:click={handlePolishToggle}
              aria-label="Toggle polish"
            >
              <span class="toggle-knob"></span>
            </button>
          </div>
          <div class="row-sep"></div>
          <div class="row">
            <div class="row-label-stack">
              <span class="row-label">{$t("settings.show_idle_hud")}</span>
              <span class="row-sub">{$t("settings.show_idle_hud_desc")}</span>
            </div>
            <button
              class="toggle"
              class:on={showIdleHud}
              on:click={handleShowIdleHudToggle}
              aria-label="Toggle idle HUD"
            >
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
    background: #1a1a1c;
    overflow: hidden;
    font-family: -apple-system, "SF Pro Text", BlinkMacSystemFont, sans-serif;
    -webkit-font-smoothing: antialiased;
    color: rgba(255, 255, 255, 0.88);
  }

  /* ── Title bar ── */
  .titlebar {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 44px;
    padding: 0 14px;
    background: #1e1e20;
    border-bottom: 1px solid rgba(255, 255, 255, 0.08);
    cursor: grab;
    flex-shrink: 0;
  }
  .titlebar:active {
    cursor: grabbing;
  }
  .titlebar-label {
    font-size: 13px;
    font-weight: 600;
    color: rgba(255, 255, 255, 0.85);
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
  .close-btn:hover {
    filter: brightness(0.88);
  }
  .close-btn svg {
    opacity: 0;
    transition: opacity 0.1s;
  }
  .close-btn:hover svg {
    opacity: 1;
  }

  .status-pill {
    font-size: 10px;
    font-weight: 500;
    padding: 2px 8px;
    border-radius: 999px;
    flex-shrink: 0;
  }
  .status-pill.recording {
    background: rgba(239, 68, 68, 0.15);
    color: #f87171;
  }
  .status-pill.processing {
    background: rgba(99, 130, 246, 0.12);
    color: #818cf8;
  }

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
    background: #141416;
    border-right: 1px solid rgba(255, 255, 255, 0.06);
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
    color: rgba(255, 255, 255, 0.5);
    font-size: 13.5px;
    font-weight: 400;
    text-align: left;
    cursor: pointer;
    transition:
      background 0.12s,
      color 0.12s;
    font-family: -apple-system, "SF Pro Text", BlinkMacSystemFont, sans-serif;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  .nav-item:hover {
    background: rgba(255, 255, 255, 0.06);
    color: rgba(255, 255, 255, 0.75);
  }
  .nav-item.active {
    background: rgba(99, 102, 241, 0.18);
    color: rgba(165, 163, 255, 0.95);
    font-weight: 500;
  }

  /* ── Content ── */
  .content {
    flex: 1;
    min-width: 0;
    padding: 20px 20px 24px;
    overflow-y: auto;
    background: #1a1a1c;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .content h2 {
    font-size: 20px;
    font-weight: 700;
    color: rgba(255, 255, 255, 0.92);
    margin: 0 0 10px;
    letter-spacing: -0.02em;
  }

  .content h3 {
    font-size: 13px;
    font-weight: 600;
    color: rgba(255, 255, 255, 0.45);
    margin: 10px 0 4px;
    letter-spacing: 0;
    text-transform: uppercase;
    font-size: 11px;
  }

  /* ── Groups (card with rows) ── */
  .group {
    background: rgba(255, 255, 255, 0.05);
    border-radius: 10px;
    border: 1px solid rgba(255, 255, 255, 0.08);
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
    background: rgba(255, 255, 255, 0.06);
    margin: 0 16px;
  }

  .row-label {
    font-size: 14px;
    color: rgba(255, 255, 255, 0.85);
    flex-shrink: 0;
  }

  .row-label-stack {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .row-sub {
    font-size: 12px;
    color: rgba(255, 255, 255, 0.35);
  }

  /* ── Controls ── */
  .row-select {
    font-size: 13px;
    color: rgba(255, 255, 255, 0.85);
    background: rgba(255, 255, 255, 0.08);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 7px;
    padding: 5px 7px;
    outline: none;
    cursor: pointer;
    max-width: 200px;
    font-family: -apple-system, "SF Pro Text", BlinkMacSystemFont, sans-serif;
  }

  .row-input {
    font-size: 13px;
    color: rgba(255, 255, 255, 0.88);
    background: rgba(255, 255, 255, 0.07);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 7px;
    padding: 6px 9px;
    outline: none;
    width: 100%;
    max-width: 200px;
    font-family: -apple-system, "SF Pro Text", BlinkMacSystemFont, sans-serif;
    transition:
      border-color 0.15s,
      background 0.15s;
  }
  .row-input:focus {
    border-color: #007aff;
    background: white;
  }
  .row-input::placeholder {
    color: #aeaeb2;
  }
  .row-input.mono {
    font-family: "SF Mono", "Fira Code", monospace;
    font-size: 12px;
  }
  .row-input:focus {
    border-color: rgba(129, 140, 248, 0.5);
    background: rgba(255, 255, 255, 0.1);
  }
  .row-input::placeholder {
    color: rgba(255, 255, 255, 0.2);
  }
  .row-input.mono {
    font-family: "SF Mono", "Fira Code", monospace;
    font-size: 12px;
  }

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
    border: none;
    background: rgba(99, 102, 241, 0.75);
    color: white;
    cursor: pointer;
    white-space: nowrap;
    flex-shrink: 0;
    font-family: -apple-system, "SF Pro Text", BlinkMacSystemFont, sans-serif;
    transition: background 0.1s;
  }
  .apply-btn:hover {
    background: rgba(99, 102, 241, 0.9);
  }

  /* macOS-style toggle */
  .toggle {
    position: relative;
    width: 44px;
    height: 26px;
    border-radius: 13px;
    border: none;
    background: rgba(255, 255, 255, 0.1);
    cursor: pointer;
    flex-shrink: 0;
    padding: 0;
    transition: background 0.2s;
  }
  .toggle.on {
    background: rgba(99, 102, 241, 0.85);
  }
  .toggle-knob {
    position: absolute;
    top: 3px;
    left: 3px;
    width: 20px;
    height: 20px;
    border-radius: 50%;
    background: white;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.35);
    transition: transform 0.2s cubic-bezier(0.4, 0, 0.2, 1);
  }

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
    background: rgba(99, 102, 241, 0.85);
    color: white;
    font-size: 13.5px;
    font-weight: 600;
    cursor: pointer;
    transition: background 0.1s;
    font-family: -apple-system, "SF Pro Text", BlinkMacSystemFont, sans-serif;
    letter-spacing: -0.01em;
  }
  .save-btn:hover:not(:disabled) {
    background: rgba(99, 102, 241, 1);
  }
  .save-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  /* Auth badge */
  .auth-badge {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    color: #f87171;
    padding: 6px 10px;
    border-radius: 7px;
    background: rgba(248, 113, 113, 0.1);
    border: 1px solid rgba(248, 113, 113, 0.2);
  }
  .auth-badge.ok {
    color: rgba(134, 239, 172, 0.9);
    background: rgba(74, 222, 128, 0.08);
    border-color: rgba(74, 222, 128, 0.2);
  }
  .auth-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: #f87171;
    flex-shrink: 0;
  }
  .auth-badge.ok .auth-dot {
    background: rgba(134, 239, 172, 0.9);
  }

  /* Misc text */
  .hint {
    font-size: 12px;
    color: rgba(255, 255, 255, 0.35);
    line-height: 1.5;
    margin: 0;
  }
  .hint :global(a) {
    color: rgba(129, 140, 248, 0.85);
    text-decoration: none;
  }
  .hint :global(code) {
    font-family: "SF Mono", "Fira Code", monospace;
    font-size: 10px;
    background: rgba(255, 255, 255, 0.08);
    padding: 1px 4px;
    border-radius: 3px;
  }

  .warn {
    font-size: 12px;
    color: rgba(251, 191, 36, 0.9);
    padding: 6px 10px;
    border-radius: 7px;
    background: rgba(251, 191, 36, 0.08);
    border: 1px solid rgba(251, 191, 36, 0.2);
    margin: 0;
  }

  .inline-error {
    font-size: 12px;
    color: #f87171;
  }

  .saved-note {
    font-size: 12px;
    color: rgba(134, 239, 172, 0.9);
    margin: 0;
  }

  /* ── History ── */
  .history-section-hint {
    font-size: 0.78rem;
    color: rgba(255, 255, 255, 0.45);
    line-height: 1.45;
    margin: -4px 0 12px;
    max-width: 520px;
  }
  .history-failed-preview {
    color: rgba(254, 202, 202, 0.92);
    font-size: 0.82rem;
    line-height: 1.4;
    word-break: break-word;
  }
  .history-empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 32px 16px;
    gap: 4px;
    text-align: center;
    border-radius: 10px;
    background: rgba(255, 255, 255, 0.03);
    border: 1px dashed rgba(255, 255, 255, 0.08);
  }
  .history-empty-title {
    font-size: 13.5px;
    color: rgba(255, 255, 255, 0.7);
    font-weight: 500;
    margin: 0;
  }
  .history-empty-hint {
    font-size: 12px;
    color: rgba(255, 255, 255, 0.35);
    line-height: 1.5;
    margin: 0;
  }

  .history-list {
    list-style: none;
    padding: 0;
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .history-item {
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid rgba(255, 255, 255, 0.06);
    border-radius: 9px;
    padding: 10px 12px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .history-item.failed {
    border-color: rgba(239, 68, 68, 0.28);
    background: rgba(239, 68, 68, 0.06);
  }

  .history-head {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
  }

  .history-time {
    font-size: 12px;
    color: rgba(255, 255, 255, 0.85);
    font-weight: 500;
    font-variant-numeric: tabular-nums;
  }

  .history-meta {
    font-size: 11px;
    color: rgba(255, 255, 255, 0.4);
    flex: 1;
    min-width: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .history-status {
    font-size: 10.5px;
    font-weight: 600;
    padding: 2px 7px;
    border-radius: 999px;
    flex-shrink: 0;
    letter-spacing: 0.02em;
    text-transform: uppercase;
  }
  .status-completed {
    background: rgba(74, 222, 128, 0.15);
    color: rgba(134, 239, 172, 0.95);
  }
  .status-failed {
    background: rgba(248, 113, 113, 0.15);
    color: #fca5a5;
  }
  .status-pending {
    background: rgba(129, 140, 248, 0.15);
    color: #a5b4fc;
  }

  .history-text {
    font-size: 13px;
    color: rgba(255, 255, 255, 0.78);
    line-height: 1.4;
    margin: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    display: -webkit-box;
    -webkit-line-clamp: 3;
    -webkit-box-orient: vertical;
    word-break: break-word;
  }

  .history-error {
    font-size: 12px;
    color: #fca5a5;
    line-height: 1.35;
    margin: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    -webkit-box-orient: vertical;
  }

  .history-actions {
    display: flex;
    gap: 6px;
    justify-content: flex-end;
  }

  .history-btn {
    font-size: 11.5px;
    font-weight: 500;
    padding: 4px 10px;
    border-radius: 6px;
    border: 1px solid rgba(255, 255, 255, 0.12);
    background: rgba(255, 255, 255, 0.05);
    color: rgba(255, 255, 255, 0.8);
    cursor: pointer;
    font-family: -apple-system, "SF Pro Text", BlinkMacSystemFont, sans-serif;
    transition:
      background 0.12s,
      border-color 0.12s;
  }
  .history-btn:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.1);
  }
  .history-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .history-btn.primary {
    background: rgba(99, 102, 241, 0.6);
    border-color: rgba(99, 102, 241, 0.75);
    color: white;
  }
  .history-btn.primary:hover:not(:disabled) {
    background: rgba(99, 102, 241, 0.85);
  }
</style>
