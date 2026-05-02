//! macOS-specific global shortcut via CGEventTap at the HID level.
//!
//! The Tauri global-shortcut plugin uses Carbon `RegisterEventHotKey`, which sits
//! *after* CGEventTap in the macOS event chain. Apps like 1Password that register
//! a CGEventTap at the session level consume matching key events before Carbon
//! hotkeys ever fire.
//!
//! By installing our own tap at `kCGHIDEventTap` (the earliest interception point),
//! we see the event first and can consume it — preventing other apps from stealing
//! the shortcut.
//!
//! Requires Accessibility permission (the app already requests this).

use anyhow::{bail, Result};
use std::ffi::c_void;
use std::sync::{Arc, Mutex};
use tracing::info;

// ── CoreGraphics FFI ────────────────────────────────────────────────────────

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventTapCreate(
        tap: u32,
        place: u32,
        options: u32,
        events_of_interest: u64,
        callback: unsafe extern "C" fn(
            proxy: *mut c_void,
            event_type: u32,
            event: *mut c_void,
            user_info: *mut c_void,
        ) -> *mut c_void,
        user_info: *mut c_void,
    ) -> *mut c_void;
    fn CGEventTapEnable(tap: *mut c_void, enable: bool);
    fn CGEventGetIntegerValueField(event: *mut c_void, field: u32) -> i64;
    fn CGEventGetFlags(event: *mut c_void) -> u64;
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFMachPortCreateRunLoopSource(
        allocator: *const c_void,
        port: *mut c_void,
        order: i64,
    ) -> *mut c_void;
    fn CFRunLoopGetCurrent() -> *mut c_void;
    fn CFRunLoopAddSource(rl: *mut c_void, source: *mut c_void, mode: *const c_void);
    fn CFRunLoopRun();
    static kCFRunLoopCommonModes: *const c_void;
}

// ── Constants ───────────────────────────────────────────────────────────────

// HID level = 0 sits earlier in the macOS event chain than session level (= 1).
// Session-level taps (e.g. 1Password) cannot steal an HID-level match.
const KCG_HID_EVENT_TAP: u32 = 0;
const KCG_HEAD_INSERT_EVENT_TAP: u32 = 0;
const KCG_EVENT_TAP_OPTION_DEFAULT: u32 = 0;

const KCG_EVENT_KEY_DOWN: u32 = 10;
const KCG_EVENT_KEY_UP: u32 = 11;
const KCG_EVENT_TAP_DISABLED_BY_TIMEOUT: u32 = 0xFFFF_FFFE;
const KCG_EVENT_TAP_DISABLED_BY_USER_INPUT: u32 = 0xFFFF_FFFF;

const KCG_KEYBOARD_EVENT_KEYCODE: u32 = 9;

/// Mask covering only the four modifier keys we care about.
const MODIFIER_MASK: u64 = (1 << 20) | (1 << 19) | (1 << 18) | (1 << 17);
// Cmd=20, Alt=19, Ctrl=18, Shift=17

// ── Internal types ──────────────────────────────────────────────────────────

struct ShortcutTarget {
    keycode: i64,
    modifiers: u64,
}

struct TapContext {
    target: Arc<Mutex<ShortcutTarget>>,
    sender: std::sync::mpsc::Sender<()>,
    tap_ref: *mut c_void,
}

unsafe impl Send for TapContext {}
unsafe impl Sync for TapContext {}

// ── Public handle ───────────────────────────────────────────────────────────

/// Stored in Tauri state; allows updating the shortcut at runtime.
pub struct ShortcutHandle {
    target: Arc<Mutex<ShortcutTarget>>,
}

impl ShortcutHandle {
    /// Update the target shortcut without reinstalling the tap.
    pub fn update(&self, shortcut_str: &str) -> Result<()> {
        let (keycode, modifiers) = parse_to_cg(shortcut_str)?;
        let mut t = self.target.lock().unwrap();
        t.keycode = keycode;
        t.modifiers = modifiers;
        info!("CGEventTap shortcut updated: {}", shortcut_str);
        Ok(())
    }
}

// ── CGEventTap callback ────────────────────────────────────────────────────

unsafe extern "C" fn tap_callback(
    _proxy: *mut c_void,
    event_type: u32,
    event: *mut c_void,
    user_info: *mut c_void,
) -> *mut c_void {
    if user_info.is_null() {
        return event;
    }

    let ctx = &*(user_info as *const TapContext);

    // macOS can disable the tap (slow callback, user input, etc.); re-enable
    // automatically so the shortcut keeps working across long sessions.
    if event_type == KCG_EVENT_TAP_DISABLED_BY_TIMEOUT
        || event_type == KCG_EVENT_TAP_DISABLED_BY_USER_INPUT
    {
        CGEventTapEnable(ctx.tap_ref, true);
        return event;
    }

    if event.is_null() || (event_type != KCG_EVENT_KEY_DOWN && event_type != KCG_EVENT_KEY_UP) {
        return event;
    }

    let target = match ctx.target.lock() {
        Ok(t) => t,
        Err(_) => return event,
    };

    let keycode = CGEventGetIntegerValueField(event, KCG_KEYBOARD_EVENT_KEYCODE);
    let flags = CGEventGetFlags(event) & MODIFIER_MASK;

    if keycode == target.keycode && flags == target.modifiers {
        drop(target);
        if event_type == KCG_EVENT_KEY_DOWN {
            let _ = ctx.sender.send(());
        }
        // Return null to consume both keyDown and keyUp — other apps never see them.
        return std::ptr::null_mut();
    }

    event
}

