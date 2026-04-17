<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { LogicalSize, LogicalPosition } from "@tauri-apps/api/dpi";
  import type { UnlistenFn } from "@tauri-apps/api/event";
  import { createAppApi } from "./lib/app-api";
  import { log } from "./lib/logger";
  import {
    AX_H,
    AX_W,
    HUD_POS_KEY,
    applyAppStateChange,
    deriveUiDecision,
    parseAppState,
    SETTINGS_POS_KEY,
    type AppState,
    type UiModelState,
  } from "./lib/ui-model";

  async function savePos(key: string) {
    try {
      const phys = await appWindow.outerPosition();
      const factor = await appWindow.scaleFactor();
      localStorage.setItem(
        key,
        JSON.stringify({
          x: phys.x / factor,
          y: phys.y / factor,
        })
      );
    } catch {}
  }

  async function resizeTo(w: number, h: number, posKey?: string) {
    await appWindow.setSize(new LogicalSize(w, h));
    if (posKey) {
      try {
        const saved = localStorage.getItem(posKey);
        if (saved) {
          const { x, y } = JSON.parse(saved);
          await appWindow.setPosition(new LogicalPosition(x, y));
          return;
        }
      } catch {}
    }
    await appWindow.center();
  }

  import RecordingIndicator from "./lib/RecordingIndicator.svelte";
  import SettingsPanel from "./lib/SettingsPanel.svelte";
  import OnboardingFlow from "./lib/OnboardingFlow.svelte";
  import { t } from "./lib/i18n";

  let appState: AppState = "idle";
  let errorMsg = "";
  let initializationError = "";
  let lastTranscription = "";
  let showSettings = false;
  let injectionFailed = false;
  let needsAccessibilityRestart = false;
  let showOnboarding = false;
  let polishFailed = false;
  let shortcutConflict = "";

  // Settings data
  let polishEnabled = true;
  let audioDevices: string[] = [];
  let autostartEnabled = false;
  let screenshotContextEnabled = false;
  let showIdleHud = false;

  const INJECTION_FAILURE_DISPLAY_DURATION_MS = 1500;
  const POLISH_FAILURE_DISPLAY_DURATION_MS = 3000;

  const appApi = createAppApi();
  const appWindow = appApi.window;
  const unlisten: UnlistenFn[] = [];
  let injectionTimer: ReturnType<typeof setTimeout> | null = null;

  function clearInjectionTimer() {
    if (injectionTimer !== null) {
      clearTimeout(injectionTimer);
      injectionTimer = null;
    }
  }

  function getUiState(): UiModelState {
    return {
      onboardingDone: !showOnboarding,
      axGranted: !needsAccessibilityRestart,
      showSettings,
      appState,
      injectionFailed,
      polishFailed,
      showIdleHud,
    };
  }

  async function syncWindow() {
    const ui = deriveUiDecision(getUiState());

    log(
      `[syncWindow] view=${ui.view} opaque=${ui.nativeOpaque} show=${ui.shouldShowWindow} size=${ui.window.w}x${ui.window.h}`
    );

    await appApi.setNativeOpaque(ui.nativeOpaque, ui.shouldShowWindow);
    await appWindow.setResizable(ui.shouldShowWindow && ui.nativeOpaque);

    if (!ui.shouldShowWindow) {
      // Window stays on-screen at alphaValue=0 (set by setNativeOpaque above).
      // This keeps the CVDisplayLink alive so the WKWebView compositor never
      // suspends — the root cause of the all-black settings window bug.
      log("[syncWindow] window hidden via alphaValue=0 (compositor stays alive)");
      return;
    }

    log(
      `[syncWindow] showing window at ${ui.window.w}x${ui.window.h} posKey=${ui.window.posKey ?? "center"}`
    );
    await resizeTo(ui.window.w, ui.window.h, ui.window.posKey);
  }

  onMount(async () => {
    try {
      // Clear stale inline styles left by previous HMR cycles
      document.documentElement.removeAttribute("style");
      document.body.removeAttribute("style");
      localStorage.removeItem("window-opacity");

      // Show the window immediately (invisible via alphaValue=0, set by the
      // native side at startup). Keeping it on-screen ensures the CVDisplayLink
      // fires and the WKWebView compositor stays warm, so it never needs to
      // "wake up" when settings opens — the root cause of the all-black window.
      await appWindow.show();

      const onboardingDone = await appApi
        .invoke<boolean>("get_onboarding_completed")
        .catch(() => true);
      showOnboarding = !onboardingDone;

      const state = await appApi.invoke<string>("get_app_state").catch(() => "idle");
      handleStateChange(state);

      const axGranted = await appApi.invoke<boolean>("get_accessibility_status").catch(() => true);
      needsAccessibilityRestart = !axGranted;

      polishEnabled = await appApi.invoke<boolean>("get_polish_enabled").catch(() => true);
      audioDevices = await appApi.invoke<string[]>("list_audio_devices").catch(() => []);
      autostartEnabled = await appApi.invoke<boolean>("get_autostart_enabled").catch(() => false);
      screenshotContextEnabled = await appApi
        .invoke<boolean>("get_screenshot_context_enabled")
        .catch(() => false);
      showIdleHud = await appApi.invoke<boolean>("get_show_idle_hud").catch(() => false);

      unlisten.push(
        await appApi.listen<string>("state-change", async (e) => {
          clearInjectionTimer();
          const closingSettings =
            showSettings && (e.payload === "recording" || e.payload === "processing");
          if (closingSettings) {
            await savePos(SETTINGS_POS_KEY);
          }

          handleStateChange(e.payload);
          await syncWindow();

          if (e.payload === "idle" && injectionFailed) {
            injectionTimer = setTimeout(async () => {
              injectionFailed = false;
              await syncWindow();
            }, INJECTION_FAILURE_DISPLAY_DURATION_MS);
          }
        })
      );

      unlisten.push(
        await appApi.listen<string>("transcription-result", (e) => {
          lastTranscription = e.payload;
          clearInjectionTimer();
          injectionFailed = false;
        })
      );

      unlisten.push(
        await appApi.listen<string>("injection-failed", async (e) => {
          lastTranscription = e.payload;
          injectionFailed = true;
          await syncWindow();
        })
      );

      unlisten.push(
        await appApi.listen("api-key-missing", async () => {
          await savePos(HUD_POS_KEY);
          showSettings = true;
          await syncWindow();
        })
      );

      unlisten.push(
        await appApi.listen("show-settings", async () => {
          await savePos(HUD_POS_KEY);
          showSettings = true;
          await syncWindow();
        })
      );

      unlisten.push(
        await appApi.listen("accessibility-missing", async () => {
          errorMsg = "Accessibility permission required";
          appState = "error";
          needsAccessibilityRestart = true;
          await syncWindow();
        })
      );

      unlisten.push(
        await appApi.listen<boolean>("polish-changed", (e) => {
          polishEnabled = e.payload;
        })
      );

      unlisten.push(
        await appApi.listen("polish-failed", async () => {
          polishFailed = true;
          await syncWindow();
          setTimeout(async () => {
            polishFailed = false;
            await syncWindow();
          }, POLISH_FAILURE_DISPLAY_DURATION_MS);
        })
      );

      unlisten.push(
        await appApi.listen<string>("shortcut-conflict", async (e) => {
          shortcutConflict = e.payload;
          await savePos(HUD_POS_KEY);
          showSettings = true;
          await syncWindow();
        })
      );

      await syncWindow();
    } catch (err) {
      const parsed = parseAppState(`error:${err instanceof Error ? err.message : String(err)}`);
      initializationError = parsed.errorMsg;
      appState = parsed.appState;
      errorMsg = parsed.errorMsg;
      console.error("App initialization failed", err);
      try {
        await appApi.setNativeOpaque(true);
      } catch {}
      try {
        await resizeTo(AX_W, AX_H);
      } catch {}
    }
  });

  onDestroy(() => {
    clearInjectionTimer();
    unlisten.forEach((fn) => fn());
  });

  function handleStateChange(raw: string) {
    const transition = applyAppStateChange(getUiState(), raw);
    appState = transition.state.appState;
    showSettings = transition.state.showSettings;
    errorMsg = transition.errorMsg;
  }

  async function handleSettingsSaved() {
    await savePos(SETTINGS_POS_KEY);
    showSettings = false;
    await syncWindow();
  }

  async function handleSettingsClosed() {
    await savePos(SETTINGS_POS_KEY);
    showSettings = false;
    await syncWindow();
  }

  async function handleOnboardingDone() {
    showOnboarding = false;
    await syncWindow();
  }

  async function handleAccessibilityDismiss() {
    needsAccessibilityRestart = false;
    await syncWindow();
  }
