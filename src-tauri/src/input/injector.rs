use anyhow::{bail, Context, Result};
use std::process::Command;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{error, info};

pub async fn inject_text(text: &str) -> Result<()> {
    if text.is_empty() {
        return Ok(());
    }

    info!("inject_text: {} chars", text.chars().count());

    // Write to clipboard first (works as fallback on all platforms)
    write_clipboard(text)?;
    info!("clipboard write OK");

    sleep(Duration::from_millis(100)).await;

    if !check_accessibility_permission() {
        bail!("Accessibility permission not granted — text copied to clipboard, press ⌘V to paste manually.");
    }

    info!("simulating paste keypress");
    paste_via_keyevent()?;

    sleep(Duration::from_millis(200)).await;
    Ok(())
}

// --- Clipboard -----------------------------------------------------------------

fn write_clipboard(text: &str) -> Result<()> {
    let mut ctx = arboard::Clipboard::new().context("clipboard init failed")?;
    ctx.set_text(text).context("clipboard write failed")?;
    Ok(())
}

// --- Paste key simulation ------------------------------------------------------

#[cfg(target_os = "macos")]
fn paste_via_keyevent() -> Result<()> {
    use std::ptr;

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGEventCreateKeyboardEvent(
            source: *const std::ffi::c_void,
            virtual_key: u16,
            key_down: bool,
        ) -> *mut std::ffi::c_void;
        fn CGEventSetFlags(event: *mut std::ffi::c_void, flags: u64);
        fn CGEventPost(tap: i32, event: *mut std::ffi::c_void);
        fn CFRelease(cf: *const std::ffi::c_void);
    }

    const KCG_HID_EVENT_TAP: i32 = 0;
    const KVK_ANSI_V: u16 = 9;
    const KCG_EVENT_FLAG_MASK_COMMAND: u64 = 0x0010_0000;

    unsafe {
        let down = CGEventCreateKeyboardEvent(ptr::null(), KVK_ANSI_V, true);
        if down.is_null() {
            bail!("CGEventCreateKeyboardEvent returned null — check Accessibility permission");
        }
        CGEventSetFlags(down, KCG_EVENT_FLAG_MASK_COMMAND);
        CGEventPost(KCG_HID_EVENT_TAP, down);
        CFRelease(down);

        let up = CGEventCreateKeyboardEvent(ptr::null(), KVK_ANSI_V, false);
        if up.is_null() {
            error!("CGEventCreateKeyboardEvent(up) returned null");
        } else {
            CGEventSetFlags(up, KCG_EVENT_FLAG_MASK_COMMAND);
            CGEventPost(KCG_HID_EVENT_TAP, up);
            CFRelease(up);
        }
    }

    Ok(())
}

#[cfg(target_os = "windows")]
fn paste_via_keyevent() -> Result<()> {
    use enigo::{Direction, Enigo, Key, Keyboard, Settings};

    let mut enigo = Enigo::new(&Settings::default())
        .map_err(|e| anyhow::anyhow!("enigo init: {:?}", e))?;

    enigo
        .key(Key::Control, Direction::Press)
        .map_err(|e| anyhow::anyhow!("ctrl press: {:?}", e))?;
    enigo
        .key(Key::Unicode('v'), Direction::Click)
        .map_err(|e| anyhow::anyhow!("v click: {:?}", e))?;
    enigo
        .key(Key::Control, Direction::Release)
        .map_err(|e| anyhow::anyhow!("ctrl release: {:?}", e))?;

    Ok(())
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn paste_via_keyevent() -> Result<()> {
    // Linux fallback: requires xdotool
    let status = Command::new("xdotool")
        .args(["key", "ctrl+v"])
        .status()
        .context("xdotool failed — install with: apt install xdotool")?;
    if !status.success() {
        bail!("xdotool ctrl+v failed");
    }
    Ok(())
}

// --- Accessibility permission --------------------------------------------------

#[cfg(target_os = "macos")]
pub fn check_accessibility_permission() -> bool {
    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXIsProcessTrusted() -> bool;
    }
    unsafe { AXIsProcessTrusted() }
}

#[cfg(not(target_os = "macos"))]
pub fn check_accessibility_permission() -> bool {
    true // Windows/Linux don't require a special permission for key simulation
}

// Allow dead code — these FFI bindings are used in request_accessibility_permission()
// which may be called in future onboarding flows.
#[allow(dead_code)]
#[cfg(target_os = "macos")]
pub fn request_accessibility_permission() -> bool {
    use std::ffi::c_void;

    #[link(name = "ApplicationServices", kind = "framework")]
    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        static kAXTrustedCheckOptionPrompt: *const c_void;
        static kCFBooleanTrue: *const c_void;
        static kCFTypeDictionaryKeyCallBacks: *const c_void;
        static kCFTypeDictionaryValueCallBacks: *const c_void;
        fn AXIsProcessTrustedWithOptions(options: *const c_void) -> bool;
        fn CFDictionaryCreate(
            allocator: *const c_void,
            keys: *const *const c_void,
            values: *const *const c_void,
            num: isize,
            key_cbs: *const c_void,
            val_cbs: *const c_void,
        ) -> *mut c_void;
        fn CFRelease(cf: *const c_void);
    }

    unsafe {
        let keys = [kAXTrustedCheckOptionPrompt];
        let values = [kCFBooleanTrue];
        let dict = CFDictionaryCreate(
            std::ptr::null(),
            keys.as_ptr(),
            values.as_ptr(),
            1,
            kCFTypeDictionaryKeyCallBacks,
            kCFTypeDictionaryValueCallBacks,
        );
        let trusted = AXIsProcessTrustedWithOptions(dict as *const c_void);
        CFRelease(dict as *const c_void);
        trusted
    }
}

#[cfg(not(target_os = "macos"))]
pub fn request_accessibility_permission() -> bool {
    true
}

#[cfg(target_os = "macos")]
pub fn open_accessibility_settings() {
    let _ = Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
        .spawn();
}

#[cfg(not(target_os = "macos"))]
pub fn open_accessibility_settings() {}
