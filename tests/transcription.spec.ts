import { test, expect } from "@playwright/test";
import { loadApp } from "./helpers.js";

test.describe("Transcription", () => {
  test("displays transcription result after successful transcription", async ({
    page,
  }) => {
    await loadApp(page);

    // Simulate processing started then result received
    await page.evaluate(() => {
      (window as any).__tauriEmit("state-change", "processing");
    });
    await expect(page.locator(".hud.processing")).toBeVisible();

    // Backend emits transcription result
    await page.evaluate(() => {
      (window as any).__tauriEmit(
        "transcription-result",
        "Hello from transcription"
      );
    });

    // State returns to idle after successful transcription
    await page.evaluate(() => {
      (window as any).__tauriEmit("state-change", "idle");
    });

    // HUD should be in idle state (not error, not processing)
    await expect(page.locator(".hud.processing")).not.toBeVisible();
    await expect(page.locator(".hud.error")).not.toBeVisible();
  });

  test("shows injection-failed when text cannot be typed into app", async ({
    page,
  }) => {
    await loadApp(page);

    // Simulate the flow: transcription succeeds but injection fails
    await page.evaluate(() => {
      (window as any).__tauriEmit(
        "transcription-result",
        "Transcribed text here"
      );
      (window as any).__tauriEmit("injection-failed", "Transcribed text here");
    });

    // Should show the clipboard fallback panel
    await expect(page.locator(".hud.injection")).toBeVisible();
    await expect(page.locator(".injection-title")).toHaveText(
      "Copied to Clipboard"
    );
    await expect(page.locator(".injection-msg")).toContainText(
      "Text copied to clipboard"
    );
    await expect(
      page.getByRole("button", { name: "Copy Again" })
    ).toBeVisible();
  });

  test("shows polish-failed notification and it auto-clears", async ({
    page,
  }) => {
    await loadApp(page);

    await page.evaluate(() => {
      (window as any).__tauriEmit("polish-failed", null);
    });

    await expect(page.locator(".label.amber")).toHaveText(
      "Polish failed — original used"
    );

    // The polish-failed state auto-clears after 3 seconds
    // We wait up to 4 seconds for it to disappear
    await expect(page.locator(".label.amber")).not.toBeVisible({
      timeout: 4500,
    });
  });

  test("copy manually dismisses the success HUD immediately", async ({
    page,
  }) => {
    await loadApp(page);

    await page.evaluate(() => {
      (window as any).__tauriEmit("transcription-result", "Copied text");
      (window as any).__tauriEmit("state-change", "idle");
      (window as any).__tauriEmit("transcription-success", null);
    });

    const copyButton = page.getByRole("button", { name: "Copy Manually" });
    await expect(copyButton).toBeVisible();

    await copyButton.click();

    await expect(copyButton).not.toBeVisible();
    await expect(page.locator(".success-head")).not.toBeVisible();
  });

  test("success HUD lets users edit before manual copy", async ({ page }) => {
    await page.addInitScript(() => {
      (window as any).__lastClipboardText = "";
      Object.defineProperty(navigator, "clipboard", {
        configurable: true,
        value: {
          writeText: async (text: string) => {
            (window as any).__lastClipboardText = text;
          },
        },
      });
    });
    await loadApp(page);

    await page.evaluate(() => {
      (window as any).__tauriEmit("transcription-result", "originnal transcript");
      (window as any).__tauriEmit("state-change", "idle");
      (window as any).__tauriEmit("transcription-success", null);
    });

    const editor = page.getByLabel("Editable transcription");
    await expect(editor).toHaveValue("originnal transcript");

    await editor.fill("original transcript");
    await page.getByRole("button", { name: "Copy Manually" }).click();

    await expect
      .poll(() => page.evaluate(() => (window as any).__lastClipboardText))
      .toBe("original transcript");
    await expect(editor).not.toBeVisible();
  });

  test("focusing success transcript keeps the HUD open past the default timeout", async ({
    page,
  }) => {
    await loadApp(page);

    await page.evaluate(() => {
      (window as any).__tauriEmit("transcription-result", "Keep this visible");
      (window as any).__tauriEmit("state-change", "idle");
      (window as any).__tauriEmit("transcription-success", null);
    });

    const editor = page.getByLabel("Editable transcription");
    await editor.focus();

    await page.waitForTimeout(5500);
    await expect(editor).toBeVisible();
    await expect(editor).toHaveValue("Keep this visible");
  });

  test("typing in the success transcript keeps focus in the editor", async ({
    page,
  }) => {
    await loadApp(page);

    await page.evaluate(() => {
      (window as any).__tauriEmit("transcription-result", "Edit");
      (window as any).__tauriEmit("state-change", "idle");
      (window as any).__tauriEmit("transcription-success", null);
    });

    const editor = page.getByLabel("Editable transcription");
    await editor.click();
    await page.keyboard.type(" this text smoothly");

    await expect(editor).toBeFocused();
    await expect(editor).toHaveValue("Edit this text smoothly");
  });

  test("shows API key missing error and opens settings", async ({ page }) => {
    await loadApp(page);

    // Backend emits api-key-missing, which should open settings
    await page.evaluate(() => {
      (window as any).__tauriEmit("api-key-missing", null);
    });

    await expect(page.locator(".settings-root")).toBeVisible({ timeout: 3000 });
  });

  test("handles full recording → transcription cycle", async ({ page }) => {
    await loadApp(page);

    // 1. Start recording
    await page.evaluate(() => {
      (window as any).__tauriEmit("state-change", "recording");
    });
    await expect(page.locator(".hud.recording")).toBeVisible();

    // 2. Stop → processing
    await page.evaluate(() => {
      (window as any).__tauriEmit("state-change", "processing");
    });
    await expect(page.locator(".hud.processing")).toBeVisible();
    await expect(page.locator(".spinner")).toBeVisible();

    // 3. Result arrives
    await page.evaluate(() => {
      (window as any).__tauriEmit("transcription-result", "The quick brown fox");
    });

    // 4. State returns to idle
    await page.evaluate(() => {
      (window as any).__tauriEmit("state-change", "idle");
    });

    // Final: processing indicator gone, app is back to idle
    await expect(page.locator(".spinner")).not.toBeVisible();
    await expect(page.locator(".hud.processing")).not.toBeVisible();
  });
});
