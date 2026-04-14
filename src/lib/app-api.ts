import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { listen as tauriListen, type EventCallback, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";

type AppWindow = Pick<
  ReturnType<typeof getCurrentWindow>,
  "show" | "hide" | "setSize" | "setPosition" | "center" | "outerPosition" | "scaleFactor"
>;

export interface AppApi {
  window: AppWindow;
  invoke: <T>(cmd: string, args?: Record<string, unknown>) => Promise<T>;
  listen: <T>(event: string, handler: EventCallback<T>) => Promise<UnlistenFn>;
  setNativeOpaque: (opaque: boolean) => Promise<void>;
}

function hasTauriWindow(): boolean {
  if (typeof window === "undefined") {
    return false;
  }

  const tauriWindow = window as Window & {
    __TAURI_INTERNALS__?: {
      metadata?: {
        currentWindow?: unknown;
      };
    };
  };

  return Boolean(tauriWindow.__TAURI_INTERNALS__?.metadata?.currentWindow);
}

export function createAppApi(): AppApi {
  const fallbackWindow: AppWindow = {
    show: async () => {},
    hide: async () => {},
    setSize: async () => {},
    setPosition: async () => {},
    center: async () => {},
    outerPosition: async () => ({ x: 0, y: 0 }),
    scaleFactor: async () => 1,
  };
  const inTauri = hasTauriWindow();

  return {
    window: inTauri ? getCurrentWindow() : fallbackWindow,
    invoke<T>(cmd: string, args?: Record<string, unknown>) {
      if (!inTauri) {
        return Promise.reject(new Error(`Failed to invoke ${cmd}: Tauri runtime unavailable`));
      }

      return tauriInvoke<T>(cmd, args);
    },
    listen<T>(event: string, handler: EventCallback<T>) {
      if (!inTauri) {
        return Promise.reject(new Error(`Failed to listen to ${event}: Tauri runtime unavailable`));
      }

      return tauriListen<T>(event, handler);
    },
    setNativeOpaque(opaque: boolean) {
      if (!inTauri) {
        return Promise.resolve();
      }

      return tauriInvoke("set_native_opaque", { opaque });
    },
  };
}
