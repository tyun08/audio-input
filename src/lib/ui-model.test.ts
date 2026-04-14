import { applyAppStateChange, deriveUiDecision, type UiModelState } from "./ui-model.js";

function testAssert(condition: boolean, message: string): void {
  if (!condition) {
    throw new Error(message);
  }
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function deepEqual(actual: unknown, expected: unknown): boolean {
  if (Object.is(actual, expected)) {
    return true;
  }

  if (Array.isArray(actual) && Array.isArray(expected)) {
    return actual.length === expected.length && actual.every((value, index) => deepEqual(value, expected[index]));
  }

  if (isObject(actual) && isObject(expected)) {
    const actualKeys = Object.keys(actual);
    const expectedKeys = Object.keys(expected);

    return (
      actualKeys.length === expectedKeys.length &&
      actualKeys.every((key) => key in expected && deepEqual(actual[key], expected[key]))
    );
  }

  return false;
}

function assertEqual<T>(actual: T, expected: T, message: string): void {
  if (!deepEqual(actual, expected)) {
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
  testAssert(result.state.showSettings === false, "recording should close settings");
  testAssert(result.state.appState === "recording", "recording should update app state");
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
  testAssert(decision.shouldShowWindow === true, "injection failure should keep the HUD visible");
}
