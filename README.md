# Audio Input

macOS 菜单栏语音输入工具。按快捷键录音，松手后自动转录并注入当前输入框。

## 功能

- 全局快捷键（默认 `⌘⇧Space`）或点击菜单栏图标触发录音
- 转录完成后自动粘贴到当前光标位置
- 可选 LLM 润色层（消除口语停顿，修正标点）
- 支持选择麦克风、自定义快捷键、开机自启
- 常驻菜单栏，极低资源占用（~20MB 内存）

## 快速开始

### 1. 获取 Groq API Key

在 [console.groq.com](https://console.groq.com) 免费注册，创建 API Key。

### 2. 安装

从 [Releases](../../releases) 下载最新 `.dmg`，拖入 Applications。

首次启动会要求授予**麦克风**和**辅助功能**权限（辅助功能权限用于注入文字）。

### 3. 配置 API Key

点击菜单栏麦克风图标 → 右键 → **配置 API Key**，粘贴 Groq Key 并保存。

### 4. 使用

1. 在任意输入框中，按 `⌘⇧Space` 开始录音
2. 再按一次（或点击菜单栏图标）停止
3. 转录结果自动注入光标位置

## 菜单栏图标状态

| 图标 | 含义 |
|------|------|
| 黑色麦克风 | 待机 |
| 红色麦克风 | 录音中 |
| 蓝色麦克风 | 转录处理中 |
| 橙色麦克风 | 出错（见 tooltip） |

## 设置

右键菜单栏图标 → **配置 API Key** 打开设置面板，可配置：

- **API Key** — Groq API Key
- **润色** — 启用/关闭 LLM 润色层
- **麦克风** — 选择输入设备
- **快捷键** — 自定义全局快捷键（如 `Ctrl+Alt+R`）
- **开机自启** — 登录时自动启动

## 本地开发

```bash
# 依赖
brew install node
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 克隆
git clone https://github.com/your-username/audio-input
cd audio-input
npm install

# 配置 API Key（开发用）
cp .env.example .env
# 编辑 .env，填入 GROQ_API_KEY=gsk_...

# 开发模式
npm run tauri dev

# 构建
npm run tauri build
```

## 技术栈

- **Tauri 2** — 原生 macOS 容器（~5MB 产物）
- **Rust** — 录音（cpal）、转录请求（reqwest）、快捷键、文字注入
- **Svelte** — 设置面板 UI
- **Groq API** — Whisper large-v3-turbo 转录 + LLM 润色

## 已知限制

- 仅支持 macOS（Apple Silicon & Intel）
- 文字注入依赖辅助功能权限，部分沙盒 App 可能无法注入
- API Key 以明文存储在 `~/Library/Application Support/com.audioinput.app/config.json`

## License

MIT
