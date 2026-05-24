# InputMethodKit 集成计划

> 从"转录工具"升级为"系统级写作助手"

> **状态（2026-05-24）：设计完成，未实施。** Phase 1（Swift IMK helper 骨架）
> 和 Phase 2（Rust IPC client）有早期 WIP 代码，曾在 `feat/input-method-kit`
> 分支（commit `bf87e5b`），但因为同期主干大量改动而搁置。下次推进时以本
> 文档为权威，参考那个分支的代码作历史素材，但 `commands.rs` / `injector.rs`
> 的接入要重写以匹配当前代码结构。

---

## 一、为什么要做

### 现有架构的根本限制

| 限制 | 原因 |
|---|---|
| 某些 App 注入失败（终端、Electron、沙盒 App） | 依赖 clipboard + CGEventPostToPid(⌘V)，各 App 对 ⌘V 行为不一致 |
| 需要 Accessibility 权限才能注入 | CGEvent API 要求 |
| 读不到光标前文本 / 选中文本 | AX API 在 Electron / 沙盒 App 中不可靠 |
| 注入后 ⌘Z 历史混乱 | 剪贴板粘贴不参与目标 App 的 undo stack |
| HUD 只能显示在屏幕固定位置 | 不知道光标的精确坐标 |

### IMK 解锁的新能力

| 能力 | 产品价值 |
|---|---|
| `insertText:replacementRange:` 直接插入 | 注入覆盖所有 App，无需 Accessibility 权限 |
| `selectedRange` + `attributedSubstringFromRange:` | 读到光标前文本 → 上下文感知的转录润色 |
| 读选中文本 + `replacementRange` | 选中即变换：翻译、精简、改写语气 |
| `firstRectForCharacterRange:` | 光标精确屏幕坐标 → HUD 贴着光标显示 |
| Marked text（preedit） | 流式插入：转录结果实时"打出来" |
| 原生 undo stack 集成 | ⌘Z 精确撤销，不破坏 App 历史 |
| IMK 始终知道焦点字段 | per-app 行为、使用分析、上下文切换感知 |

---

## 二、目标用户体验

### 场景 A：上下文感知转录（核心升级）

```
用户在邮件里已写: "Hi John, regarding our meeting next week..."
→ 按录音键，说"我们需要把预算提高到五万美元"
→ 系统读到光标前内容，识别这是英文邮件场景
→ 润色输出: "we'll need to increase the budget to $50,000"
→ 直接插入，语气格式与已有文本匹配
```

### 场景 B：选中变换

```
用户选中一段文字 → 按录音键 → 说"翻译成英文"
→ IMK 读到 selectedRange + 文本内容
→ LLM(原文 + 指令) → insertText(结果, replacementRange: selection)
→ 直接替换，⌘Z 可撤销
```

### 场景 C：流式插入（体感升级）

```
录音开始 → 插入 "▌" marked text（光标处可见占位）
转录流式返回 → marked text 实时更新为实际文字
转录完成 → commitComposition → 文字稳定
```

### 场景 D：HUD 贴光标显示

```
录音键按下 → 查询 firstRectForCharacterRange
→ HUD 弹出在光标右侧 8pt 处
→ 不再是屏幕角落的浮窗，而是内联感的反馈
```

---

## 三、架构

```
┌─────────────────────────────────┐     Unix Socket IPC
│         Tauri App               │ ◄──────────────────────►  ┌──────────────────────────────┐
│                                 │                            │    IMK Helper.app            │
│  ・录音 / 转录 / 润色            │   InsertRequest {          │                              │
│  ・全局快捷键                    │     text: String,          │  ・常驻后台进程               │
│  ・HUD / UI                     │     replacement_range:     │  ・IMKInputController         │
│  ・账号 / 配置 / 历史            │       Option<Range>,       │  ・监听 Unix socket           │
│                                 │     cursor_rect: Rect,     │  ・insertText → 目标 App      │
│  inject_text() →                │   }                        │                              │
│    IPC client                   │                            │  ContextResponse {            │
│                                 │   ContextRequest {}  ◄──── │    before_cursor: String,     │
│                                 │                            │    selected_text: String,     │
│                                 │                            │    cursor_rect: CGRect,       │
│                                 │                            │    app_bundle_id: String,     │
│                                 │                            │  }                            │
└─────────────────────────────────┘                            └──────────────────────────────┘
         ↑                                                                    ↑
    现有代码改动最小                                                  新建 Swift 进程
    只替换 inject_text() 底层                                        ~/Library/Input Methods/
```

### IPC 消息协议（newline-delimited JSON over Unix socket）

```jsonc
// Tauri → IMK: 插入文字
{ "type": "insert", "text": "hello world", "replacement_range": null }

// Tauri → IMK: 插入并替换选中区域
{ "type": "insert", "text": "hello world", "replacement_range": { "location": 10, "length": 5 } }

// Tauri → IMK: 查询上下文
{ "type": "get_context" }

// IMK → Tauri: 上下文响应
{
  "type": "context",
  "before_cursor": "Hi John, regarding our meeting next week...",
  "selected_text": "",
  "cursor_rect": { "x": 320.0, "y": 480.0, "w": 2.0, "h": 18.0 },
  "app_bundle_id": "com.apple.mail",
  "app_name": "Mail"
}

// IMK → Tauri: 确认
{ "type": "ok" }

// IMK → Tauri: 错误
{ "type": "error", "message": "no active client" }
```

