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

    info!("步骤1: 写入剪贴板 (pbcopy)");
    write_clipboard(text)?;
    info!("步骤1完成: pbcopy 成功");

    sleep(Duration::from_millis(100)).await;

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

    // 前置检查：若 Accessibility 权限未授予，CGEvent 不会生效
    if !check_accessibility_permission() {
        warn!("AXIsProcessTrusted = false，CGEvent Cmd+V 可能无效，请重启 app 后重试");
        // 不 bail，仍然尝试（权限有时有延迟）
    } else {
        debug!("AXIsProcessTrusted = true");
    }

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
