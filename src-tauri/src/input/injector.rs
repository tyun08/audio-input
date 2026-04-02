use anyhow::{bail, Context, Result};
use std::process::Command;
use std::time::Duration;
use tokio::time::sleep;
use tracing::info;

/// 将文字注入到当前焦点输入框
pub async fn inject_text(text: &str) -> Result<()> {
    if text.is_empty() {
        return Ok(());
    }

    info!("注入文字: {:?}", text);

    // 用 pbcopy 写剪贴板（子进程，线程安全，避免 NSPasteboard 主线程要求）
    write_clipboard(text)?;

    // 等待剪贴板就绪
    sleep(Duration::from_millis(80)).await;

    // 用 CGEvent 模拟 Cmd+V（在进程内直接调用，不依赖 osascript 的独立权限）
    paste_via_cgevent()?;

    // 等待粘贴完成
    sleep(Duration::from_millis(150)).await;

    Ok(())
}

fn write_clipboard(text: &str) -> Result<()> {
    use std::io::Write;
    let mut child = Command::new("/usr/bin/pbcopy")
        .stdin(std::process::Stdio::piped())
        .spawn()
        .context("启动 pbcopy 失败")?;

    // 必须 take() 拿走所有权，block 结束时 ChildStdin drop → 发送 EOF → pbcopy 才会退出
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(text.as_bytes()).context("写入 pbcopy 失败")?;
        // drop(stdin) 在这里隐式发生
    }

    let status = child.wait().context("等待 pbcopy 完成失败")?;
    if !status.success() {
        bail!("pbcopy 退出状态异常: {:?}", status.code());
    }
    Ok(())
}

/// 用 CoreGraphics CGEvent 直接 post Cmd+V，不依赖 osascript
/// 要求：app 已获得 Accessibility 权限（AXIsProcessTrusted() == true）
#[cfg(target_os = "macos")]
fn paste_via_cgevent() -> Result<()> {
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
    const KCG_EVENT_FLAG_MASK_COMMAND: u64 = 0x0010_0000; // NX_COMMANDMASK

    unsafe {
        let down = CGEventCreateKeyboardEvent(ptr::null(), KVK_ANSI_V, true);
        if down.is_null() {
            bail!("CGEventCreateKeyboardEvent(down) 返回 null");
        }
        CGEventSetFlags(down, KCG_EVENT_FLAG_MASK_COMMAND);
        CGEventPost(KCG_HID_EVENT_TAP, down);
        CFRelease(down);

        let up = CGEventCreateKeyboardEvent(ptr::null(), KVK_ANSI_V, false);
        if up.is_null() {
            bail!("CGEventCreateKeyboardEvent(up) 返回 null");
        }
        CGEventSetFlags(up, KCG_EVENT_FLAG_MASK_COMMAND);
        CGEventPost(KCG_HID_EVENT_TAP, up);
        CFRelease(up);
    }

    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn paste_via_cgevent() -> Result<()> {
    // Windows/Linux fallback
    let status = Command::new("xdotool")
        .args(["key", "ctrl+v"])
        .status()
        .context("xdotool 失败")?;
    if !status.success() {
        bail!("xdotool ctrl+v 失败");
    }
    Ok(())
}

/// 检查 macOS Accessibility 权限
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
    true
}

/// 打开 macOS 辅助功能设置页
#[cfg(target_os = "macos")]
pub fn open_accessibility_settings() {
    let _ = Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
        .spawn();
}

#[cfg(not(target_os = "macos"))]
pub fn open_accessibility_settings() {}
