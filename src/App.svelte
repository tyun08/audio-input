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

  const appWindow = getCurrentWindow();
  const unlisten: UnlistenFn[] = [];

  onMount(async () => {
    // 获取当前状态
    const state = await invoke<string>("get_app_state");
    handleStateChange(state);

    // 监听状态变化
    unlisten.push(
      await listen<string>("state-change", (e) => {
        handleStateChange(e.payload);
        // 录音或处理中时显示浮窗，idle 时隐藏（设置面板打开时不隐藏）
        if (e.payload === "recording" || e.payload === "processing") {
          appWindow.show();
        } else if (e.payload === "idle" && !showSettings && !injectionFailed) {
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

    // 监听辅助功能权限缺失
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
  {#if showSettings}
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
</style>
