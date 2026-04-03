# macOS 打包分发踩坑经验

本文记录从 `npm run tauri dev` 能用、到打包的 `.app` 真正可用的完整排查过程。

---

## 问题一：CGEventPost 在打包 app 里静默失效

### 现象
- Dev 模式（`npm run tauri dev`）：文字自动注入目标输入框 ✓
- 打包的 `.app`：转录成功，但文字不出现，无任何报错

### 根因
打包 app 的 `AXIsProcessTrusted()` 始终返回 `false`，导致 `CGEventPost` 发出的 Cmd+V 被 macOS 静默丢弃。

但直接原因并非"没有授权 Accessibility"——即使在系统设置里打开了开关，仍然返回 `false`。

### 深层原因：linker-signed vs codesign

Tauri build 默认只做 **linker-signed**（编译器在链接阶段内嵌的签名），而不对整个 `.app` bundle 执行 `codesign`。

用 `codesign -dvvv` 检查打包的 app：

```
Identifier=audio_input-f46b4702279cd222   ← binary hash，每次 build 都变
Info.plist=not bound                       ← Info.plist 未绑定到签名
Sealed Resources=none                      ← 资源未封存
Signature=adhoc
```

macOS TCC（隐私权限数据库）用这个 **hash-based identifier** 来识别 app。每次重新 build，binary hash 变化，TCC 里的权限记录就失效——即便 System Settings 里还显示"已开启"。

相比之下，dev 模式的 binary 路径固定（`target/debug/audio-input`），hash 不变，所以 TCC 记录持续有效。

### 修复

在 `tauri.conf.json` 中加一行：

```json
"bundle": {
  "macOS": {
    "signingIdentity": "-"
  }
}
```

这让 Tauri 在打包时对整个 bundle 执行 `codesign --sign -`，结果变为：

```
Identifier=com.audioinput.app   ← 固定的 bundle ID
Info.plist=bound                ← Info.plist 已绑定
Sealed Resources=version=2      ← 资源已封存
```

TCC 现在用固定的 bundle ID 来识别 app，权限跨版本持续有效，不再需要每次 build 后重新授权。

### 验证已修复的打包 app，完整步骤

1. `npm run tauri build`
2. 将 `.app` 拷贝到 `/Applications`
3. 运行 app
4. 系统设置 → 隐私与安全性 → 辅助功能 → `+` → 选择 `/Applications/Audio Input.app` → 打开
5. 托盘 → 退出
6. 重新打开 app
7. 日志出现 `Accessibility 权限: 已授权 ✓`，文字注入正常工作

---

## 问题二：CGEvent 注入文字去了哪里？

### 现象
授权后 `AXIsProcessTrusted = true`，CGEvent 发出，但粘贴位置不对。

### 根因
录音开始时 `appWindow.show()` 将我们的浮窗推到前台，**抢走了目标 app 的键盘焦点**。发送 CGEvent Cmd+V 时，键盘焦点在我们自己的窗口上，而不是用户的输入框。

### 修复：NSApplicationActivationPolicyAccessory

在 `setup()` 里设置 macOS 激活策略为 Accessory：

```rust
#[cfg(target_os = "macos")]
{
    use objc::{class, msg_send, sel, sel_impl};
    unsafe {
        let ns_app: *mut objc::runtime::Object =
            msg_send![class!(NSApplication), sharedApplication];
        let _: () = msg_send![ns_app, setActivationPolicy: 1i64]; // Accessory = 1
    }
}
```

Accessory 策略的效果：
- App 不出现在 Dock
- **App 的窗口显示时不抢夺其他 app 的键盘焦点**
- 用户的目标输入框在整个录音→转录→注入过程中始终保持焦点

---

## 问题三：注入失败时用户毫无反馈

### 现象
`AXIsProcessTrusted = false` 时，`CGEventPost` 静默失败，`inject_text()` 返回 `Ok(())`，窗口隐藏，用户不知道发生了什么。

### 修复
在 `inject_text()` 里，检测到 AX 未授权时提前返回 `Err`：

```rust
if !check_accessibility_permission() {
    bail!("Accessibility 权限未授予 — 文字已写入剪贴板，请按 ⌘V 手动粘贴");
}
```

这样 `injection-failed` 事件会被触发，前端显示"已复制到剪贴板，请按 ⌘V"，用户至少可以手动粘贴。

---

## 经验总结

| 问题 | 错误排查方向 | 实际原因 |
|------|-------------|---------|
| AXIsProcessTrusted=false | 以为用户没授权 | linker-signed 导致 TCC 无法用 bundle ID 识别 app |
| 粘贴到错误位置 | 以为是 CGEvent 延迟问题 | 浮窗抢走了键盘焦点 |
| 注入失败无反馈 | 以为 CGEvent 有返回值 | CGEventPost 是 void，失败静默 |

**关键命令**：`codesign -dvvv "/Applications/YourApp.app"` — 遇到 Accessibility/TCC 问题，第一步就应该看 `Info.plist=not bound` 和 `Identifier` 字段。
