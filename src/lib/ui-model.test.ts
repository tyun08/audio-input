import { applyAppStateChange, deriveUiDecision, type UiModelState } from "./ui-model.js";

function assert(condition: boolean, message: string): void {
  if (!condition) {
    throw new Error(message);
  }
}

function assertEqual<T>(actual: T, expected: T, message: string): void {
  if (JSON.stringify(actual) !== JSON.stringify(expected)) {
    throw new Error(`${message}\nExpected: ${JSON.stringify(expected)}\nActual: ${JSON.stringify(actual)}`);
  }
}

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

{
  const result = applyAppStateChange(baseState({ showSettings: true }), "recording");
  assert(result.state.showSettings === false, "recording should close settings");
  assert(result.state.appState === "recording", "recording should update app state");
}

{
  const decision = deriveUiDecision(baseState({ onboardingDone: false, showSettings: true, appState: "processing" }));
  assertEqual(
    decision,
    {
      view: "onboarding",
      window: { w: 370, h: 540 },
      nativeOpaque: true,
      shouldShowWindow: true,
    },
    "onboarding should take priority over all other views",
  );
}

{
  const decision = deriveUiDecision(baseState({ axGranted: false, appState: "recording" }));
  assertEqual(
    decision,
    {
      view: "ax",
      window: { w: 320, h: 160 },
      nativeOpaque: true,
      shouldShowWindow: true,
    },
    "accessibility banner should stay visible instead of shrinking to HUD",
  );
}

{
  const decision = deriveUiDecision(baseState({ injectionFailed: true }));
  assertEqual(
    decision.window,
    { w: 200, h: 72, posKey: "hud-window-pos" },
    "injection failure should use the taller HUD size",
  );
  assert(decision.shouldShowWindow === true, "injection failure should keep the HUD visible");
}
