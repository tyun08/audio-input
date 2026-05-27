/**
 * Mock @tauri-apps/api/core for standalone browser testing.
 *
 * Override `mockStore` to simulate backend responses for specific commands.
 * Access it from the browser console: `__tauriMock.invoke.mockResolvedValue('get_provider', 'groq')`
 */

export type MockStore = Record<string, unknown>;

// Stubs for @tauri-apps/plugin-updater (and any other plugin importing these)
export class Resource {
  /** @internal stub */
}
export class Channel<T = unknown> {
  onmessage?: (payload: T) => void;
  /** @internal stub */
}

let store: MockStore = {
  // Transcription provider
  get_provider: "groq",
  save_provider: null,

  // Provider config
  get_provider_config: {
    api_key: "gsk_mock_key_for_testing",
    model: "whisper-large-v3-turbo",
  },
  save_provider_config: null,

  // Shortcut
  get_shortcut: "Meta+Shift+Space",
  save_shortcut: null,

  // Audio devices
  list_audio_devices: ["System Default", "MacBook Pro Microphone", "External USB Mic"],
  get_preferred_device: null,
  save_preferred_device: null,

  // Polish
  save_polish_enabled: null,
  save_screenshot_context_enabled: null,
  save_show_idle_hud: null,
  save_sent_hud_timeout: null,

  // Autostart
  save_autostart: null,

  // Smart compose
  toggle_smart_compose: null,

  // Locale
  get_locale: "en",
  save_locale: null,

  // History
  get_max_history: 100,
  save_max_history: null,
  list_history: [],
  retry_transcription: null,
  delete_history_entry: null,
  clear_history: null,

  // AI action configs
  get_ai_action_config: {
    provider: "",
    effective_provider: "groq",
    model: "openai/gpt-oss-20b",
    vision_model: "meta-llama/llama-4-scout-17b-16e-instruct",
    prompt: "You are a helpful assistant. Fix any typos and add punctuation.",
    default_model: "openai/gpt-oss-20b",
    default_vision_model: "meta-llama/llama-4-scout-17b-16e-instruct",
    default_prompt: "You are a helpful assistant. Fix any typos and add punctuation.",
  },
  save_ai_action_config: null,
  get_ai_action_defaults: {
    default_model: "openai/gpt-oss-20b",
    default_vision_model: "meta-llama/llama-4-scout-17b-16e-instruct",
    default_prompt: "You are a helpful assistant. Fix any typos and add punctuation.",
  },

  // Auth checks
  check_provider_status: true,
};

export function getMockStore(): MockStore {
  return store;
}

export function setMockStore(updates: Partial<MockStore>): void {
  store = { ...store, ...updates };
}

export function resetMockStore(): void {
  store = {
    get_provider: "groq",
    save_provider: null,
    get_provider_config: { api_key: "gsk_mock_key_for_testing", model: "whisper-large-v3-turbo" },
    save_provider_config: null,
    get_shortcut: "Meta+Shift+Space",
    save_shortcut: null,
    list_audio_devices: ["System Default", "MacBook Pro Microphone", "External USB Mic"],
    get_preferred_device: null,
    save_preferred_device: null,
    save_polish_enabled: null,
    save_screenshot_context_enabled: null,
    save_show_idle_hud: null,
    save_sent_hud_timeout: null,
    save_autostart: null,
    toggle_smart_compose: null,
    get_locale: "en",
    save_locale: null,
    get_max_history: 100,
    save_max_history: null,
    list_history: [],
    retry_transcription: null,
    delete_history_entry: null,
    clear_history: null,
    get_ai_action_config: {
      provider: "",
      effective_provider: "groq",
      model: "openai/gpt-oss-20b",
      vision_model: "meta-llama/llama-4-scout-17b-16e-instruct",
      prompt: "You are a helpful assistant. Fix any typos and add punctuation.",
      default_model: "openai/gpt-oss-20b",
      default_vision_model: "meta-llama/llama-4-scout-17b-16e-instruct",
      default_prompt: "You are a helpful assistant. Fix any typos and add punctuation.",
    },
    save_ai_action_config: null,
    get_ai_action_defaults: {
      default_model: "openai/gpt-oss-20b",
      default_vision_model: "meta-llama/llama-4-scout-17b-16e-instruct",
      default_prompt: "You are a helpful assistant. Fix any typos and add punctuation.",
    },
    check_provider_status: true,
  };
}

// Expose mock store on window for console-based testing
if (typeof window !== "undefined") {
  (window as Record<string, unknown>).__tauriMock = {
    store,
    invoke: {
      mockResolvedValue(cmd: string, value: unknown) {
        store[cmd] = value;
      },
      mockRejectedValue(cmd: string, error: string) {
        store[cmd] = { __reject: error };
      },
      reset() {
        resetMockStore();
      },
    },
  };
}

export async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  // Simulate a small async delay for realism
  await new Promise((r) => setTimeout(r, 10));

  const value = store[cmd];

  if (value !== undefined && value !== null && typeof value === "object" && "__reject" in value) {
    throw new Error((value as { __reject: string }).__reject);
  }

  // Commands that accept args to update state
  if (cmd === "save_provider" && args) store.get_provider = args.provider;
  if (cmd === "save_provider_config" && args) store.get_provider_config = args.configValues;
  if (cmd === "save_shortcut" && args) store.get_shortcut = args.shortcut;
  if (cmd === "save_max_history" && args) store.get_max_history = args.max;
  if (cmd === "save_preferred_device" && args) store.get_preferred_device = args.device;

  return value as T;
}
