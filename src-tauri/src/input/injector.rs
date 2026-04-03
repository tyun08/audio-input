use anyhow::{bail, Context, Result};
use std::process::Command;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, error, info, warn};

pub async fn inject_text(text: &str) -> Result<()> {
    if text.is_empty() {
        return Ok(());
    }

    info!("inject_text: {} 个字符, 前20字: {:?}", text.chars().count(), text.chars().take(20).collect::<String>());

    // 先写剪贴板，无论是否有 AX 权限都写（方便用户手动粘贴）
    info!("步骤1: 写入剪贴板 (pbcopy)");
    write_clipboard(text)?;
    info!("步骤1完成: pbcopy 成功");

    sleep(Duration::from_millis(100)).await;

    // AX 权限检查：没有权限时直接返回错误，不尝试无效的 CGEvent
    if !check_accessibility_permission() {
        bail!("Accessibility 权限未授予 — 文字已写入剪贴板，请按 ⌘V 手动粘贴。\n如需自动注入，请在系统设置中授权辅助功能后重启 App。");
    }

    info!("步骤2: 模拟 Cmd+V (CGEvent)");
    paste_via_cgevent()?;
    info!("步骤2完成: CGEvent 成功");

    sleep(Duration::from_millis(200)).await;
    Ok(())
}

fn write_clipboard(text: &str) -> Result<()> {
    use std::io::Write;

    debug!("pbcopy: 写入 {} 字节", text.as_bytes().len());

    let mut child = Command::new("/usr/bin/pbcopy")
        // 显式指定 UTF-8，防止打包 app 环境里 locale 未设置导致乱码
        .env("LANG", "en_US.UTF-8")
        .env("LC_ALL", "en_US.UTF-8")
        .stdin(std::process::Stdio::piped())
        .spawn()
        .context("启动 /usr/bin/pbcopy 失败")?;

    // 必须 take() 拿走所有权，使 ChildStdin 在 block 末尾 drop，
    // 发送 EOF，pbcopy 才会退出并提交剪贴板内容
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(text.as_bytes()).context("写入 pbcopy stdin 失败")?;
        // 显式 drop，确保 flush + close
        drop(stdin);
    } else {
        bail!("pbcopy stdin 句柄为空");
    }

    let status = child.wait().context("等待 pbcopy 退出失败")?;
    if !status.success() {
        bail!("pbcopy 退出码异常: {:?}", status.code());
    }

    debug!("pbcopy 退出码: {:?}", status.code());
    Ok(())
}

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
    const KCG_EVENT_FLAG_MASK_COMMAND: u64 = 0x0010_0000;

    unsafe {
        let down = CGEventCreateKeyboardEvent(ptr::null(), KVK_ANSI_V, true);
        if down.is_null() {
            bail!("CGEventCreateKeyboardEvent(down) 返回 null，检查 Accessibility 权限");
        }
        CGEventSetFlags(down, KCG_EVENT_FLAG_MASK_COMMAND);
        CGEventPost(KCG_HID_EVENT_TAP, down);
        CFRelease(down);

        let up = CGEventCreateKeyboardEvent(ptr::null(), KVK_ANSI_V, false);
        if up.is_null() {
            error!("CGEventCreateKeyboardEvent(up) 返回 null");
        } else {
            CGEventSetFlags(up, KCG_EVENT_FLAG_MASK_COMMAND);
            CGEventPost(KCG_HID_EVENT_TAP, up);
            CFRelease(up);
        }
    }

    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn paste_via_cgevent() -> Result<()> {
    let status = Command::new("xdotool")
        .args(["key", "ctrl+v"])
        .status()
        .context("xdotool 失败")?;
    if !status.success() {
        bail!("xdotool ctrl+v 失败");
    }
    Ok(())
}

#[cfg(target_os = "macos")]
pub fn check_accessibility_permission() -> bool {
    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXIsProcessTrusted() -> bool;
    }
    unsafe { AXIsProcessTrusted() }
}

/// 弹出系统 Accessibility 授权对话框（首次运行或未授权时调用）
/// 返回当前是否已授权
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

#[cfg(not(target_os = "macos"))]
pub fn check_accessibility_permission() -> bool {
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
