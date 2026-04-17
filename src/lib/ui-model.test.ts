import { describe, it, expect } from "vitest";
import { applyAppStateChange, deriveUiDecision, type UiModelState } from "./ui-model.js";

function baseState(overrides: Partial<UiModelState> = {}): UiModelState {
  return {
    onboardingDone: true,
    axGranted: true,
    showSettings: false,
    appState: "idle",
    injectionFailed: false,
    polishFailed: false,
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
      window: { w: 370, h: 540 },
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
      window: { w: 620, h: 480, posKey: "settings-window-pos" },
      nativeOpaque: true,
      shouldShowWindow: true,
    });
  });

  it("injection failure uses taller HUD and stays visible", () => {
    const decision = deriveUiDecision(baseState({ injectionFailed: true }));
    expect(decision.window).toEqual({ w: 200, h: 72, posKey: "hud-window-pos" });
    expect(decision.shouldShowWindow).toBe(true);
    expect(decision.nativeOpaque).toBe(false);
  });

  it("polish failure keeps HUD visible", () => {
    const decision = deriveUiDecision(baseState({ polishFailed: true }));
    expect(decision.view).toBe("hud");
    expect(decision.shouldShowWindow).toBe(true);
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
});
