<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { invoke } from "@tauri-apps/api/core";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { LogicalSize, LogicalPosition } from "@tauri-apps/api/dpi";

  const HUD_W = 200, HUD_H = 44;
  const SETTINGS_W = 340, SETTINGS_H = 620;
  const ONBOARDING_W = 370, ONBOARDING_H = 540;
  const AX_W = 320, AX_H = 160;

  const HUD_POS_KEY = "hud-window-pos";
  const SETTINGS_POS_KEY = "settings-window-pos";

  async function savePos(key: string) {
    try {
      const phys = await appWindow.outerPosition();
      const factor = await appWindow.scaleFactor();
      localStorage.setItem(key, JSON.stringify({
        x: phys.x / factor,
        y: phys.y / factor,
      }));
    } catch {}
  }

  async function resizeTo(w: number, h: number, posKey?: string) {
    await appWindow.show();
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

  type AppState = "idle" | "recording" | "processing" | "error";

  let appState: AppState = "idle";
  let errorMsg = "";
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

  const appWindow = getCurrentWindow();
  const unlisten: UnlistenFn[] = [];

  onMount(async () => {
    // Check onboarding
    const onboardingDone = await invoke<boolean>("get_onboarding_completed").catch(() => true);
    if (!onboardingDone) {
      showOnboarding = true;
      resizeTo(ONBOARDING_W, ONBOARDING_H);
    }

    // 获取当前状态
    const state = await invoke<string>("get_app_state");
    handleStateChange(state);

    // 检查 Accessibility 权限
    const axGranted = await invoke<boolean>("get_accessibility_status");
    if (!axGranted) {
      needsAccessibilityRestart = true;
      resizeTo(AX_W, AX_H);
    }

    // Load settings data
    polishEnabled = await invoke<boolean>("get_polish_enabled").catch(() => true);
    audioDevices = await invoke<string[]>("list_audio_devices").catch(() => []);
    autostartEnabled = await invoke<boolean>("get_autostart_enabled").catch(() => false);
    screenshotContextEnabled = await invoke<boolean>("get_screenshot_context_enabled").catch(() => false);

    // 监听状态变化
    unlisten.push(
      await listen<string>("state-change", (e) => {
        handleStateChange(e.payload);
        if (e.payload === "recording" || e.payload === "processing") {
          resizeTo(HUD_W, HUD_H, HUD_POS_KEY);
        } else if (e.payload === "idle" && !showSettings && !injectionFailed && !needsAccessibilityRestart) {
          setTimeout(() => appWindow.hide(), 800);
        } else if (e.payload === "idle") {
          injectionFailed = false;
        }
      })
    );

    // 监听转录结果
    unlisten.push(
      await listen<string>("transcription-result", (e) => {
        lastTranscription = e.payload;
        injectionFailed = false;
      })
    );

    // 监听注入失败
    unlisten.push(
      await listen<string>("injection-failed", (e) => {
        lastTranscription = e.payload;
        injectionFailed = true;
        resizeTo(HUD_W, 72, HUD_POS_KEY);
      })
    );

    // 监听需要配置 API Key
    unlisten.push(
      await listen("api-key-missing", async () => {
        await savePos(HUD_POS_KEY);
        showSettings = true;
        await resizeTo(SETTINGS_W, SETTINGS_H, SETTINGS_POS_KEY);
      })
    );

    // 监听托盘菜单打开设置
    unlisten.push(
      await listen("show-settings", async () => {
        await savePos(HUD_POS_KEY);
        showSettings = true;
        await resizeTo(SETTINGS_W, SETTINGS_H, SETTINGS_POS_KEY);
      })
    );

    // 监听辅助功能权限缺失
    unlisten.push(
      await listen("accessibility-missing", () => {
        errorMsg = "请在系统设置中授予辅助功能权限";
        appState = "error";
        appWindow.show();
      })
    );

    // 监听润色失败
    unlisten.push(
      await listen("polish-failed", () => {
        polishFailed = true;
        setTimeout(() => { polishFailed = false; }, 3000);
      })
    );

    // 监听快捷键冲突
    unlisten.push(
      await listen<string>("shortcut-conflict", async (e) => {
        shortcutConflict = e.payload;
        await savePos(HUD_POS_KEY);
        showSettings = true;
        await resizeTo(SETTINGS_W, SETTINGS_H, SETTINGS_POS_KEY);
      })
    );
  });

  onDestroy(() => {
    unlisten.forEach((fn) => fn());
  });

  function handleStateChange(raw: string) {
    if (raw.startsWith("error:")) {
      appState = "error";
      errorMsg = raw.slice(6);
    } else {
      appState = raw as AppState;
      if (appState !== "error") errorMsg = "";
    }
  }

  async function handleSettingsSaved() {
    await savePos(SETTINGS_POS_KEY);
    showSettings = false;
    if (appState === "idle") { appWindow.hide(); } else { await resizeTo(HUD_W, HUD_H, HUD_POS_KEY); }
  }

  async function handleSettingsClosed() {
    await savePos(SETTINGS_POS_KEY);
    showSettings = false;
    if (appState === "idle") { appWindow.hide(); } else { await resizeTo(HUD_W, HUD_H, HUD_POS_KEY); }
  }

  async function handleOnboardingDone() {
    showOnboarding = false;
    if (appState === "idle" && !needsAccessibilityRestart) {
      appWindow.hide();
    } else {
      await resizeTo(HUD_W, HUD_H);
    }
  }
</script>

<div class="container">
  {#if showOnboarding}
    <OnboardingFlow on:done={handleOnboardingDone} />
  {:else if needsAccessibilityRestart}
    <div class="ax-banner">
      <div class="ax-icon">
        <svg width="20" height="20" viewBox="0 0 24 24" fill="none">
          <circle cx="12" cy="12" r="10" stroke="rgba(251,191,36,0.85)" stroke-width="2"/>
          <line x1="12" y1="7" x2="12" y2="13" stroke="rgba(251,191,36,0.85)" stroke-width="2" stroke-linecap="round"/>
          <circle cx="12" cy="17" r="1.2" fill="rgba(251,191,36,0.85)"/>
        </svg>
      </div>
      <div class="ax-text">
        <p>需要<strong>辅助功能</strong>权限才能自动注入文字</p>
        <p class="hint">授权后请完全退出并重启 App</p>
      </div>
      <div class="ax-buttons">
        <button class="primary" on:click={() => invoke("open_accessibility_prefs")}>打开系统设置</button>
        <button on:click={() => (needsAccessibilityRestart = false)}>忽略</button>
      </div>
    </div>
  {:else if showSettings}
    <SettingsPanel
      bind:polishEnabled
      {audioDevices}
      bind:autostartEnabled
      bind:screenshotContextEnabled
      appState={appState}
      bind:shortcutConflict
      on:saved={handleSettingsSaved}
      on:close={handleSettingsClosed}
    />
  {:else}
    <RecordingIndicator state={appState} {errorMsg} {lastTranscription} {injectionFailed} {polishFailed} />
  {/if}
</div>

<style>
  :global(*) {
    box-sizing: border-box;
    margin: 0;
    padding: 0;
  }

  :global(body) {
    background: transparent;
    font-family: -apple-system, BlinkMacSystemFont, "SF Pro Text", sans-serif;
    -webkit-font-smoothing: antialiased;
    user-select: none;
  }

  .container {
    width: 100%;
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
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
    box-shadow: 0 8px 40px rgba(0,0,0,0.45);
  }

  .ax-icon {
    display: flex;
    justify-content: center;
  }

  .ax-text {
    text-align: center;
    color: rgba(255,255,255,0.85);
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
    border: 1px solid rgba(255,255,255,0.15);
    background: rgba(255,255,255,0.08);
    color: rgba(255,255,255,0.75);
    font-size: 12px;
    cursor: pointer;
    font-family: -apple-system, "SF Pro Text", BlinkMacSystemFont, sans-serif;
    transition: background 0.15s;
  }

  .ax-buttons button:hover {
    background: rgba(255,255,255,0.14);
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
