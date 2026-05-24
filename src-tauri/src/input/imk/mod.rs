//! InputMethodKit integration — Tauri-side IPC client.
//!
//! Connects to the `imk-helper` Swift app over a Unix domain socket and lets
//! the Rust app:
//!   - insert text at the current IME insertion point (replacing
//!     `clipboard + ⌘V`, which fails in sandboxed / Electron apps and
//!     can't read context)
//!   - query the cursor context (text before cursor, selected text,
//!     cursor rect, active app) — used for context-aware polishing and
//!     selected-text transforms
//!
//! This module is currently scaffolding only — no callers in
//! `inject_text` / `commands.rs` yet. Phase 2 of `docs/IMK_PLAN.md`
//! wires the swap; for now Phase 1 just builds + tests the protocol and
//! client surface so the Swift helper has a stable contract to target.

pub mod client;
pub mod protocol;

// Phase 2 will start calling these from `inject_text` / context-aware
// polish. Until then the re-exports are unused; allow the warning so
// the surface stays at the module root for downstream callers.
#[allow(unused_imports)]
pub use client::{ImkClient, ImkError};
#[allow(unused_imports)]
pub use protocol::{Range, Rect, Request, Response};

/// Filesystem path the IMK helper listens on. Hardcoded — both sides must
/// agree, and `/tmp` is writable by both a sandboxed input method (in
/// `~/Library/Input Methods`) and the main Tauri app.
pub const SOCKET_PATH: &str = "/tmp/audio-input-imk.sock";