Socket 路径：`/tmp/audio-input-imk.sock`

---

## 四、实现阶段

### Phase 1 — IMK Helper 骨架（当前）

**目标**：一个能注册为系统输入法、监听 socket、执行 `insertText` 的最小 Swift app。

**产出物**：
- `imk-helper/` 目录，Swift Package 结构
- `Info.plist` — IMKit 必需 key（`InputMethodConnectionName`, `ComponentSubtype`, etc.）
- `AppInputController.swift` — `IMKInputController` 子类
- `main.swift` — `IMKServer` 初始化 + Unix socket server
- `install.sh` 更新 — 自动安装到 `~/Library/Input Methods/`

**验证标准**：在系统"输入法"设置里能看到该 IME，切换后在 TextEdit 里 socket 能收到光标坐标。

---

### Phase 2 — Rust IPC 客户端 + 注入替换

**目标**：Tauri 侧用 IPC 替换现有 clipboard + ⌘V 注入路径。

**改动文件**：
- `src-tauri/src/input/injector.rs` — 新增 `inject_via_imk()` 函数，走 Unix socket
- `src-tauri/src/input/ipc_client.rs`（新建）— socket 连接池、序列化/反序列化
- `src-tauri/src/commands.rs` — 在 `inject_text` 前先尝试 IMK 路径，失败回退 clipboard

**逻辑**：
```rust
pub async fn inject_text(text: &str, ...) -> Result<()> {
    // 优先尝试 IMK 进程
    if let Ok(()) = inject_via_imk(text, None).await {
        return Ok(());
    }
    // 回退：clipboard + ⌘V（现有逻辑）
    inject_via_clipboard(text, target_pid).await
}
```

---

### Phase 3 — 上下文读取接入润色

**目标**：转录前先读光标上下文，注入润色 prompt。

**改动文件**：
- `src-tauri/src/transcription/polish.rs` — `PolishContext` 结构，接收 `before_cursor` / `app_name`
- `src-tauri/src/commands.rs` — 录音开始时异步查询上下文，转录完成后带入润色

**Prompt 变化**：
```
现有: "修正标点和错别字，保持原意"
新增: "当前应用: Mail，光标前文本: 'Hi John, regarding...'"
      "请匹配已有文本的语气、语言和格式"
```

---

### Phase 4 — 选中变换

**目标**：选中文字 + 录音键 → 说指令 → 直接替换。

**新增逻辑**：
- 录音触发时，查询 `selected_text`
- 若非空，进入"变换模式"：用户语音作为指令，selected_text 作为输入
- `insert` 请求携带 `replacement_range`

---

### Phase 5 — HUD 贴光标 + 流式插入

**目标**：HUD 出现在光标旁；转录结果流式"打出来"。

**改动**：
- HUD 窗口定位改为从 IPC 拿到的 `cursor_rect` 计算
- 转录 streaming 接入 IMK marked text 机制（Swift 侧实现 `setMarkedText`）

---

## 五、文件结构

```
audio-input/
├── imk-helper/                    ← 新建
│   ├── Package.swift
│   ├── Sources/
│   │   ├── main.swift             ← IMKServer + socket server
│   │   ├── AppInputController.swift  ← IMKInputController 子类
│   │   ├── SocketServer.swift     ← Unix domain socket
│   │   └── Protocol.swift        ← IPC 消息编解码
│   └── Info.plist                 ← IMKit 注册信息
├── src-tauri/src/input/
│   ├── injector.rs                ← 修改：新增 IMK 路径
│   ├── ipc_client.rs             ← 新建：Rust socket client
│   └── mod.rs
└── install.sh                     ← 更新：安装 IMK helper
```

---

## 六、不做的事（范围控制）

- 不做候选框 UI（IMK candidate window）— 我们不是 CJK 输入法
- 不做键盘拦截 / 热键重定义 — 保持现有 Tauri 全局快捷键
- 不强制要求用户切换输入法 — IMK helper 常驻但透明，热键仍由 Tauri 触发
- 不放弃 clipboard fallback — 保留作为兼容层

---

## 七、风险

| 风险 | 应对 |
|---|---|
| IMK helper 未激活时无法注入 | Tauri 启动时自动激活 IMK helper（`TISSelectInputSource`） |
| `attributedSubstringFromRange` 在某些 App 返回空 | 截图上下文作为补充（现有功能） |
| 沙盒限制 Unix socket 路径 | IMK helper 在 `~/Library/Input Methods/` 运行，不受 App 沙盒限制 |
| 分发时 IMK helper 签名 | 需与主 App 同一 Developer ID，打包流程更新 |
