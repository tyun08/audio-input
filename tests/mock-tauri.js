/**
 * Tauri IPC mock injected by Playwright before the app loads.
 *
 * Sets up window.__TAURI_INTERNALS__ so that @tauri-apps/api's invoke,
 * listen and window helpers work in a plain browser context.
 *
 * Test helpers exposed on window:
 *   window.__tauriEmit(event, payload)    – simulate a backend→frontend event
 *   window.__tauriSetResponse(cmd, value) – override a command's return value
 */
(function () {
  "use strict";

  window.__TAURI_INTERNALS__ = window.__TAURI_INTERNALS__ || {};
  window.__TAURI_EVENT_PLUGIN_INTERNALS__ =
    window.__TAURI_EVENT_PLUGIN_INTERNALS__ || {};

  // ── Window metadata (required by getCurrentWindow()) ─────────────────────
  window.__TAURI_INTERNALS__.metadata = {
    currentWindow: { label: "main" },
    currentWebview: { windowLabel: "main", label: "main" },
  };

  // ── Callback registry ─────────────────────────────────────────────────────
  const callbacks = new Map();

  function transformCallback(fn, once) {
    const id = window.crypto.getRandomValues(new Uint32Array(1))[0];
    callbacks.set(id, once
      ? function (data) { callbacks.delete(id); fn && fn(data); }
      : fn
    );
    return id;
  }

  function runCallback(id, data) {
    const cb = callbacks.get(id);
    if (cb) cb(data);
  }

  function unregisterCallback(id) {
    callbacks.delete(id);
  }

  // ── Event listener registry ───────────────────────────────────────────────
  const eventListeners = new Map();

  // ── Default command responses ─────────────────────────────────────────────
  const defaultResponses = {
    // App state
    get_onboarding_completed: true,
    get_app_state: "idle",
    get_accessibility_status: true,
    // Settings
    get_polish_enabled: true,
    list_audio_devices: ["Default Microphone", "USB Microphone"],
    get_autostart_enabled: false,
    get_screenshot_context_enabled: false,
    get_show_idle_hud: false,
    get_success_hud_width: 560,
    get_provider: "groq",
    get_provider_config: { api_key: "" },
    get_shortcut: "Meta+Shift+Space",
    get_preferred_device: null,
    // Persistence (no-ops)
    save_provider: null,
    save_provider_config: null,
    save_polish_enabled: null,
    save_preferred_device: null,
    save_shortcut: null,
    save_autostart_enabled: null,
    save_screenshot_context_enabled: null,
    save_show_idle_hud: null,
    save_success_hud_width: null,
    set_native_opaque: null,
    // Provider auth
    check_provider_status: false,
    // Other
    open_accessibility_prefs: null,
  };

  // Per-test overrides
  window.__tauriMockOverrides = {};

  // ── IPC invoke handler ────────────────────────────────────────────────────
  async function handleInvoke(cmd, args) {
    // Event plugin
    if (cmd === "plugin:event|listen") {
      const event = args.event;
      if (!eventListeners.has(event)) eventListeners.set(event, []);
      eventListeners.get(event).push(args.handler);
      return args.handler; // eventId
    }
    if (cmd === "plugin:event|emit") {
      (eventListeners.get(args.event) || []).forEach(function (hid) {
        runCallback(hid, { event: args.event, payload: args.payload });
      });
      return null;
    }
    if (cmd === "plugin:event|unlisten") {
      const ls = eventListeners.get(args.event);
      if (ls) {
        const idx = ls.indexOf(args.eventId);
        if (idx !== -1) ls.splice(idx, 1);
      }
      return null;
    }

    // Window / plugin commands – no-op or sensible defaults
    if (cmd.startsWith("plugin:window|")) {
      const windowResponses = {
        "plugin:window|scale_factor": 1,
        "plugin:window|outer_position": { x: 100, y: 100 },
        "plugin:window|inner_size": { width: 200, height: 44 },
        "plugin:window|outer_size": { width: 200, height: 44 },
      };
      return windowResponses[cmd] !== undefined ? windowResponses[cmd] : null;
    }
    if (
      cmd.startsWith("plugin:global-shortcut|") ||
      cmd.startsWith("plugin:notification|")
    ) {
      return null;
    }

    // Per-test override
    if (Object.prototype.hasOwnProperty.call(window.__tauriMockOverrides, cmd)) {
      return window.__tauriMockOverrides[cmd];
    }

    // Default
    if (Object.prototype.hasOwnProperty.call(defaultResponses, cmd)) {
      return defaultResponses[cmd];
    }

    console.warn("[TAURI MOCK] Unknown command:", cmd, args);
    return null;
  }

  // Attach to internals
  window.__TAURI_INTERNALS__.invoke = handleInvoke;
  window.__TAURI_INTERNALS__.transformCallback = transformCallback;
  window.__TAURI_INTERNALS__.unregisterCallback = unregisterCallback;
  window.__TAURI_INTERNALS__.runCallback = runCallback;
  window.__TAURI_INTERNALS__.callbacks = callbacks;

  window.__TAURI_EVENT_PLUGIN_INTERNALS__.unregisterListener = function (
    _event,
    id
  ) {
    unregisterCallback(id);
  };

  // ── Test helpers ──────────────────────────────────────────────────────────

  /** Simulate a Tauri backend → frontend event emission. */
  window.__tauriEmit = function (event, payload) {
    (eventListeners.get(event) || []).forEach(function (hid) {
      runCallback(hid, { event: event, payload: payload });
    });
  };

  /** Override a command's return value for the current test. */
  window.__tauriSetResponse = function (cmd, value) {
    window.__tauriMockOverrides[cmd] = value;
  };
})();