</script>

<div class="container">
  {#if initializationError}
    <div class="fallback-banner" role="alert">
      <p class="title">{$t("hud.error")}</p>
      <p>{initializationError}</p>
    </div>
  {:else if showOnboarding}
    <OnboardingFlow on:done={handleOnboardingDone} />
  {:else if needsAccessibilityRestart}
    <div class="ax-banner">
      <div class="ax-icon">
        <svg width="20" height="20" viewBox="0 0 24 24" fill="none">
          <circle cx="12" cy="12" r="10" stroke="rgba(251,191,36,0.85)" stroke-width="2" />
          <line
            x1="12"
            y1="7"
            x2="12"
            y2="13"
            stroke="rgba(251,191,36,0.85)"
            stroke-width="2"
            stroke-linecap="round"
          />
          <circle cx="12" cy="17" r="1.2" fill="rgba(251,191,36,0.85)" />
        </svg>
      </div>
      <div class="ax-text">
        <p>{$t("ax.need")}</p>
        <p class="hint">{$t("ax.restart")}</p>
      </div>
      <div class="ax-buttons">
        <button class="primary" on:click={() => appApi.invoke("open_accessibility_prefs")}
          >{$t("ax.open")}</button
        >
        <button on:click={handleAccessibilityDismiss}>{$t("ax.dismiss")}</button>
      </div>
    </div>
  {:else if showSettings}
    <SettingsPanel
      bind:polishEnabled
      {audioDevices}
      bind:autostartEnabled
      bind:screenshotContextEnabled
      bind:showIdleHud
      {appState}
      bind:shortcutConflict
      on:saved={handleSettingsSaved}
      on:close={handleSettingsClosed}
    />
  {:else}
    <RecordingIndicator
      state={appState}
      {errorMsg}
      {lastTranscription}
      {injectionFailed}
      {polishFailed}
    />
  {/if}
</div>

<style>
  :global(*) {
    box-sizing: border-box;
    margin: 0;
    padding: 0;
  }

  :global(html),
  :global(body),
  :global(#app) {
    height: 100%;
    width: 100%;
  }

  :global(body) {
    background: transparent;
    font-family: -apple-system, BlinkMacSystemFont, "SF Pro Text", sans-serif;
    -webkit-font-smoothing: antialiased;
    user-select: none;
  }

  .container {
    position: relative;
    width: 100%;
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .container :global(.settings-root) {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
  }

  .fallback-banner {
    background: rgba(48, 20, 20, 0.92);
    border: 1px solid rgba(248, 113, 113, 0.35);
    border-radius: 16px;
    padding: 16px 18px;
    display: flex;
    flex-direction: column;
    gap: 6px;
    max-width: 320px;
    color: rgba(255, 255, 255, 0.9);
    box-shadow: 0 8px 40px rgba(0, 0, 0, 0.45);
    text-align: center;
  }

  .fallback-banner .title {
    color: #fca5a5;
    font-weight: 700;
  }

  .ax-banner {
    background: rgba(30, 30, 32, 0.88);
    backdrop-filter: blur(20px) saturate(180%);
    -webkit-backdrop-filter: blur(20px) saturate(180%);
    border: 1px solid rgba(251, 191, 36, 0.3);
    border-radius: 16px;
    padding: 16px 18px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    max-width: 300px;
    box-shadow: 0 8px 40px rgba(0, 0, 0, 0.45);
  }

  .ax-icon {
    display: flex;
    justify-content: center;
  }

  .ax-text {
    text-align: center;
    color: rgba(255, 255, 255, 0.85);
    font-size: 13px;
    line-height: 1.6;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .ax-text .hint {
    font-size: 11px;
    opacity: 0.6;
  }

  .ax-buttons {
    display: flex;
    gap: 8px;
    justify-content: center;
  }

  .ax-buttons button {
    padding: 6px 14px;
    border-radius: 8px;
    border: 1px solid rgba(255, 255, 255, 0.15);
    background: rgba(255, 255, 255, 0.08);
    color: rgba(255, 255, 255, 0.75);
    font-size: 12px;
    cursor: pointer;
    font-family: -apple-system, "SF Pro Text", BlinkMacSystemFont, sans-serif;
    transition: background 0.15s;
  }

  .ax-buttons button:hover {
    background: rgba(255, 255, 255, 0.14);
  }

  .ax-buttons button.primary {
    background: rgba(251, 191, 36, 0.25);
    border-color: rgba(251, 191, 36, 0.5);
    color: rgba(253, 224, 71, 0.95);
    font-weight: 600;
  }

  .ax-buttons button.primary:hover {
    background: rgba(251, 191, 36, 0.38);
  }
</style>
