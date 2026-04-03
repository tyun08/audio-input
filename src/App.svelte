<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { invoke } from "@tauri-apps/api/core";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import RecordingIndicator from "./lib/RecordingIndicator.svelte";
  import SettingsPanel from "./lib/SettingsPanel.svelte";

  type AppState = "idle" | "recording" | "processing" | "error";

  let appState: AppState = "idle";
  let errorMsg = "";
  let lastTranscription = "";
  let showSettings = false;
  let injectionFailed = false;
  let needsAccessibilityRestart = false;

  const appWindow = getCurrentWindow();
  const unlisten: UnlistenFn[] = [];

  onMount(async () => {
    // 获取当前状态
    const state = await invoke<string>("get_app_state");
    handleStateChange(state);

    // 检查 Accessibility 权限（命令查询，避免事件竞争）
    const axGranted = await invoke<boolean>("get_accessibility_status");
    if (!axGranted) {
      needsAccessibilityRestart = true;
      appWindow.show();
    }

    // 监听状态变化
    unlisten.push(
      await listen<string>("state-change", (e) => {
        handleStateChange(e.payload);
        // 录音或处理中时显示浮窗，idle 时隐藏（设置面板打开时不隐藏）
        if (e.payload === "recording" || e.payload === "processing") {
          appWindow.show();
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

    // 监听注入失败（文字已在剪贴板，需要用户手动粘贴）
    unlisten.push(
      await listen<string>("injection-failed", (e) => {
        lastTranscription = e.payload;
        injectionFailed = true;
        appWindow.show();
      })
    );

    // 监听需要配置 API Key
    unlisten.push(
      await listen("api-key-missing", () => {
        showSettings = true;
        appWindow.show();
      })
    );

    // 监听托盘菜单打开设置
    unlisten.push(
      await listen("show-settings", () => {
        showSettings = true;
      })
    );

    // 监听辅助功能权限缺失（注入失败时）
    unlisten.push(
      await listen("accessibility-missing", () => {
        errorMsg = "请在系统设置中授予辅助功能权限";
        appState = "error";
        appWindow.show();
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

  function handleSettingsSaved() {
    showSettings = false;
    if (appState === "idle") appWindow.hide();
  }

  function handleSettingsClosed() {
    showSettings = false;
    if (appState === "idle") appWindow.hide();
  }
</script>

<div class="container">
  {#if needsAccessibilityRestart}
    <div class="ax-banner">
      <p>需要<strong>辅助功能</strong>权限才能自动注入文字</p>
      <p class="hint">在系统设置中授权后，请<strong>完全退出并重启 App</strong></p>
      <div class="ax-buttons">
        <button class="primary" on:click={() => invoke("open_accessibility_prefs")}>打开系统设置</button>
        <button on:click={() => (needsAccessibilityRestart = false)}>忽略</button>
      </div>
    </div>
  {:else if showSettings}
    <SettingsPanel on:saved={handleSettingsSaved} on:close={handleSettingsClosed} />
  {:else}
    <RecordingIndicator state={appState} {errorMsg} {lastTranscription} {injectionFailed} />
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
    background: rgba(255, 200, 0, 0.15);
    border: 1px solid rgba(255, 180, 0, 0.5);
    border-radius: 10px;
    padding: 14px 18px;
    text-align: center;
    color: #fff;
    font-size: 13px;
    line-height: 1.6;
    display: flex;
    flex-direction: column;
    gap: 6px;
    max-width: 320px;
  }

  .ax-banner .hint {
    font-size: 12px;
    opacity: 0.85;
  }

  .ax-buttons {
    display: flex;
    gap: 8px;
    justify-content: center;
    margin-top: 4px;
  }

  .ax-banner button {
    padding: 5px 14px;
    border-radius: 6px;
    border: 1px solid rgba(255,255,255,0.3);
    background: rgba(255,255,255,0.15);
    color: #fff;
    font-size: 12px;
    cursor: pointer;
  }

  .ax-banner button.primary {
    background: rgba(255, 200, 0, 0.4);
    border-color: rgba(255, 200, 0, 0.7);
    font-weight: 600;
  }

  .ax-banner button:hover {
    background: rgba(255,255,255,0.25);
  }

  .ax-banner button.primary:hover {
    background: rgba(255, 200, 0, 0.55);
  }
</style>