// ── Shortcut string → macOS virtual keycode + CG modifier flags ────────────

fn parse_to_cg(shortcut_str: &str) -> Result<(i64, u64)> {
    let mut mods: u64 = 0;
    let mut keycode: Option<i64> = None;

    for part in shortcut_str.split('+') {
        match part.trim() {
            "Meta" | "Super" | "Cmd" | "Command" => mods |= 1 << 20,
            "Ctrl" | "Control" => mods |= 1 << 18,
            "Alt" | "Option" => mods |= 1 << 19,
            "Shift" => mods |= 1 << 17,
            key => {
                keycode = Some(match key {
                    "Space" => 0x31,
                    "A" => 0x00,
                    "B" => 0x0B,
                    "C" => 0x08,
                    "D" => 0x02,
                    "E" => 0x0E,
                    "F" => 0x03,
                    "G" => 0x05,
                    "H" => 0x04,
                    "I" => 0x22,
                    "J" => 0x26,
                    "K" => 0x28,
                    "L" => 0x25,
                    "M" => 0x2E,
                    "N" => 0x2D,
                    "O" => 0x1F,
                    "P" => 0x23,
                    "Q" => 0x0C,
                    "R" => 0x0F,
                    "S" => 0x01,
                    "T" => 0x11,
                    "U" => 0x20,
                    "V" => 0x09,
                    "W" => 0x0D,
                    "X" => 0x07,
                    "Y" => 0x10,
                    "Z" => 0x06,
                    "0" => 0x1D,
                    "1" => 0x12,
                    "2" => 0x13,
                    "3" => 0x14,
                    "4" => 0x15,
                    "5" => 0x17,
                    "6" => 0x16,
                    "7" => 0x1A,
                    "8" => 0x1C,
                    "9" => 0x19,
                    "F1" => 0x7A,
                    "F2" => 0x78,
                    "F3" => 0x63,
                    "F4" => 0x76,
                    "F5" => 0x60,
                    "F6" => 0x61,
                    "F7" => 0x62,
                    "F8" => 0x64,
                    "F9" => 0x65,
                    "F10" => 0x6D,
                    "F11" => 0x67,
                    "F12" => 0x6F,
                    other => bail!("Unknown key: {}", other),
                });
            }
        }
    }

    let code = keycode.ok_or_else(|| anyhow::anyhow!("Shortcut missing main key"))?;
    Ok((code, mods))
}

// ── Public install ──────────────────────────────────────────────────────────

/// Install a CGEventTap at the HID level to intercept the shortcut before any
/// other app. `on_trigger` is called on each keyDown match from a background
/// thread. Returns a [`ShortcutHandle`] for runtime updates.
pub fn install<F>(shortcut_str: &str, on_trigger: F) -> Result<ShortcutHandle>
where
    F: Fn() + Send + 'static,
{
    let (keycode, modifiers) = parse_to_cg(shortcut_str)?;

    let target = Arc::new(Mutex::new(ShortcutTarget { keycode, modifiers }));
    let (sender, receiver) = std::sync::mpsc::channel::<()>();

    let handle = ShortcutHandle {
        target: Arc::clone(&target),
    };

    let ctx = Box::new(TapContext {
        target,
        sender,
        tap_ref: std::ptr::null_mut(),
    });
    let ctx_ptr = Box::into_raw(ctx);

    let event_mask: u64 = (1u64 << KCG_EVENT_KEY_DOWN) | (1u64 << KCG_EVENT_KEY_UP);

    unsafe {
        let tap = CGEventTapCreate(
            KCG_HID_EVENT_TAP,
            KCG_HEAD_INSERT_EVENT_TAP,
            KCG_EVENT_TAP_OPTION_DEFAULT,
            event_mask,
            tap_callback,
            ctx_ptr as *mut c_void,
        );

        if tap.is_null() {
            drop(Box::from_raw(ctx_ptr));
            bail!("CGEventTapCreate failed — Accessibility permission may not be granted");
        }

        (*ctx_ptr).tap_ref = tap;

        let source = CFMachPortCreateRunLoopSource(std::ptr::null(), tap, 0);
        if source.is_null() {
            drop(Box::from_raw(ctx_ptr));
            bail!("CFMachPortCreateRunLoopSource failed");
        }

        // Cast to usize so the closure is Send (raw pointers aren't Send).
        // Safety: these CF objects are thread-safe; we only use them on the new thread.
        let tap_addr = tap as usize;
        let source_addr = source as usize;
        let modes_addr = kCFRunLoopCommonModes as usize;
        std::thread::spawn(move || {
            let tap = tap_addr as *mut c_void;
            let source = source_addr as *mut c_void;
            let modes = modes_addr as *const c_void;
            let rl = CFRunLoopGetCurrent();
            CFRunLoopAddSource(rl, source, modes);
            CGEventTapEnable(tap, true);
            info!("CGEventTap run loop started (HID-level shortcut interception active)");
            CFRunLoopRun();
        });
    }

    std::thread::spawn(move || {
        while receiver.recv().is_ok() {
            on_trigger();
        }
    });

    Ok(handle)
}
