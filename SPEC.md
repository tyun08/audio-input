# 桌面语音输入助手 — 产品规格

## 项目目标

开发一个跨平台桌面语音输入助手，让用户可以在任意文字输入框使用语音输入。
核心价值：语音 → 转录 → 文字，不做传统输入法的其他功能。

## 核心功能

1. 系统托盘图标（麦克风 icon）常驻
2. 点击图标 或 全局快捷键触发录音开始/停止
3. 录音结束后发送到 Groq Whisper API 转录
4. 转录文字自动粘贴到当前焦点输入框

## 技术栈

| 层 | 选型 | 原因 |
|----|------|------|
| 桌面框架 | Tauri 2.x | 产物 ~5MB，无需打包 Chromium；Rust 原生系统调用 |
| 前端 | Svelte + TypeScript | 无运行时，bundle 极小，状态 UI 简单 |
| 录音 | `cpal` 0.15 | 跨平台 CoreAudio/WASAPI/ALSA |
| 音频编码 | `hound` 3.5 | 纯 Rust WAV 编码，不写磁盘 |
| HTTP 客户端 | `reqwest` 0.12 | async multipart/form-data |
| 文字注入 | `arboard` + `enigo` | 剪贴板写入 + 模拟 Cmd+V |
| 全局快捷键 | `tauri-plugin-global-shortcut` 2.x | Tauri 官方插件 |

平台优先级：macOS 优先，Windows 次之，Linux 可选。

## 已确认设计决策

| 项目 | 决策 |
|------|------|
| 快捷键 | `Cmd+Shift+Space` |
| 录音模式 | Toggle（按一次开始，再按一次停止） |
| API Key 管理 | 应用内配置 UI（托盘菜单入口，持久化到 app data dir） |
| 状态浮窗 | 需要，简洁精美的录音 icon 浮窗，常驻顶层，不抢焦点 |

## 项目目录结构

```
audio-input/
├── src-tauri/
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── build.rs
│   ├── icons/
│   │   ├── tray-idle.png
│   │   ├── tray-recording.png
│   │   └── tray-processing.png
│   └── src/
│       ├── main.rs               # 入口：Tauri Builder + 插件注册
│       ├── tray.rs               # 系统托盘管理
│       ├── hotkey.rs             # 全局快捷键
│       ├── state.rs              # AppState（Arc<Mutex<RecordingState>>）
│       ├── commands.rs           # Tauri IPC commands
│       ├── audio/
│       │   ├── mod.rs
│       │   ├── recorder.rs       # 麦克风录音
│       │   └── encoder.rs        # WAV 编码
│       ├── transcription/
│       │   ├── mod.rs
│       │   └── groq.rs           # Groq Whisper API 客户端
│       └── input/
│           ├── mod.rs
│           └── injector.rs       # 文字注入
├── src/
│   ├── App.svelte                # 状态浮窗 UI
│   ├── lib/
│   │   ├── stores.ts
│   │   └── tauri.ts
│   └── main.ts
├── SPEC.md                       # 本文件
├── TASKS.md                      # 任务追踪
├── .env.example
├── vite.config.ts
└── package.json
```

## Groq API

- Endpoint: `POST https://api.groq.com/openai/v1/audio/transcriptions`
- Auth: `Authorization: Bearer $GROQ_API_KEY`
- Model: `whisper-large-v3-turbo`
- 输入格式：`multipart/form-data`，文件字段 `file`（WAV bytes）
- 响应格式：`verbose_json`，取 `.text` 字段

参考实现见 `whisper-example.py`。

## 文字注入方案

**主方案：剪贴板 + 模拟粘贴**

1. 保存当前剪贴板内容
2. 写入转录文字到剪贴板
3. 延迟 50ms（等待剪贴板写入）
4. 模拟 `Cmd+V`（macOS）或 `Ctrl+V`（Windows/Linux）
5. 延迟 100ms（等待粘贴完成）
6. 恢复原剪贴板内容（恢复前先检查是否已被用户修改）

**前提**：macOS 需要辅助功能（Accessibility）权限。首次运行检测并引导用户开启。

**备用方案**：`osascript` (`tell application "System Events" to keystroke "v" using {command down}`)

## 状态机

```
Idle ──触发──→ Recording ──触发──→ Processing ──成功──→ Idle
  ↑                                     │
  └──────────────── 错误回退 ────────────┘
```

状态对应托盘图标：
- `Idle`：麦克风图标（正常色）
- `Recording`：麦克风图标（红色）
- `Processing`：加载图标
- `Error`：短暂显示错误图标（1.5s）后回 Idle

## 权限声明（macOS）

- `NSMicrophoneUsageDescription`：用于语音输入转录
- Accessibility：用于模拟键盘输入（引导用户手动开启）

## 风险与备选

| 风险 | 缓解方案 |
|------|----------|
| enigo 在某些应用中模拟粘贴失效 | 备用 osascript |
| cpal 录音配置问题 | 显式指定默认输入设备和格式 |
| 全局快捷键冲突 | 允许用户自定义，注册失败时提示 |
| Groq API 不可用 | 错误提示，可选本地 whisper.cpp（离线备用） |
| 剪贴板恢复竞态 | 恢复前检查当前内容，已变化则跳过恢复 |
