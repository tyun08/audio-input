pub mod injector;
pub use injector::inject_text;

// Phase 2 of docs/IMK_PLAN.md will wire `imk::client`/`imk::protocol`
// into `inject_text` and context-aware polishing. Until then, every
// item in the module is "dead code" from clippy's perspective even
// though the unit tests in client.rs / protocol.rs exercise them.
// Lift this allow when Phase 2 lands.
#[cfg(target_os = "macos")]
#[allow(dead_code)]
pub mod imk;
