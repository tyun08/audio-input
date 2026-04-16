export const HUD_W = 200;
export const HUD_H = 44;
export const HUD_ALERT_H = 72;
export const SETTINGS_W = 480;
export const SETTINGS_H = 480;
export const ONBOARDING_W = 370;
export const ONBOARDING_H = 540;
export const AX_W = 320;
export const AX_H = 160;

export const HUD_POS_KEY = "hud-window-pos";
export const SETTINGS_POS_KEY = "settings-window-pos";

export type AppState = "idle" | "recording" | "processing" | "error";
export type UiView = "onboarding" | "ax" | "settings" | "hud";

export interface UiModelState {
  onboardingDone: boolean;
  axGranted: boolean;
  showSettings: boolean;
  appState: AppState;
  injectionFailed: boolean;
  polishFailed: boolean;
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

  if (!state.axGranted) {
    return {
      view: "ax",
      window: { w: AX_W, h: AX_H },
      nativeOpaque: true,
      shouldShowWindow: true,
    };
  }

  if (state.showSettings) {
    return {
      view: "settings",
      window: { w: SETTINGS_W, h: SETTINGS_H, posKey: SETTINGS_POS_KEY },
      nativeOpaque: true,
      shouldShowWindow: true,
    };
  }

  return {
    view: "hud",
    window: {
      w: HUD_W,
      h: state.injectionFailed ? HUD_ALERT_H : HUD_H,
      posKey: HUD_POS_KEY,
    },
    nativeOpaque: false,
    shouldShowWindow:
      state.appState !== "idle" || state.injectionFailed || state.polishFailed,
  };
}
