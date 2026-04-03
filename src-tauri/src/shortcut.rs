use anyhow::{bail, Context, Result};
use tauri::Runtime;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};
use tracing::info;

/// Parse a shortcut string like "Meta+Shift+Space" into a Shortcut
pub fn parse_shortcut(s: &str) -> Result<Shortcut> {
    use tauri_plugin_global_shortcut::{Code, Modifiers};

    let mut mods = Modifiers::empty();
    let mut key_code: Option<Code> = None;

    for part in s.split('+') {
        match part.trim() {
            "Meta" | "Super" | "Cmd" | "Command" => mods |= Modifiers::META,
            "Ctrl" | "Control" => mods |= Modifiers::CONTROL,
            "Alt" | "Option" => mods |= Modifiers::ALT,
            "Shift" => mods |= Modifiers::SHIFT,
            key => {
                key_code = Some(match key {
                    "Space" => Code::Space,
                    "A" => Code::KeyA,
                    "B" => Code::KeyB,
                    "C" => Code::KeyC,
                    "D" => Code::KeyD,
                    "E" => Code::KeyE,
                    "F" => Code::KeyF,
                    "G" => Code::KeyG,
                    "H" => Code::KeyH,
                    "I" => Code::KeyI,
                    "J" => Code::KeyJ,
                    "K" => Code::KeyK,
                    "L" => Code::KeyL,
                    "M" => Code::KeyM,
                    "N" => Code::KeyN,
                    "O" => Code::KeyO,
                    "P" => Code::KeyP,
                    "Q" => Code::KeyQ,
                    "R" => Code::KeyR,
                    "S" => Code::KeyS,
                    "T" => Code::KeyT,
                    "U" => Code::KeyU,
                    "V" => Code::KeyV,
                    "W" => Code::KeyW,
                    "X" => Code::KeyX,
                    "Y" => Code::KeyY,
                    "Z" => Code::KeyZ,
                    "0" => Code::Digit0,
                    "1" => Code::Digit1,
                    "2" => Code::Digit2,
                    "3" => Code::Digit3,
                    "4" => Code::Digit4,
                    "5" => Code::Digit5,
                    "6" => Code::Digit6,
                    "7" => Code::Digit7,
                    "8" => Code::Digit8,
                    "9" => Code::Digit9,
                    "F1" => Code::F1,
                    "F2" => Code::F2,
                    "F3" => Code::F3,
                    "F4" => Code::F4,
                    "F5" => Code::F5,
                    "F6" => Code::F6,
                    "F7" => Code::F7,
                    "F8" => Code::F8,
                    "F9" => Code::F9,
                    "F10" => Code::F10,
                    "F11" => Code::F11,
                    "F12" => Code::F12,
                    other => bail!("未知按键: {}", other),
                });
            }
        }
    }

    let code = key_code.context("快捷键中缺少主键")?;
    Ok(Shortcut::new(Some(mods), code))
}

pub fn reregister_shortcut<R: Runtime>(
    app: &tauri::AppHandle<R>,
    shortcut_str: &str,
) -> Result<()> {
    use crate::commands::RecorderState;
    use crate::state::SharedState;
    use std::sync::Arc;
    use tauri::Manager;
    use tauri_plugin_global_shortcut::ShortcutState;

    // Unregister all existing shortcuts
    let _ = app.global_shortcut().unregister_all();

    let shortcut = parse_shortcut(shortcut_str)?;

    let handle2 = app.clone();
    let shared_state2 = app.state::<SharedState>().inner().clone();
    let recorder2 = Arc::clone(&app.state::<RecorderState>().inner().0);

    app.global_shortcut()
        .on_shortcut(shortcut, move |_app, _shortcut, event| {
            if event.state() == ShortcutState::Pressed {
                let app = handle2.clone();
                let state = shared_state2.clone();
                let rec = Arc::clone(&recorder2);
                tauri::async_runtime::spawn(async move {
                    crate::commands::toggle_recording(app, state, rec).await;
                });
            }
        })
        .context("注册快捷键失败")?;

    info!("快捷键重新注册: {}", shortcut_str);
    Ok(())
}
