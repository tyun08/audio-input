/**
 * Shared helpers for Playwright integration tests.
 *
 * The app is designed to run inside a native Tauri window whose size is managed
 * by the Rust backend (via `appWindow.setSize`). In tests the window API is
 * mocked so resizes are no-ops; the browser viewport keeps its default size.
 *
 * Because `.container` uses `height: 100%` – which collapses to 0 when the
 * Svelte `#app` mount point has no explicit height – we inject a small CSS
 * override that pins the container to the full viewport height so that
 * absolutely-positioned panels (SettingsPanel) have a non-zero bounding box
 * and Playwright's visibility checks pass.
 */

import { type Page } from "@playwright/test";
import path from "path";
import { fileURLToPath } from "url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

const MOCK_SCRIPT = path.join(__dirname, "mock-tauri.js");

/** Inject the Tauri mock and viewport CSS, then navigate to the app root. */
export async function loadApp(page: Page): Promise<void> {
  // Inject Tauri IPC mock before any page scripts run
  await page.addInitScript({ path: MOCK_SCRIPT });

  // Inject CSS that ensures the app container fills the full viewport even
  // when no native window-resize IPC call has been made.
  await page.addInitScript(() => {
    const style = document.createElement("style");
    style.textContent = `
      html, body, #app, .container { height: 100vh !important; min-height: 100vh !important; }
    `;
    document.addEventListener("DOMContentLoaded", () => document.head.appendChild(style));
  });

  await page.goto("/");

  // Wait until the Svelte app has mounted and onMount has finished setting up
  // Tauri event listeners (show-settings etc.).
  await page.waitForSelector(".hud", { timeout: 5000 });
  // Allow async onMount tasks (invoke calls, listen registrations) to complete.
  await page.waitForTimeout(200);
}

/** Trigger the "show-settings" backend event and wait for the panel. */
export async function openSettings(page: Page): Promise<void> {
  await page.evaluate(() => {
    (window as any).__tauriEmit("show-settings", null);
  });
  await page.waitForSelector(".settings-root", {
    state: "visible",
    timeout: 3000,
  });
}
