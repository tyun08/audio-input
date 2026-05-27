/**
 * Standalone mount for SettingsPanel.svelte — browser-testable settings.
 * Svelte 4 — uses `$set()` for prop updates (no runes).
 */
import SettingsPanel from "./lib/SettingsPanel.svelte";
import { locale } from "./lib/i18n";

let polishEnabled = false;
let audioDevices = ["System Default", "MacBook Pro Microphone", "External USB Mic"];
let autostartEnabled = false;
let screenshotContextEnabled = false;
let showIdleHud = false;
let sentHudTimeoutSecs = 5;
let appState = "idle";
let transcriptionMode = "dictate";
let shortcutConflict = "";

let app;

function syncProps() {
  if (!app) return;
  app.$set({ polishEnabled, audioDevices, autostartEnabled, screenshotContextEnabled,
    showIdleHud, sentHudTimeoutSecs, appState, transcriptionMode, shortcutConflict });
}

app = new SettingsPanel({
  target: document.getElementById("settings-root"),
  props: { polishEnabled, audioDevices, autostartEnabled, screenshotContextEnabled,
    showIdleHud, sentHudTimeoutSecs, appState, transcriptionMode, shortcutConflict },
});

const stateBtn = document.getElementById("sim-state");
if (stateBtn) {
  const states = ["idle", "recording", "processing"];
  let idx = 0;
  stateBtn.onclick = () => {
    idx = (idx + 1) % states.length;
    appState = states[idx];
    stateBtn.textContent = "App: " + states[idx];
    syncProps();
  };
}

const langBtn = document.getElementById("toggle-lang");
if (langBtn) {
  langBtn.onclick = () => {
    locale.update((v) => v === "en" ? "zh" : "en");
    langBtn.textContent = "EN / 中文";
  };
}

export default app;
