/**
 * Standalone mount for RecordingIndicator.svelte — browser-testable HUD.
 * Svelte 4 — uses `$set()` for prop updates (no runes).
 *
 * Source sync: imports directly from src/lib/. Vite HMR auto-updates
 * when you edit RecordingIndicator.svelte — zero manual steps.
 */
import RecordingIndicator from "./lib/RecordingIndicator.svelte";
import { locale } from "./lib/i18n";

// ── State ──────────────────────────────────────────────────────────────────
let state = "idle";
let errorMsg = "";
let injectionFailed = false;
let polishFailed = false;
let composeContextStatus = "idle";
let composeError = "";
let audioLevels = new Array(20).fill(0);
let retryableSessionId = null;
let retrying = false;
let transcriptionSuccessFlash = false;
let successFlashDurationMs = 5000;
let transcriptionMode = "dictate";

let app;

/** Reset EVERY state field to default — called before each state switch. */
function resetAll() {
  stopSimLevels();
  state = "idle";
  errorMsg = "";
  injectionFailed = false;
  polishFailed = false;
  composeContextStatus = "idle";
  composeError = "";
  retryableSessionId = null;
  retrying = false;
  transcriptionSuccessFlash = false;
}

function syncProps() {
  if (!app) return;
  app.$set({ state, errorMsg, injectionFailed, polishFailed, composeContextStatus,
    composeError, audioLevels, retryableSessionId, retrying, transcriptionSuccessFlash,
    successFlashDurationMs, transcriptionMode });
}

// ── Simulated audio levels ────────────────────────────────────────────────
let simTimer = null;

function startSimLevels() {
  stopSimLevels();
  audioLevels = new Array(20).fill(0).map(function() { return Math.random() * 0.3; });
  syncProps();
  simTimer = setInterval(function() {
    if (state !== "recording") return;
    audioLevels = new Array(20).fill(0).map(function() { return Math.random() * 0.7 + 0.05; });
    syncProps();
  }, 120);
}

function stopSimLevels() {
  if (simTimer !== null) { clearInterval(simTimer); simTimer = null; }
  audioLevels = new Array(20).fill(0);
}

// ── Event handlers ────────────────────────────────────────────────────────
var handlers = {
  retry: function() {
    retrying = true; syncProps();
    setTimeout(function() {
      resetAll(); syncProps();
    }, 1500);
  },
  dismiss: function() { resetAll(); syncProps(); },
  clipboardCopy: function() {
    navigator.clipboard.writeText("Mock transcription text for testing");
  },
  clipboardDismiss: function() { resetAll(); syncProps(); },
  successCopy: function() {
    navigator.clipboard.writeText("Mock transcription text for testing");
  },
  composeDismiss: function() { resetAll(); syncProps(); },
  modeToggle: function() {
    transcriptionMode = transcriptionMode === "dictate" ? "smart_compose" : "dictate";
    syncProps();
  },
};

// ── Mount ─────────────────────────────────────────────────────────────────
app = new RecordingIndicator({
  target: document.getElementById("hud-root"),
  props: { state: state, errorMsg: errorMsg, injectionFailed: injectionFailed,
    polishFailed: polishFailed, composeContextStatus: composeContextStatus,
    composeError: composeError, audioLevels: audioLevels,
    retryableSessionId: retryableSessionId, retrying: retrying,
    transcriptionSuccessFlash: transcriptionSuccessFlash,
    successFlashDurationMs: successFlashDurationMs,
    transcriptionMode: transcriptionMode },
});
app.$on("retry", handlers.retry);
app.$on("dismiss", handlers.dismiss);
app.$on("clipboardCopy", handlers.clipboardCopy);
app.$on("clipboardDismiss", handlers.clipboardDismiss);
app.$on("successCopy", handlers.successCopy);
app.$on("composeDismiss", handlers.composeDismiss);
app.$on("modeToggle", handlers.modeToggle);

