# WebKit Transparency and HUD Window Behavior

This document describes the known differences between **dev** and **release/production** for the transparent HUD window on macOS, and how the codebase guards against the most common failure modes.

---

## How the window works

The main window is created with:

```json
"transparent": true,
"visible": false,
"decorations": false
```

Two distinct rendering modes are toggled at runtime via `set_native_opaque`:

| Mode | `NSWindow.isOpaque` | `NSWindow.backgroundColor` | Used for |
|------|--------------------|-----------------------------|----------|
| HUD (transparent) | `false` | `clearColor` | Recording/processing indicator |
| Opaque panel | `true` | near-black `rgba(30,30,32,1)` | Onboarding, settings, accessibility banner |

Switching modes calls `[NSWindow invalidateShadow]` so macOS repaints the compositor layer immediately.

---

## Known dev vs release differences

### 1. Stale HMR styles in dev

`vite dev` hot-module reloads can leave inline `style` attributes on `<html>` and `<body>` from a previous cycle. These stale styles override `background: transparent`, causing the HUD to appear with a dark or colored background.

**Guard in code**: On each mount, `App.svelte` removes any stale inline styles:

```ts
document.documentElement.removeAttribute("style");
document.body.removeAttribute("style");
localStorage.removeItem("window-opacity");
```

This is not needed in production builds (no HMR) but is harmless to run there.

### 2. WebKit compositing order during opaque toggle

When transitioning from transparent to opaque (e.g., opening settings), the sequence matters:

```
setNativeOpaque(true)  →  show()  →  setSize()
```

The centralized `syncWindow()` path in `App.svelte` calls `setNativeOpaque` first, then delegates to `resizeTo()` (which calls `show()`).

**Important caveat — hidden windows:** The `set_native_opaque` Tauri command applies to all windows regardless of visibility. An earlier version only applied to visible windows (`isVisible` check), which caused a black settings window: when the HUD was hidden (idle state), `setNativeOpaque(true)` was a silent no-op, then `show()` rendered the window with the old transparent/clear background. The `isVisible` guard has been removed — `NSWindow.isOpaque` and `backgroundColor` are now set on hidden windows too, so the state is correct the moment the window becomes visible.

### 3. Rounded corners on the opaque panel

`set_native_opaque` in `src-tauri/src/commands.rs` also sets `cornerRadius` on the `NSVisualEffectView` frame layer:

- **opaque = true**: `cornerRadius = 16`, `masksToBounds = true` — the panel background is clipped to rounded corners.
- **opaque = false**: `cornerRadius = 0`, `masksToBounds = false` — the transparent HUD has no clipping (its own CSS border-radius is used instead).

This is a macOS-only operation; on Windows the code path is a no-op.

### 4. Release builds with ad-hoc signing

See [`macos-distribution-lessons.md`](./macos-distribution-lessons.md) for the TCC/Accessibility issue where linker-signed builds lose permissions between builds. That document covers the `signingIdentity: "-"` fix.

---

## Diagnosing "black window" incidents

When the settings panel (or any opaque panel) appears completely black:

1. **Check the syncWindow log** — open Console.app and filter on the process name. Look for `[syncWindow]` lines to confirm `opaque=true` is logged before the window shows. A log line like `opaque=true show=true` but a black window means `set_native_opaque` was not applied — check that the Rust command isn't silently failing.
2. **Check for stale inline styles** — in dev, open DevTools → Elements and confirm `<html>` and `<body>` have no `style` attribute with a background color.
3. **Confirm window ordering** — `setNativeOpaque` must be called before `show()` AND the Rust command must not skip the window (e.g., due to visibility filtering). The `syncWindow()` function keeps this ordering correct; any new code path that calls `show()` directly bypasses this guard.
4. **Check for early-return paths** — if `onMount` throws before reaching `syncWindow()`, the window stays in whatever state it was last set to. The initialization `try/catch` in `App.svelte` shows a visible error banner and calls `setNativeOpaque(true)` as a fallback so the window is at least readable.

---

## CI and manual testing scope

| Test type | What is covered | How to run |
|-----------|----------------|------------|
| Unit tests (Vitest) | `deriveUiDecision`, `applyAppStateChange` pure logic | `npm test` |
| Manual dev | Full transparent window rendering, HMR stale-style guard | `npm run tauri dev` |
| Manual release | Opaque panel rendering, TCC/Accessibility, rounded corners | `npm run tauri build` → install `.app` |

The full transparent window + `set_native_opaque` cycle cannot be tested in CI without a real macOS display server; those paths are covered by manual testing only.
