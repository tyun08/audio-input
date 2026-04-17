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

    // Keep the clipboard handle alive until after the paste so macOS doesn't
    // reclaim ownership and clear the content when the handle is dropped.
    let mut clipboard = arboard::Clipboard::new().context("clipboard init failed")?;

    // Save the current clipboard content so we can restore it after a successful paste.
    let previous_clipboard = clipboard.get_text().ok();

    clipboard.set_text(text).context("clipboard write failed")?;
    info!("clipboard write OK");

    sleep(Duration::from_millis(100)).await;

    if !check_accessibility_permission() {
        bail!("Accessibility permission not granted — text copied to clipboard, press ⌘V to paste manually.");
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
pub fn open_accessibility_settings() {
    let _ = Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
        .spawn();
}

#[cfg(not(target_os = "macos"))]
pub fn open_accessibility_settings() {}
