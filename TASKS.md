# 任务追踪

状态：`[ ]` 未开始 / `[~]` 进行中 / `[x]` 完成 / `[!]` 阻塞

---

## Phase 0 — 脚手架

- [ ] **0.1** 初始化 Tauri 2 项目（`npm create tauri-app`，模板 svelte-ts）
  - 验收：`cargo tauri dev` 启动无报错，窗口默认隐藏
- [ ] **0.2** 配置 macOS 麦克风权限（`Info.plist` + `NSMicrophoneUsageDescription`）
  - 验收：首次运行弹出系统麦克风授权对话框
- [ ] **0.3** 配置 `.env.example`，`dotenvy` 加载 `GROQ_API_KEY`

## Phase 1 — 系统托盘

- [ ] **1.1** 基础托盘图标 + 右键退出菜单（`tray.rs`）
  - 验收：菜单栏出现图标，右键退出可用
- [ ] **1.2** 托盘状态机（`state.rs`）：Idle / Recording / Processing / Error
  - 验收：状态切换后托盘图标相应变化，线程安全

## Phase 2 — 全局快捷键

- [ ] **2.1** 注册全局热键（默认 `Cmd+Shift+Space`）（`hotkey.rs`）
  - 验收：任意应用前台均可触发，退出时释放注册
- [ ] **2.2** 托盘左键单击调用同一 `toggle_recording` 函数
  - 验收：点击与快捷键行为完全一致

## Phase 3 — 麦克风录音

- [ ] **3.1** `recorder.rs`：`start()` / `stop()` → `Vec<f32>`（内存缓冲）
  - 验收：5秒录音内存 < 5MB，无权限时明确报错
- [ ] **3.2** `encoder.rs`：`Vec<f32>` → WAV `Vec<u8>`（16kHz，单声道，16-bit PCM）
  - 验收：编码结果可被 `hound::WavReader` 验证读取

## Phase 4 — Groq API

- [ ] **4.1** `groq.rs`：multipart POST，解析 `verbose_json` 响应
  - 验收：有效 WAV 返回文字，API key 无效时友好报错，超时 30s
- [ ] **4.2** API Key 管理：环境变量优先，缺失时托盘菜单提示配置

## Phase 5 — 文字注入（核心难点）

- [ ] **5.1** `injector.rs`：剪贴板写入 + 模拟 Cmd+V（`arboard` + `enigo`）
  - 验收：Safari / Chrome / VS Code / Terminal / Notes 均可正确粘贴中英文
- [ ] **5.2** macOS Accessibility 权限检测 + 引导开启
  - 验收：缺权限时弹提示并跳转系统设置
- [ ] **5.3** 剪贴板竞态保护：恢复前检查内容是否已被用户修改

## Phase 6 — 端到端串联

- [ ] **6.1** 主流程串联（`commands.rs`）+ 任意步骤失败状态回退
  - 验收：完整 录音→转录→注入 流程自动执行，失败后可重试
- [ ] **6.2** Svelte 状态浮窗：录音动画 / 加载圈 / 完成对勾（窗口不抢焦点）
  - 验收：状态变化 UI 响应 < 100ms，不抢占目标输入框焦点

## Phase 7 — 打包与分发

- [ ] **7.1** `cargo tauri build` 产出 `.dmg` + `.app`，正确签名
- [ ] **7.2** 开机自启选项（`tauri-plugin-autostart`），托盘菜单可切换

---

## 已确认设计决策

| 项目 | 决策 |
|------|------|
| 快捷键 | `Cmd+Shift+Space` |
| 录音模式 | Toggle |
| API Key 管理 | 应用内配置 UI |
| 状态浮窗 | 需要，简洁精美的录音 icon |
