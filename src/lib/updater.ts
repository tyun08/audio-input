import { invoke } from "@tauri-apps/api/core";
import { ask, message } from "@tauri-apps/plugin-dialog";
import { relaunch } from "@tauri-apps/plugin-process";

type AvailableUpdate = {
  version: string;
  body?: string;
};

let inFlight = false;

/**
 * Check for an update from the user's persisted stable/beta channel.
 *
 * @param silent  When true (startup auto-check), only shows UI if an update
 *                is found. When false (user clicked "Check for Updates…"),
 *                also shows a confirmation when the app is already up to date.
 */
export async function checkForUpdates(silent: boolean): Promise<void> {
  if (inFlight) return;
  inFlight = true;
  try {
    const update = await invoke<AvailableUpdate | null>("check_for_update");
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

    // The native updater re-checks the selected channel, verifies that the
    // offered version has not changed, then verifies its signature and
    // installs it. Relaunch from the frontend after the IPC command returns;
    // restarting inside that command can terminate the app before Tauri has
    // completed the relaunch handoff on macOS.
    await invoke("install_update", { version: update.version });
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