// ── Control panel — each button calls resetAll() first ────────────────────
var buttons = [
  { label: "Idle",
    apply: function() { resetAll(); syncProps(); },
    active: function() { return state === "idle" && !injectionFailed && !polishFailed
      && composeContextStatus !== "failed" && !transcriptionSuccessFlash
      && state !== "error"; } },
  { label: "Recording",
    apply: function() { resetAll(); state = "recording"; syncProps(); startSimLevels(); },
    active: function() { return state === "recording"; } },
  { label: "Processing",
    apply: function() { resetAll(); state = "processing"; syncProps(); },
    active: function() { return state === "processing"; } },
  { label: "Error (retryable)",
    apply: function() { resetAll(); state = "error";
      errorMsg = "Network timeout: failed to reach Groq API";
      retryableSessionId = "mock-session-001"; syncProps(); },
    active: function() { return state === "error" && !!retryableSessionId; } },
  { label: "Error (no retry)",
    apply: function() { resetAll(); state = "error";
      errorMsg = "Invalid API key"; retryableSessionId = null; syncProps(); },
    active: function() { return state === "error" && !retryableSessionId; } },
  { label: "Injection Failed",
    apply: function() { resetAll(); injectionFailed = true; syncProps(); },
    active: function() { return injectionFailed; } },
  { label: "Polish Failed",
    apply: function() { resetAll(); polishFailed = true; syncProps(); },
    active: function() { return polishFailed; } },
  { label: "Success Flash",
    apply: function() { resetAll(); transcriptionSuccessFlash = true; syncProps();
      setTimeout(function() { transcriptionSuccessFlash = false; state = "idle"; syncProps(); },
        successFlashDurationMs); },
    active: function() { return transcriptionSuccessFlash; } },
  { label: "Compose Failed",
    apply: function() { resetAll(); composeContextStatus = "failed";
      composeError = "__compose_failed__"; syncProps(); },
    active: function() { return composeContextStatus === "failed" && composeError === "__compose_failed__"; } },
  { label: "Context Failed",
    apply: function() { resetAll(); composeContextStatus = "failed";
      composeError = "Screenshot capture permission denied"; syncProps(); },
    active: function() { return composeContextStatus === "failed" && composeError !== "__compose_failed__"; } },
  { label: "Recording + Context",
    apply: function() { resetAll(); state = "recording";
      composeContextStatus = "ready"; syncProps(); startSimLevels(); },
    active: function() { return state === "recording" && composeContextStatus === "ready"; } },
];

// ── Controls UI ────────────────────────────────────────────────────────────
function el(tag, cls, html) {
  var e = document.createElement(tag);
  e.className = cls;
  if (html) e.innerHTML = html;
  return e;
}

function renderControls() {
  var c = document.getElementById("controls"); if (!c) return; c.innerHTML = "";

  // Mode toggle
  var modeRow = el("div", "control-row", '<span class="control-label">Mode:</span>');
  for (var i = 0; i < 2; i++) {
    var m = i === 0 ? "dictate" : "smart_compose";
    var b = document.createElement("button");
    b.textContent = m === "dictate" ? "Dictate" : "Smart Compose";
    b.className = transcriptionMode === m ? "active" : "";
    b.onclick = (function(mode) { return function() {
      transcriptionMode = mode; syncProps(); renderControls();
    }; })(m);
    modeRow.appendChild(b);
  }
  c.appendChild(modeRow);

  // Flash duration
  var durRow = el("div", "control-row", '<span class="control-label">Flash (s):</span>');
  var inp = document.createElement("input");
  inp.type = "number"; inp.value = String(successFlashDurationMs / 1000);
  inp.min = "1"; inp.max = "30"; inp.style.width = "50px";
  inp.onchange = function() {
    successFlashDurationMs = Math.max(1000, Math.min(30000, Number(inp.value) * 1000));
  };
  durRow.appendChild(inp); c.appendChild(durRow);

  // State buttons
  var stateRow = el("div", "control-row", '<span class="control-label">State:</span>');
  for (var j = 0; j < buttons.length; j++) {
    var btnData = buttons[j];
    var btn = document.createElement("button");
    btn.textContent = btnData.label;
    btn.className = btnData.active() ? "active" : "";
    btn.onclick = (function(bd) { return function() {
      bd.apply(); renderControls();
    }; })(btnData);
    stateRow.appendChild(btn);
  }
  c.appendChild(stateRow);
}

// ── Locale toggle ──────────────────────────────────────────────────────────
var langBtn = document.getElementById("toggle-lang");
if (langBtn) {
  langBtn.onclick = function() {
    locale.update(function(v) { return v === "en" ? "zh" : "en"; });
    langBtn.textContent = "EN / 中文";
  };
}

// ── Boot ───────────────────────────────────────────────────────────────────
renderControls();
setInterval(renderControls, 300);
