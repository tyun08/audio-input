# Apple Developer 签名与公证指南

## 背景：为什么现在需要手动 run 脚本？

当前 `tauri.conf.json` 里 `signingIdentity` 是 `"-"`，这是 **ad-hoc 签名**（临时签名）。  
macOS TCC 系统对 ad-hoc 签名的 app 不信任，导致：

| 权限 | 当前状态 | 原因 |
|------|---------|------|
| 麦克风 | 需要手动跑 `fix-permissions.sh` | ad-hoc 签名的 app 无法触发系统权限弹窗 |
| 辅助功能 | 需要手动跑 `fix-accessibility-permissions.sh` | 同上，且每次重新构建 cdhash 变化导致 TCC 记录失效 |

**有了 Developer ID + 公证之后：**
- 麦克风：系统自动弹窗请求，用户点允许即可 ✅
- 辅助功能：用户仍需手动在 System Settings 里授权（这是 Apple 的设计），但 app 可以主动引导，且授权后跨版本更新保持稳定 ✅

---

## 第一步：注册 Apple Developer Program

1. 访问 [developer.apple.com/programs/enroll](https://developer.apple.com/programs/enroll/)
2. 用你的 Apple ID 登录，选择 **Individual**（个人开发者，$99/年）
3. 等待 24-48 小时审核通过（通常很快）

---

## 第二步：创建 Developer ID Application 证书

> 这是用于在 Mac App Store 以外分发 app 的证书类型。

### 方法 A：通过 Xcode（推荐）

1. 打开 Xcode → `Settings` → `Accounts`
2. 添加你的 Apple ID
3. 点击 `Manage Certificates` → `+` → 选 `Developer ID Application`
4. 证书会自动安装到 Keychain

### 方法 B：通过开发者后台

1. 去 [developer.apple.com/account/resources/certificates](https://developer.apple.com/account/resources/certificates/list)
2. 点 `+` → 选 `Developer ID Application`
3. 按照引导创建 CSR（Certificate Signing Request），下载并双击安装

### 验证安装成功

```bash
security find-identity -v -p codesigning
```

输出应该包含类似：
```
1) ABCDEF1234... "Developer ID Application: Tony Yun (XXXXXXXXXX)"
```

记下 `XXXXXXXXXX`，这是你的 **Team ID**。

---

## 第三步：配置公证（Notarization）凭据

公证需要向 Apple 服务器提交二进制文件，需要提供认证信息。**推荐用 API Key 方式**（比 App-specific password 更稳定，适合 CI）。

### 创建 App Store Connect API Key

1. 访问 [appstoreconnect.apple.com/access/api](https://appstoreconnect.apple.com/access/api)
2. 点 `Generate API Key`，角色选 `Developer`
3. 下载 `.p8` 文件（**只能下载一次，务必保存好**）
4. 记下 **Key ID** 和 **Issuer ID**

### 保存凭据到 Keychain（本地构建用）

```bash
xcrun notarytool store-credentials "notarytool-profile" \
  --key "/path/to/AuthKey_XXXXXXXXXX.p8" \
  --key-id "XXXXXXXXXX" \
  --issuer "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx"
```

验证：
```bash
xcrun notarytool history --keychain-profile "notarytool-profile"
```

---

## 第四步：修改 tauri.conf.json

把 `signingIdentity` 从 ad-hoc 改为你的 Developer ID：

```json
"macOS": {
  "infoPlist": "Info.plist",
  "entitlements": "entitlements.plist",
  "signingIdentity": "Developer ID Application: Tony Yun (XXXXXXXXXX)"
}
```

> 把 `Tony Yun (XXXXXXXXXX)` 换成第二步 `security find-identity` 命令输出里的实际字符串。

---

## 第五步：更新 entitlements.plist（重要）

当前的 entitlements 有两个问题需要注意：

**`com.apple.security.cs.allow-unsigned-executable-memory`** — 这个 entitlement 在 hardened runtime 下是一个豁免项，Apple 公证时会接受，但会触发额外审查。如果 app 不需要（比如 Whisper 模型不需要 JIT），可以考虑移除。先保留，看公证能否通过。

**辅助功能（Accessibility）没有对应 entitlement** — 这是正常的。Apple 不提供 entitlement 来自动授予辅助功能权限，必须用户手动授权。App 需要在代码里调用 `AXIsProcessTrusted()` 来检测并引导用户。（详见第七步）

---

## 第六步：配置 GitHub Actions CI/CD 自动签名公证

### 添加 GitHub Secrets

去仓库 `Settings → Secrets and variables → Actions`，添加以下 secrets：

| Secret 名 | 内容 |
|-----------|------|
| `APPLE_SIGNING_IDENTITY` | `Developer ID Application: Tony Yun (XXXXXXXXXX)` |
| `APPLE_CERTIFICATE` | 导出的 .p12 证书（Base64 编码，见下方） |
| `APPLE_CERTIFICATE_PASSWORD` | .p12 导出时设置的密码 |
| `APPLE_API_KEY` | .p8 文件内容（Base64 编码） |
| `APPLE_API_KEY_ID` | API Key ID |
| `APPLE_API_ISSUER` | Issuer ID |

### 导出证书为 Base64

```bash
# 从 Keychain 导出 .p12（Keychain Access → 右键证书 → Export）
# 然后 Base64 编码：
base64 -i Certificates.p12 | pbcopy  # 自动复制到剪贴板
```

### 更新 release.yml 的 build-macos job

在 `Build and upload to release` 步骤前加上签名配置：

```yaml
- name: Import certificate
  env:
    APPLE_CERTIFICATE: ${{ secrets.APPLE_CERTIFICATE }}
    APPLE_CERTIFICATE_PASSWORD: ${{ secrets.APPLE_CERTIFICATE_PASSWORD }}
  run: |
    echo "$APPLE_CERTIFICATE" | base64 --decode > certificate.p12
    security create-keychain -p "" build.keychain
    security default-keychain -s build.keychain
    security unlock-keychain -p "" build.keychain
    security import certificate.p12 -k build.keychain \
      -P "$APPLE_CERTIFICATE_PASSWORD" -T /usr/bin/codesign
    security set-key-partition-list -S apple-tool:,apple: -s \
      -k "" build.keychain

- name: Write API key
  env:
    APPLE_API_KEY: ${{ secrets.APPLE_API_KEY }}
    APPLE_API_KEY_ID: ${{ secrets.APPLE_API_KEY_ID }}
  run: |
    mkdir -p ~/.private_keys
    echo "$APPLE_API_KEY" | base64 --decode \
      > ~/.private_keys/AuthKey_${{ secrets.APPLE_API_KEY_ID }}.p8

- name: Build and upload to release
  uses: tauri-apps/tauri-action@v0
  env:
    GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
    APPLE_SIGNING_IDENTITY: ${{ secrets.APPLE_SIGNING_IDENTITY }}
    APPLE_API_KEY: ${{ secrets.APPLE_API_KEY_ID }}
    APPLE_API_ISSUER: ${{ secrets.APPLE_API_ISSUER }}
    APPLE_API_KEY_PATH: ~/.private_keys/AuthKey_${{ secrets.APPLE_API_KEY_ID }}.p8
  with:
    releaseId: ${{ needs.create-release.outputs.release-id }}
    args: ${{ matrix.args }}
```

> `tauri-apps/tauri-action` 会自动调用 `xcrun notarytool` 做公证，只需提供上面的环境变量。

---

## 第七步：App 内引导 Accessibility 权限（代码改动）

辅助功能权限永远需要用户手动授权，但 app 可以主动检测并打开 System Settings：

在 Rust 端（`src-tauri/src/`）调用 macOS API：

```rust
// 检查辅助功能权限
#[cfg(target_os = "macos")]
fn check_accessibility() -> bool {
    use std::process::Command;
    // AXIsProcessTrusted — 如果没有权限，macOS 会自动弹提示引导用户
    // 或者直接打开 System Settings
    Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
        .spawn()
        .is_ok()
}
```

或者用 `accessibility` crate，调用 `AXIsProcessTrustedWithOptions` 并传入 `kAXTrustedCheckOptionPrompt = true`，让 macOS 自动弹出授权提示。

**这样用户第一次运行时会看到系统级的引导，不再需要手动跑脚本。**

---

## 完成后用户体验

| 场景 | 现在 | 有了 Developer ID + 公证后 |
|------|------|--------------------------|
| 安装 app | 需要跑 fix 脚本 | 直接双击 DMG 安装 |
| 麦克风权限 | 需要跑 fix 脚本 | 第一次使用时系统自动弹窗 |
| 辅助功能权限 | 需要跑 fix 脚本 | App 启动时引导用户去 System Settings 手动点一次 |
| 更新 app 后权限是否保留 | 可能丢失（cdhash 变化） | 稳定保留（Developer ID 不变） |
| Gatekeeper 警告 | 有（需要右键→打开绕过） | 无（公证通过） |

---

## 快速检查清单

- [ ] 注册 Apple Developer Program，等待审核
- [ ] 创建 Developer ID Application 证书，安装到 Keychain
- [ ] 创建 App Store Connect API Key，保存 .p8 文件
- [ ] 本地测试：修改 `tauri.conf.json` signingIdentity，`npm run tauri build`，验证签名和公证
- [ ] 配置 GitHub Secrets（6 个）
- [ ] 更新 `.github/workflows/release.yml`
- [ ] 在 app 代码里添加 Accessibility 权限引导逻辑
- [ ] 打 tag 测试完整 CI/CD 流程
- [ ] 删除 `fix-permissions.sh` 和 `fix-accessibility-permissions.sh`
