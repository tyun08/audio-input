use anyhow::{bail, Context, Result};
use std::process::Command;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, warn};

/// 将文字注入到当前焦点输入框
/// 策略：pbcopy 写剪贴板 → osascript 模拟 Cmd+V
/// 避免在 tokio 工作线程调用 arboard/enigo（它们需要主线程，否则崩溃）
pub async fn inject_text(text: &str) -> Result<()> {
    if text.is_empty() {
        return Ok(());
    }

    info!("注入文字: {:?}", text);

    // 用 pbcopy 写剪贴板（进程级调用，线程安全）
    write_clipboard(text)?;

    // 等待剪贴板就绪
    sleep(Duration::from_millis(80)).await;

    // 用 osascript 模拟 Cmd+V（不依赖 enigo，更可靠）
    paste_via_osascript()?;

    // 等待粘贴完成
    sleep(Duration::from_millis(150)).await;

    Ok(())
}

fn write_clipboard(text: &str) -> Result<()> {
    use std::io::Write;
    let mut child = Command::new("pbcopy")
        .stdin(std::process::Stdio::piped())
        .spawn()
        .context("启动 pbcopy 失败")?;

    if let Some(stdin) = child.stdin.as_mut() {
        stdin.write_all(text.as_bytes()).context("写入 pbcopy 失败")?;
    }
    let status = child.wait().context("等待 pbcopy 完成失败")?;
    if !status.success() {
        bail!("pbcopy 退出状态异常");
    }
    Ok(())
}

fn paste_via_osascript() -> Result<()> {
    let status = Command::new("osascript")
        .args([
            "-e",
            r#"tell application "System Events" to keystroke "v" using {command down}"#,
        ])
        .status()
        .context("启动 osascript 失败")?;

    if !status.success() {
        bail!("osascript 粘贴失败，请确认辅助功能权限已开启");
    }
    Ok(())
}

/// 检查 macOS Accessibility 权限（调用真实的系统 API）
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
