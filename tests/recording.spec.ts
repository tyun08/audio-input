import { test, expect } from "@playwright/test";
import { loadApp } from "./helpers.js";

test.describe("Recording HUD", () => {
  test("shows idle state on initial load", async ({ page }) => {
    await loadApp(page);
    const hud = page.locator(".hud");
    await expect(hud).toBeVisible();
    // In idle state the HUD is empty — no recording dot, no spinner
    await expect(hud.locator(".dot")).not.toBeVisible();
    await expect(hud.locator(".spinner")).not.toBeVisible();
  });

  test("shows recording state with timer", async ({ page }) => {
    await loadApp(page);

    // Simulate backend emitting state-change → recording
    await page.evaluate(() => {
      (window as any).__tauriEmit("state-change", "recording");
    });

    await expect(page.locator(".hud.recording")).toBeVisible();
    await expect(page.locator(".dot")).toBeVisible();
    // Timer label starts at 0:00
    await expect(page.locator(".label.red")).toHaveText("0:00");
  });

  test("shows processing state with spinner", async ({ page }) => {
    await loadApp(page);

    await page.evaluate(() => {
      (window as any).__tauriEmit("state-change", "processing");
    });

    await expect(page.locator(".hud.processing")).toBeVisible();
    await expect(page.locator(".spinner")).toBeVisible();
    await expect(page.locator(".label.blue")).toHaveText("Transcribing…");
  });

  test("shows error state with message", async ({ page }) => {
    await loadApp(page);

    await page.evaluate(() => {
      (window as any).__tauriEmit("state-change", "error:API key missing");
    });

    await expect(page.locator(".hud.error")).toBeVisible();
    await expect(page.locator(".err-dot")).toBeVisible();
    await expect(page.locator(".label.red")).toHaveText("API key missing");
  });

  test("recovers from error back to idle on re-trigger", async ({ page }) => {
    await loadApp(page);

    // Go to error state
    await page.evaluate(() => {
      (window as any).__tauriEmit("state-change", "error:some error");
    });
    await expect(page.locator(".hud.error")).toBeVisible();

    // Toggle again clears error → idle
    await page.evaluate(() => {
      (window as any).__tauriEmit("state-change", "idle");
    });

    await expect(page.locator(".hud.error")).not.toBeVisible();
    await expect(page.locator(".hud")).toBeVisible();
  });

  test("shows injection-failed state with clipboard icon", async ({
    page,
  }) => {
    await loadApp(page);

    // First emit transcription result so lastTranscription is set
    await page.evaluate(() => {
      (window as any).__tauriEmit("transcription-result", "hello world");
    });

    // Then emit injection-failed with the same text
    await page.evaluate(() => {
      (window as any).__tauriEmit("injection-failed", "hello world");
    });

    await expect(page.locator(".clip-icon")).toBeVisible();
    await expect(page.locator(".label.amber")).toHaveText(
      "Copied — ⌘V to paste"
    );
  });

  test("shows polish-failed state", async ({ page }) => {
    await loadApp(page);

    await page.evaluate(() => {
      (window as any).__tauriEmit("polish-failed", null);
    });

    await expect(page.locator(".label.amber")).toHaveText(
      "Polish failed — original used"
    );
  });

  test("transitions from recording to processing", async ({ page }) => {
    await loadApp(page);

    await page.evaluate(() => {
      (window as any).__tauriEmit("state-change", "recording");
    });
    await expect(page.locator(".hud.recording")).toBeVisible();

    await page.evaluate(() => {
      (window as any).__tauriEmit("state-change", "processing");
    });
    await expect(page.locator(".hud.processing")).toBeVisible();
    await expect(page.locator(".hud.recording")).not.toBeVisible();
  });
});
