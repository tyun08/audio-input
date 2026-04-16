/**
 * Minimal structured logger that can be silenced in production by setting
 * the VITE_LOG_LEVEL environment variable to "none" at build time.
 *
 * Usage:
 *   import { log } from "./logger";
 *   log("[syncWindow] view=%s opaque=%s", view, opaque);
 */

const enabled = import.meta.env.VITE_LOG_LEVEL !== "none";

export function log(...args: unknown[]): void {
  if (enabled) {
    console.log(...args);
  }
}
