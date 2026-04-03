# Audio Input — Roadmap

## 已完成 ✓
- 系统托盘常驻，⌘⇧Space 全局快捷键
- cpal 录音 → Groq Whisper 转录 → CGEventPost 文字注入
- 打包 .app 完整可用（signingIdentity + Accessory 激活策略）
- 文件日志、API Key 配置、注入失败剪贴板 fallback

---

## P0 — 视觉重设计（阻塞上线）

**问题**：menu bar icon 和录音浮窗 UI 粗糙，不符合 macOS 原生应用质感。

**范围**：
- Menu bar 图标：idle / recording / processing / error 四态，符合 macOS template icon 规范（单色、@1x + @2x）
- 录音浮窗：重新设计布局、动效、排版——现在是纯功能性的占位 UI
- 整体设计语言：跟随 macOS Sonoma 风格，毛玻璃/圆角/SF Pro

**执行方式**：交给专门的 design agent 独立完成，输出 PNG/SVG 资源 + Svelte 组件，与其他开发并行。

---

## P0 — 转录后 LLM 润色

**问题**：Whisper 输出无标点、无断句，原始结果直接注入体验差。

**方案**：转录完成后，将 raw text 送入 LLM（Groq 的 llama 或 GPT-4o-mini）做后处理：

```
系统提示：你是一个文字整理助手。对输入的语音转录文字进行：
1. 添加适当标点和断句
2. 修正明显的同音错字
3. 保持原意，不改写内容
直接输出整理后的文字，不加任何解释。
```

**实现细节**：
- 用 Groq API（已有 key），模型用 `llama-3.1-8b-instant`（够快够便宜）
- 在 `transcription/` 模块里加 `polish.rs`，pipeline 变为：WAV → Whisper → LLM → inject
- 在设置里提供开关（默认开），以及"不润色直接注入"的选项
- 超时保护：LLM 超过 3s 未返回则降级用原始转录

---

## P1 — 核心体验完善

- **自定义快捷键**：设置面板里让用户配置，持久化到 config
- **麦克风选择**：列出所有输入设备，支持切换，记住上次选择
- **开机自启**：`tauri-plugin-autostart` 已装，在设置里加开关
- **声音反馈**：开始/停止录音的系统提示音（`NSSound` 或内嵌音效）

---

## P1 — 上线前准备

- **Apple Developer ID 签名 + Notarization**
  - 现在 ad-hoc 签名，用户安装时 Gatekeeper 警告"无法验证开发者"
  - 需要 $99/年 Apple Developer 账号
  - Tauri 支持配置 `signingIdentity` 和 `notarizationCredentials`，CI 里自动完成
- **隐私政策 & 使用条款**：App Store 审核或公开分发必须有
- **错误上报**：集成 Sentry 或类似工具，捕获 panic 和关键错误
- **自动更新**：`tauri-plugin-updater` + GitHub Releases，用户静默收到新版本
- **首次启动引导**：新用户流程——引导输入 API Key → 授权麦克风 → 授权 Accessibility → 完成
- **README + 安装文档**：面向非开发者用户的安装和授权步骤说明

---

## P2 — 功能扩展

- **离线模式**：集成本地 `whisper.cpp`，无需 API Key，无网络可用
- **语言设置**：强制指定识别语言（现在 Whisper 自动检测，偶尔误判）
- **转录历史**：记录最近 N 条，可复制/重新注入
- **Windows 支持**：cpal/CGEvent 替换为 Windows 等价实现

---

## 并行执行建议

```
现在
 ├── [design agent]  P0 视觉重设计（独立，输出资源文件）
 ├── [dev]           P0 LLM 润色层（polish.rs）
 └── [dev]           P1 麦克风选择 + 自定义快捷键

设计完成后
 └── [dev]           替换图标 + 浮窗 UI 组件

功能稳定后
 └── [dev/ops]       P1 上线前准备（签名、notarization、更新机制）
```
