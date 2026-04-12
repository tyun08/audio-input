# Changelog

## 0.4.0 (2026-04-12)

### Release Prep
- Aligned version numbers across package.json, Cargo.toml, and tauri.conf.json.
- Added comprehensive README with architecture docs and provider extension guide.
- Added MIT LICENSE.
- Added GitHub Actions CI/CD for automated macOS (ARM + Intel) and Windows builds.
- Fixed all Rust compiler warnings in project code.
- Added .cargo/config.toml for consistent macOS deployment target.
- Updated Homebrew Cask formula for version 0.4.0.
- Improved .gitignore.

## 0.3.x (Earlier)

- Groq Whisper and Vertex AI Gemini transcription providers.
- AI Polish with vision model support and screenshot context.
- macOS CGEventTap for HID-level shortcut interception.
- macOS Services provider for right-click text polish.
- System tray with recording state, settings, and last result.
- Onboarding flow, i18n (EN/ZH), autostart, custom mic selection.
- Vocabulary injection for Whisper accuracy.
