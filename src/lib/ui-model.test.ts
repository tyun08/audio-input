import { describe, it, expect } from "vitest";
import {
  applyAppStateChange,
  deriveUiDecision,
  isHealthy,
  type HealthStatus,
  type UiModelState,
} from "./ui-model.js";

function baseState(overrides: Partial<UiModelState> = {}): UiModelState {
  return {
    onboardingDone: true,
    micGranted: true,
    axGranted: true,
    showSettings: false,
    appState: "idle",
    injectionFailed: false,
    polishFailed: false,
    transcriptionSuccessFlash: false,
    retryableSessionId: null,
    ...overrides,
  };
}

function baseHealth(overrides: Partial<HealthStatus> = {}): HealthStatus {
  return {
    micOk: true,
    micFound: true,
    axOk: true,
    apiOk: true,
    ...overrides,
  };
}

describe("applyAppStateChange", () => {
  it("recording closes settings and updates app state", () => {
    const result = applyAppStateChange(baseState({ showSettings: true }), "recording");
    expect(result.state.showSettings).toBe(false);
    expect(result.state.appState).toBe("recording");
  });

  it("processing closes settings and updates app state", () => {
    const result = applyAppStateChange(baseState({ showSettings: true }), "processing");
    expect(result.state.showSettings).toBe(false);
    expect(result.state.appState).toBe("processing");
  });

  it("idle does not close settings", () => {
    const result = applyAppStateChange(baseState({ showSettings: true }), "idle");
    expect(result.state.showSettings).toBe(true);
    expect(result.state.appState).toBe("idle");
  });

  it("error: prefix is parsed into appState=error with errorMsg", () => {
    const result = applyAppStateChange(baseState(), "error:mic unavailable");
    expect(result.state.appState).toBe("error");
    expect(result.errorMsg).toBe("mic unavailable");
  });
});

describe("deriveUiDecision", () => {
  it("onboarding takes priority over all other views", () => {
    const decision = deriveUiDecision(
      baseState({ onboardingDone: false, showSettings: true, appState: "processing" })
    );
    expect(decision).toEqual({
      view: "onboarding",
      window: { w: 500, h: 560 },
      nativeOpaque: true,
      shouldShowWindow: true,
    });
  });

  it("accessibility banner overrides HUD during active app state", () => {
    const decision = deriveUiDecision(baseState({ axGranted: false, appState: "recording" }));
    expect(decision).toEqual({
      view: "ax",
      window: { w: 320, h: 160 },
      nativeOpaque: true,
      shouldShowWindow: true,
    });
  });

  it("settings view is opaque and uses saved position", () => {
    const decision = deriveUiDecision(baseState({ showSettings: true }));
    expect(decision).toEqual({
      view: "settings",
      window: { w: 620, h: 560, posKey: "settings-window-pos" },
      nativeOpaque: true,
      shouldShowWindow: true,
    });
  });

  it("explicit Settings request wins over the mic permission nag", () => {
    const decision = deriveUiDecision(baseState({ showSettings: true, micGranted: false }));
    expect(decision.view).toBe("settings");
  });

  it("explicit Settings request wins over the accessibility nag", () => {
    const decision = deriveUiDecision(baseState({ showSettings: true, axGranted: false }));
    expect(decision.view).toBe("settings");
  });

  it("injection failure uses taller HUD and stays visible", () => {
    const decision = deriveUiDecision(baseState({ injectionFailed: true }));
    expect(decision.window).toEqual({ w: 300, h: 108, posKey: "hud-window-pos" });
    expect(decision.shouldShowWindow).toBe(true);
    expect(decision.nativeOpaque).toBe(false);
  });

  it("polish failure keeps HUD visible", () => {
    const decision = deriveUiDecision(baseState({ polishFailed: true }));
    expect(decision.view).toBe("hud");
    expect(decision.shouldShowWindow).toBe(true);
  });

  it("transcription success flash keeps HUD visible while idle", () => {
    const decision = deriveUiDecision(baseState({ transcriptionSuccessFlash: true }));
    expect(decision.view).toBe("hud");
    expect(decision.shouldShowWindow).toBe(true);
    expect(decision.window.w).toBe(300);
    expect(decision.window.h).toBe(108);
  });

  it("idle with no issues hides the HUD", () => {
    const decision = deriveUiDecision(baseState());
    expect(decision.view).toBe("hud");
    expect(decision.shouldShowWindow).toBe(false);
    expect(decision.nativeOpaque).toBe(false);
  });

  it("idle with showIdleHud=true keeps the HUD visible", () => {
    const decision = deriveUiDecision(baseState({ showIdleHud: true }));
    expect(decision.view).toBe("hud");
    expect(decision.shouldShowWindow).toBe(true);
    expect(decision.nativeOpaque).toBe(false);
    expect(decision.window.h).toBe(44);
  });

  it("recording keeps HUD visible at standard size", () => {
    const decision = deriveUiDecision(baseState({ appState: "recording" }));
    expect(decision.view).toBe("hud");
    expect(decision.window.h).toBe(44);
    expect(decision.shouldShowWindow).toBe(true);
  });

  it("retryable transcription error shows the retry HUD", () => {
    const decision = deriveUiDecision(
      baseState({ appState: "error", retryableSessionId: "rec_123" })
    );
    expect(decision.view).toBe("hud");
    expect(decision.shouldShowWindow).toBe(true);
    expect(decision.window.w).toBe(300);
    expect(decision.window.h).toBe(108);
  });

  it("error without a retryable session uses the small HUD", () => {
    const decision = deriveUiDecision(baseState({ appState: "error" }));
    expect(decision.view).toBe("hud");
    expect(decision.window.w).toBe(200);
    expect(decision.window.h).toBe(44);
  });

  it("health popover overrides settings and HUD, but not onboarding/mic/ax", () => {
    const decision = deriveUiDecision(
      baseState({ showHealthPopover: true, showSettings: true, appState: "recording" })
    );
    expect(decision).toEqual({
      view: "health",
      window: { w: 320, h: 260 },
      nativeOpaque: true,
      shouldShowWindow: true,
    });
  });

  it("missing mic permission still takes priority over the health popover", () => {
    const decision = deriveUiDecision(baseState({ micGranted: false, showHealthPopover: true }));
    expect(decision.view).toBe("mic");
  });

  it("missing accessibility permission still takes priority over the health popover", () => {
    const decision = deriveUiDecision(baseState({ axGranted: false, showHealthPopover: true }));
    expect(decision.view).toBe("ax");
  });

  it("health check failure keeps the retry-sized HUD visible without a retryable session", () => {
    const decision = deriveUiDecision(baseState({ healthCheckFailed: true }));
    expect(decision.view).toBe("hud");
    expect(decision.shouldShowWindow).toBe(true);
    expect(decision.window).toEqual({ w: 300, h: 108, posKey: "hud-window-pos" });
  });
});

describe("isHealthy", () => {
  it("is true when mic, mic device, accessibility, and API are all ok", () => {
    expect(isHealthy(baseHealth())).toBe(true);
  });

  it("is false when the microphone permission is missing", () => {
    expect(isHealthy(baseHealth({ micOk: false }))).toBe(false);
  });

  it("is false when no microphone device is found", () => {
    expect(isHealthy(baseHealth({ micFound: false }))).toBe(false);
  });

  it("is false when accessibility permission is missing", () => {
    expect(isHealthy(baseHealth({ axOk: false }))).toBe(false);
  });

  it("is false when the transcription API isn't configured/working", () => {
    expect(isHealthy(baseHealth({ apiOk: false }))).toBe(false);
  });
});
