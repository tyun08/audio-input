use anyhow::Result;
use arboard::Clipboard;
use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use std::time::Duration;
use tokio::time::sleep;
use tracing::info;

/// 将文字注入到当前焦点输入框
/// 策略：写入剪贴板 → 模拟 Cmd+V → 不恢复剪贴板（保留转录内容供手动粘贴兜底）
pub async fn inject_text(text: &str) -> Result<()> {
    if text.is_empty() {
        return Ok(());
    }

    info!("注入文字: {:?}", text);

    let mut clipboard = Clipboard::new()?;

    // 写入转录文字
    clipboard.set_text(text)?;

    // 等待剪贴板写入完成
    sleep(Duration::from_millis(100)).await;

    // 模拟粘贴（失败则返回 Err，调用方决定如何处理）
    paste_clipboard()?;

    // 等待粘贴操作完成
    sleep(Duration::from_millis(200)).await;

    // 注意：不恢复剪贴板。
    // 理由：无法可靠判断粘贴是否真的成功（enigo 发键后不报错但可能焦点错误）。
    // 保留转录内容在剪贴板，作为手动粘贴的兜底。

    Ok(())
}

fn paste_clipboard() -> Result<()> {
    let mut enigo = Enigo::new(&Settings::default())?;

    #[cfg(target_os = "macos")]
    {
        enigo.key(Key::Meta, Direction::Press)?;
        enigo.key(Key::Unicode('v'), Direction::Click)?;
        enigo.key(Key::Meta, Direction::Release)?;
    }

    #[cfg(not(target_os = "macos"))]
    {
        enigo.key(Key::Control, Direction::Press)?;
        enigo.key(Key::Unicode('v'), Direction::Click)?;
        enigo.key(Key::Control, Direction::Release)?;
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
    let _ = std::process::Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
        .spawn();
}

#[cfg(not(target_os = "macos"))]
pub fn open_accessibility_settings() {}
