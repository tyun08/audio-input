import { check, type Update } from "@tauri-apps/plugin-updater";
import { ask, message } from "@tauri-apps/plugin-dialog";
import { relaunch } from "@tauri-apps/plugin-process";

let inFlight = false;

/**
 * Check for an update from the configured endpoint.
 *
 * @param silent  When true (startup auto-check), only shows UI if an update
 *                is found. When false (user clicked "Check for Updates…"),
 *                also shows a confirmation when the app is already up to date.
 */
export async function checkForUpdates(silent: boolean): Promise<void> {
  if (inFlight) return;
  inFlight = true;
  try {
    const update: Update | null = await check();
    if (!update) {
      if (!silent) {
        await message("You're on the latest version of Audio Input.", {
          title: "Up to date",
          kind: "info",
        });
      }
      return;
    }

    const confirmed = await ask(
      `Audio Input ${update.version} is available.\n\n${update.body ?? ""}\n\nInstall now? The app will restart automatically.`,
      {
        title: "Update available",
        kind: "info",
        okLabel: "Install & Restart",
        cancelLabel: "Later",
      }
    );
    if (!confirmed) return;

    // Download + install. tauri-plugin-updater handles signature verification
    // against the public key in tauri.conf.json — if the signature mismatches
    // (e.g., MITM), this throws and we surface the error to the user.
    await update.downloadAndInstall();
    // On Windows, downloadAndInstall already exits the process; on macOS we
    // explicitly relaunch.
    await relaunch();
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    // Suppress noisy "network unreachable" style errors on silent startup
    // checks; the user didn't ask, so don't pop a dialog they didn't trigger.
    if (silent) {
      console.warn("[updater] silent check failed:", msg);
      return;
    }
    await message(`Update check failed:\n\n${msg}`, {
      title: "Update error",
      kind: "error",
    });
  } finally {
    inFlight = false;
  }
}
