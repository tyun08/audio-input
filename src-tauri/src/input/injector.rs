use anyhow::{bail, Context, Result};
use std::process::Command;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{error, info, warn};

pub async fn inject_text(text: &str) -> Result<()> {
    if text.is_empty() {
        return Ok(());
    }

    info!("inject_text: {} chars", text.chars().count());

    // Keep the clipboard handle alive until after the paste so macOS doesn't
    // reclaim ownership and clear the content when the handle is dropped.
    let mut clipboard = arboard::Clipboard::new().context("clipboard init failed")?;

    // Save the current clipboard content so we can restore it after a successful paste.
    let previous_clipboard = clipboard.get_text().ok();

    clipboard.set_text(text).context("clipboard write failed")?;
    info!("clipboard write OK");

    sleep(Duration::from_millis(100)).await;

    if !check_accessibility_permission() {
        // Prompt the user via the macOS system dialog (opens System Settings →
        // Privacy & Security → Accessibility). The text is already on the
        // clipboard so they can ⌘V manually while granting permission.
        request_accessibility_permission();
        bail!("Accessibility permission not granted — text copied to clipboard, press ⌘V to paste manually.");
    }

    if !has_focused_text_input() {
        info!("no focused text input — text copied to clipboard, ⌘V to paste manually");
        bail!("No focused text input — text is on your clipboard, press ⌘V to paste.");
    }

    info!("simulating paste keypress");
    paste_via_keyevent()?;

    sleep(Duration::from_millis(200)).await;

    // Restore the previous clipboard content after a successful paste so the
    // user can still access their previous clipboard item.
    if let Some(prev) = previous_clipboard {
        if let Err(e) = clipboard.set_text(&prev) {
            warn!("Failed to restore previous clipboard content: {}", e);
        } else {
            info!("Previous clipboard content restored");
        }
    }

    // clipboard dropped here, after paste is complete and restore is done
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

    const KCG_SESSION_EVENT_TAP: i32 = 1;
    const KVK_ANSI_V: u16 = 9;
    const KCG_EVENT_FLAG_MASK_COMMAND: u64 = 0x0010_0000;

    unsafe {
        let down = CGEventCreateKeyboardEvent(ptr::null(), KVK_ANSI_V, true);
        if down.is_null() {
            bail!("CGEventCreateKeyboardEvent returned null — check Accessibility permission");
        }
        CGEventSetFlags(down, KCG_EVENT_FLAG_MASK_COMMAND);
        CGEventPost(KCG_SESSION_EVENT_TAP, down);
        CFRelease(down);

        let up = CGEventCreateKeyboardEvent(ptr::null(), KVK_ANSI_V, false);
        if up.is_null() {
            error!("CGEventCreateKeyboardEvent(up) returned null");
        } else {
            CGEventSetFlags(up, KCG_EVENT_FLAG_MASK_COMMAND);
            CGEventPost(KCG_SESSION_EVENT_TAP, up);
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

// --- Focused text input detection ---------------------------------------------

/// Returns true if the currently-focused UI element supports text editing
/// (i.e. it has an `AXSelectedTextRange` attribute).  We use this to avoid
/// silently "pasting" into a non-text target when the user has clicked away
/// from an input field between recording and inject time.
#[cfg(target_os = "macos")]
fn has_focused_text_input() -> bool {
    use std::ffi::{c_char, c_void, CString};
    use std::ptr;

    // kCFStringEncodingUTF8 = 0x08000100
    const UTF8: u32 = 0x08000100;

    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXUIElementCreateSystemWide() -> *mut c_void;
        fn AXUIElementCopyAttributeValue(
            element: *const c_void,
            attribute: *const c_void,
            value: *mut *const c_void,
        ) -> i32;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFRelease(cf: *const c_void);
        fn CFStringCreateWithCString(
            alloc: *const c_void,
            cstr: *const c_char,
            encoding: u32,
        ) -> *mut c_void;
    }

    unsafe {
        let system_wide = AXUIElementCreateSystemWide();
        if system_wide.is_null() {
            return false;
        }

        // Get the currently-focused UI element.
        let attr = CString::new("AXFocusedUIElement").unwrap();
        let attr_cf = CFStringCreateWithCString(ptr::null(), attr.as_ptr(), UTF8);
        let mut focused: *const c_void = ptr::null();
        let err = AXUIElementCopyAttributeValue(system_wide, attr_cf, &mut focused);
        CFRelease(attr_cf);
        CFRelease(system_wide);

        if err != 0 || focused.is_null() {
            return false;
        }

        // Any element that accepts keyboard text (AXTextField, AXTextArea, and
        // contenteditable regions in browsers) exposes AXSelectedTextRange.
        // Non-editable elements (buttons, images, etc.) return an error here.
        let range_attr = CString::new("AXSelectedTextRange").unwrap();
        let range_cf = CFStringCreateWithCString(ptr::null(), range_attr.as_ptr(), UTF8);
        let mut range_val: *const c_void = ptr::null();
        let range_err = AXUIElementCopyAttributeValue(focused, range_cf, &mut range_val);
        CFRelease(range_cf);
        CFRelease(focused);

        if !range_val.is_null() {
            CFRelease(range_val);
        }

        range_err == 0
    }
}

#[cfg(not(target_os = "macos"))]
fn has_focused_text_input() -> bool {
    true // Windows / Linux: assume yes; platform-specific check can be added later
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

#[cfg(target_os = "macos")]
pub fn request_accessibility_permission() -> bool {
    use std::ffi::c_void;

    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        static kAXTrustedCheckOptionPrompt: *const c_void;
        static kCFBooleanTrue: *const c_void;
        static kCFTypeDictionaryKeyCallBacks: *const c_void;
        static kCFTypeDictionaryValueCallBacks: *const c_void;
        fn AXIsProcessTrustedWithOptions(options: *const c_void) -> bool;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
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
