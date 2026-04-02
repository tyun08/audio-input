# 实现计划

## 目标

开发一个 macOS 优先的桌面语音输入助手，用户按快捷键录音，松手后自动转录并粘贴到当前焦点输入框。

---

## 架构决策

### 为什么选 Tauri 而非 Electron

| 对比项 | Tauri | Electron |
|--------|-------|----------|
| 产物大小 | ~5MB | ~80-150MB |
| 内存占用 | ~20-40MB | ~80-150MB |
| 系统 API 访问 | Rust 直接调用，零开销 | Node.js native addon，N-API 层 |
| 适合场景 | 常驻后台系统工具 | 复杂跨平台应用 |

本项目是常驻后台的轻量工具，Tauri 是明确更优的选择。

### 文字注入方案选择

**选定：剪贴板 + 模拟 Cmd+V**

逐字符注入（`CGEventCreateKeyboardEvent`）需要处理 Unicode 映射、输入法状态、特殊字符转义，复杂度高且容易出问题。剪贴板方案：
- 实现简单可靠
- 支持全部 Unicode（中文、emoji 等）
- 适用于几乎所有应用

代价是需要临时覆盖剪贴板（粘贴后恢复）。

---

## 实现阶段

### Phase 0 — 脚手架（Day 1）

初始化 Tauri 2 + Svelte 项目，配置窗口默认隐藏（启动不弹窗），声明 macOS 麦克风权限。

关键配置：
- `tauri.conf.json`：`"visible": false`，启用 tray 特性
- `Info.plist`：`NSMicrophoneUsageDescription`

### Phase 1 — 系统托盘（Day 1-2）

常驻菜单栏的麦克风图标，承载三个状态的视觉反馈：

```
Idle（默认图标） → Recording（红色）→ Processing（加载）→ Idle
                                              ↓ 失败
                                          Error（1.5s 后回 Idle）
```

状态用 `Arc<Mutex<AppState>>` 管理，确保线程安全。

### Phase 2 — 全局快捷键（Day 2）

用 `tauri-plugin-global-shortcut` 注册 `Cmd+Shift+Space`（默认值，后续可自定义）。

托盘左键点击与快捷键调用同一个 `toggle_recording()` 函数，行为完全一致。

### Phase 3 — 麦克风录音（Day 3-4）

用 `cpal` 获取默认输入设备，录音数据写入内存缓冲（`Arc<Mutex<Vec<f32>>>`），不写磁盘。

停止后用 `hound` 编码为 WAV bytes（16kHz，单声道，16-bit PCM）——Whisper 的最优输入格式。

### Phase 4 — Groq API（Day 4-5）

用 `reqwest` 发 multipart/form-data 请求：

```
POST https://api.groq.com/openai/v1/audio/transcriptions
Authorization: Bearer $GROQ_API_KEY

file=<wav bytes>
model=whisper-large-v3-turbo
temperature=0
response_format=verbose_json
```

解析响应取 `.text` 字段。

API Key 从环境变量 `GROQ_API_KEY` 读取（优先），缺失时托盘右键菜单显示配置入口。

### Phase 5 — 文字注入（Day 5-6）

```rust
// 伪代码，展示逻辑
let prev = clipboard.get_text();
clipboard.set_text(transcribed_text);
sleep(50ms);                          // 等待剪贴板写入
simulate_cmd_v();                     // enigo 模拟粘贴
sleep(100ms);                         // 等待粘贴完成
if clipboard.get_text() == transcribed_text {
    clipboard.set_text(prev);         // 恢复，除非用户已修改剪贴板
}
```

首次运行检测 macOS Accessibility 权限，缺失时弹提示并跳转系统设置。

### Phase 6 — 端到端串联（Day 6-7）

把 Phase 1-5 的模块串联成完整流程，加统一错误处理。

Svelte 前端做一个极简浮窗（`always_on_top: true`，`focus: false`）显示录音状态。后端通过 Tauri events 推送状态变更。

### Phase 7 — 打包（Day 8）

`cargo tauri build` 产出 `.dmg`，配置签名。可选：开机自启（`tauri-plugin-autostart`）。

---

## 依赖清单

```toml
[dependencies]
tauri = { version = "2", features = ["tray-icon", "image-png"] }
tauri-plugin-global-shortcut = "2"
tauri-plugin-notification = "2"
tauri-plugin-autostart = "2"
cpal = "0.15"
hound = "3.5"
reqwest = { version = "0.12", features = ["multipart", "json"] }
tokio = { version = "1", features = ["full"] }
arboard = "3"
enigo = "0.2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
anyhow = "1"
dotenvy = "0.15"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
```

---

## 风险

| 风险 | 概率 | 备选方案 |
|------|------|----------|
| `enigo` 在某些应用中模拟粘贴失效 | 中 | `osascript` 调用 AppleScript |
| `cpal` 默认设备配置不兼容 | 低 | 显式枚举设备，选第一个支持输入的 |
| 全局快捷键与系统应用冲突 | 低 | 提供自定义快捷键入口 |
| Groq API 不可用（离线/限流） | 低 | 错误提示；可选本地 whisper.cpp |
| 剪贴板恢复竞态 | 低 | 恢复前校验内容是否已变化 |

---

## 已确认设计决策

| 项目 | 决策 |
|------|------|
| 快捷键 | `Cmd+Shift+Space` |
| 录音模式 | Toggle（按一次开始，再按一次停止） |
| API Key 管理 | 应用内配置 UI（托盘菜单入口，小窗口输入并持久化） |
| 状态浮窗 | 需要，简洁精美的录音 icon 浮窗（常驻顶层，不抢焦点） |
