import { test, expect } from "@playwright/test";
import { loadApp, openSettings } from "./helpers.js";

test.describe("Settings Panel", () => {
  test("opens settings panel via show-settings event", async ({ page }) => {
    await loadApp(page);
    await openSettings(page);

    await expect(page.locator(".settings-root")).toBeVisible();
    await expect(page.locator(".titlebar-label")).toHaveText("Settings");
  });

  test("shows transcription section by default", async ({ page }) => {
    await loadApp(page);
    await openSettings(page);

    // Transcription section should be active (contains voice service selector)
    await expect(page.locator(".nav-item.active")).toContainText(
      "Transcription"
    );
    await expect(page.locator(".row-label").first()).toContainText(
      "Voice Service"
    );
  });

  test("navigates to general section", async ({ page }) => {
    await loadApp(page);
    await openSettings(page);

    // Click the General nav item
    await page.getByRole("button", { name: /General/i }).click();

    await expect(page.locator(".nav-item.active")).toContainText("General");
    // General section contains microphone and shortcut settings
    await expect(page.locator(".content")).toContainText("Microphone");
  });

  test("navigates to advanced section", async ({ page }) => {
    await loadApp(page);
    await openSettings(page);

    await page.getByRole("button", { name: /Advanced/i }).click();

    await expect(page.locator(".nav-item.active")).toContainText("Advanced");
    await expect(page.locator(".content")).toContainText("Screenshot Context");
  });

  test("displays Groq provider by default", async ({ page }) => {
    await loadApp(page);
    await openSettings(page);

    // Provider selector shows Groq (first .row-select; Groq also has a model select)
    const providerSelect = page.locator(".row-select").first();
    await expect(providerSelect).toHaveValue("groq");
  });

  test("switches to Vertex AI provider", async ({ page }) => {
    await loadApp(page);
    await openSettings(page);

    const providerSelect = page.locator(".row-select").first();
    await providerSelect.selectOption("vertex_ai");

    // Vertex AI config fields should appear (project ID)
    await expect(page.locator(".content")).toContainText("GCP Project ID");
  });

  test("shows OpenAI API key and model without API base", async ({ page }) => {
    await loadApp(page);
    await openSettings(page);

    const providerSelect = page.locator(".row-select").first();
    await providerSelect.selectOption("openai");

    await expect(page.locator(".content")).not.toContainText("API Base URL");
    await expect(page.locator('input[type="password"]')).toHaveAttribute("placeholder", "sk-...");
    await expect(page.locator(".dropdown")).toHaveValue("gpt-4o-mini-transcribe");
  });

  test("shows Gemini API key and model without API base", async ({ page }) => {
    await loadApp(page);
    await openSettings(page);

    const providerSelect = page.locator(".row-select").first();
    await providerSelect.selectOption("gemini");

    await expect(page.locator(".content")).not.toContainText("API Base URL");
    await expect(page.locator('input[type="password"]')).toHaveAttribute("placeholder", "AIza...");
    await expect(page.locator(".dropdown")).toHaveValue("gemini-2.5-flash");
  });

  test("keeps API base editable for LiteLLM", async ({ page }) => {
    await loadApp(page);
    await openSettings(page);

    const providerSelect = page.locator(".row-select").first();
    await providerSelect.selectOption("litellm");

    await expect(page.locator(".content")).toContainText("API Base URL");
    await expect(page.locator('input[placeholder="http://localhost:4000/v1"]')).toBeVisible();
    await expect(page.locator('input[placeholder="whisper-1"]')).toHaveValue("whisper-1");
  });

  test("shows API key field for Groq provider", async ({ page }) => {
    await loadApp(page);
    await openSettings(page);

    await expect(page.locator('input[type="password"]')).toBeVisible();
  });

  test("saves settings and shows saved confirmation", async ({ page }) => {
    await loadApp(page);
    await openSettings(page);

    // Type an API key
    const apiKeyInput = page.locator('input[type="password"]');
    await apiKeyInput.fill("gsk_test123");

    // Click Save
    await page.getByRole("button", { name: /Save/i }).click();

    // Should show "Saved" feedback
    await expect(
      page.getByRole("button", { name: /Saved|Saving/i })
    ).toBeVisible({ timeout: 3000 });
  });

  test("closes settings panel on close button", async ({ page }) => {
    await loadApp(page);
    await openSettings(page);

    await expect(page.locator(".settings-root")).toBeVisible();

    // Click close button (aria-label="Close")
    await page.getByRole("button", { name: "Close" }).click();

    await expect(page.locator(".settings-root")).not.toBeVisible();
    await expect(page.locator(".hud")).toBeVisible();
  });

  test("shows recording status pill when app is recording and settings opens", async ({
    page,
  }) => {
    // Pre-set app state to "recording" so the settings panel opens in that state.
    // (When a state-change event fires while settings is open, the panel closes;
    // the pill is visible when settings is already open with the recording state.)
    await page.addInitScript({ path: "./tests/mock-tauri.js" });
    await page.addInitScript(() => {
      const style = document.createElement("style");
      style.textContent =
        "html, body, #app, .container { height: 100vh !important; min-height: 100vh !important; }";
      document.addEventListener("DOMContentLoaded", () =>
        document.head.appendChild(style)
      );
      // Override app state to "recording" before the app loads
      (window as any).__tauriMockOverrides = { get_app_state: "recording" };
    });
    await page.goto("/");
    await page.waitForSelector(".hud", { timeout: 5000 });
    await page.waitForTimeout(200);

    // Open settings while already in recording state
    await page.evaluate(() => {
      (window as any).__tauriEmit("show-settings", null);
    });
    await page.waitForSelector(".settings-root", {
      state: "visible",
      timeout: 3000,
    });

    await expect(page.locator(".status-pill.recording")).toBeVisible();
    await expect(page.locator(".status-pill.recording")).toContainText(
      "Recording"
    );
  });

  test("toggles AI polish setting", async ({ page }) => {
    await loadApp(page);
    await openSettings(page);

    // Navigate to advanced section which has the polish toggle
    await page.getByRole("button", { name: /Advanced/i }).click();

    // The polish toggle is a custom button with aria-label="Toggle polish"
    const polishToggle = page.getByRole("button", {
      name: "Toggle polish",
    });
    await expect(polishToggle).toBeVisible();

    // Initial state should be "on" (polishEnabled = true from mock)
    await expect(polishToggle).toHaveClass(/on/);

    // Click to disable
    await polishToggle.click();
    await expect(polishToggle).not.toHaveClass(/on/);

    // Click to re-enable
    await polishToggle.click();
    await expect(polishToggle).toHaveClass(/on/);
  });

  test("configures editable success HUD width", async ({ page }) => {
    await loadApp(page);
    await openSettings(page);

    await page.getByRole("button", { name: /Advanced/i }).click();

    const widthSlider = page.getByLabel("Success HUD width");
    await expect(widthSlider).toBeVisible();
    await expect(widthSlider).toHaveValue("560");

    await widthSlider.fill("700");
    await widthSlider.dispatchEvent("change");

    await expect(widthSlider).toHaveValue("700");
    await expect(page.locator(".range-value")).toContainText("700px");
  });
});
