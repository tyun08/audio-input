export const HUD_W = 200;
export const HUD_H = 44;
export const HUD_ALERT_W = 300;
export const HUD_ALERT_H = 108;
export const HUD_RETRY_W = 300;
export const HUD_RETRY_H = 108;
export const SETTINGS_W = 620;
export const SETTINGS_H = 560;
export const ONBOARDING_W = 500;
export const ONBOARDING_H = 560;
export const AX_W = 320;
export const AX_H = 160;
export const HEALTH_W = 320;
export const HEALTH_H = 260;

export const HUD_POS_KEY = "hud-window-pos";
export const SETTINGS_POS_KEY = "settings-window-pos";

export type AppState = "idle" | "recording" | "processing" | "error";
export type UiView = "onboarding" | "mic" | "ax" | "settings" | "health" | "hud";

/** Menu-bar/status-popover health snapshot (mirrors the Rust `HealthStatus`). */
export interface HealthStatus {
  micOk: boolean;
  micFound: boolean;
  axOk: boolean;
  apiOk: boolean;
}

export function isHealthy(health: HealthStatus): boolean {
  return health.micOk && health.micFound && health.axOk && health.apiOk;
}

export interface UiModelState {
  onboardingDone: boolean;
  micGranted: boolean;
  axGranted: boolean;
  showSettings: boolean;
  appState: AppState;
  injectionFailed: boolean;
  polishFailed: boolean;
  showIdleHud?: boolean;
  /** Brief checkmark after inject succeeds (typed into focused app). */
  transcriptionSuccessFlash?: boolean;
  /** Non-null when a transcription attempt failed and a retryable session is available. */
  retryableSessionId?: string | null;
  /** True while the menu-bar status popover (left-click on an unhealthy icon) is open. */
  showHealthPopover?: boolean;
  /**
   * True after a shortcut-triggered recording attempt failed a health check
   * (no mic device / transcription API not configured). Stays visible until
   * the user reacts — it must not auto-timeout like other HUD states.
   */
  healthCheckFailed?: boolean;
}

export interface UiDecision {
  view: UiView;
  window: {
    w: number;
    h: number;
    posKey?: string;
  };
  nativeOpaque: boolean;
  shouldShowWindow: boolean;
}

export interface AppStateTransition {
  state: UiModelState;
  errorMsg: string;
}

export function parseAppState(raw: string): { appState: AppState; errorMsg: string } {
  if (raw.startsWith("error:")) {
    return {
      appState: "error",
      errorMsg: raw.slice(6),
    };
  }

  return {
    appState: raw as AppState,
    errorMsg: "",
  };
}

export function applyAppStateChange(state: UiModelState, raw: string): AppStateTransition {
  const parsed = parseAppState(raw);
  const shouldCloseSettings = parsed.appState === "recording" || parsed.appState === "processing";

  return {
    state: {
      ...state,
      appState: parsed.appState,
      showSettings: shouldCloseSettings ? false : state.showSettings,
    },
    errorMsg: parsed.errorMsg,
  };
}

export function deriveUiDecision(state: UiModelState): UiDecision {
  if (!state.onboardingDone) {
    return {
      view: "onboarding",
      window: { w: ONBOARDING_W, h: ONBOARDING_H },
      nativeOpaque: true,
      shouldShowWindow: true,
    };
  }

  // An explicit request to open Settings wins over the mic/AX permission nag
  // banners. Otherwise a pending permission warning silently swallows the tray
  // "Settings…" click (and the dock/Reopen path) and the window never appears —
  // the bug where Settings becomes unreachable after a permission event fires.
  // The Settings panel itself exposes the permission re-setup entry, so the nag
  // is still reachable; it simply no longer blocks Settings. This does not
  // apply when the health popover is also pending: that popover is itself the
  // result of an explicit user action (a failed shortcut/tray click) and
  // should take priority over a merely-open Settings window.
  if (state.showSettings && !state.showHealthPopover) {
    return {
      view: "settings",
      window: { w: SETTINGS_W, h: SETTINGS_H, posKey: SETTINGS_POS_KEY },
      nativeOpaque: true,
      shouldShowWindow: true,
    };
  }

  if (!state.micGranted) {
    return {
      view: "mic",
      window: { w: AX_W, h: AX_H },
      nativeOpaque: true,
      shouldShowWindow: true,
    };
  }

  if (!state.axGranted) {
    return {
      view: "ax",
      window: { w: AX_W, h: AX_H },
      nativeOpaque: true,
      shouldShowWindow: true,
    };
  }

  if (state.showHealthPopover) {
    return {
      view: "health",
      window: { w: HEALTH_W, h: HEALTH_H },
      nativeOpaque: true,
      shouldShowWindow: true,
    };
  }

  const hasRetry = state.appState === "error" && Boolean(state.retryableSessionId);
  const hudW =
    hasRetry || state.injectionFailed || state.healthCheckFailed
      ? HUD_RETRY_W
      : Boolean(state.transcriptionSuccessFlash)
        ? HUD_ALERT_W
        : HUD_W;
  const hudH =
    hasRetry || state.injectionFailed || state.healthCheckFailed
      ? HUD_RETRY_H
      : Boolean(state.transcriptionSuccessFlash)
        ? HUD_ALERT_H
        : HUD_H;

  return {
    view: "hud",
    window: {
      w: hudW,
      h: hudH,
      posKey: HUD_POS_KEY,
    },
    nativeOpaque: false,
    shouldShowWindow:
      state.appState !== "idle" ||
      state.injectionFailed ||
      state.polishFailed ||
      Boolean(state.transcriptionSuccessFlash) ||
      Boolean(state.healthCheckFailed) ||
      hasRetry ||
      Boolean(state.showIdleHud),
  };
}
