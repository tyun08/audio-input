# Audio Input

I love the voice input in ChatGPT — but I couldn't find a good standalone tool that works system-wide on macOS. So I built one in Rust.

Press a global hotkey, speak, and your words are transcribed and typed into whatever is focused. Works in every app. Free and open-source alternative to [SuperWhisper](https://superwhisper.com).

[![Release](https://img.shields.io/github/v/release/tonyyun/audio-input)](https://github.com/tonyyun/audio-input/releases)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Windows-lightgrey)](#)

---

## Install

**macOS**
```bash
brew install --cask tonyyun/tap/audio-input
```
Or grab the `.dmg` from [Releases](../../releases). First launch: right-click → Open to bypass Gatekeeper.

**Windows**

Download `Audio.Input_x.x.x_x64-setup.exe` from [Releases](../../releases) and run it.

> First launch: Windows SmartScreen may say "Windows protected your PC". Click **More info → Run anyway**.

---

## Setup

1. Get a free API key at [console.groq.com](https://console.groq.com) (no credit card required)
2. Right-click the system tray mic icon → **Configure API Key**
3. Press `Ctrl+Shift+Space` (Windows) or `⌘⇧Space` (macOS) anywhere and start talking

---

## Features

- **Global hotkey** — default `⌘⇧Space`, fully customizable
- **Works everywhere** — injects text into any focused input via Accessibility API
- **50+ languages** — Whisper large-v3-turbo auto-detects your language
- **AI polish** — optional LLM pass to clean up filler words and punctuation (toggle from menu bar). At recording start, a screenshot is taken and sent as context to a vision LLM (llama-4-scout on Groq) to improve accuracy of technical and domain-specific terms.
- **Tiny footprint** — ~20 MB RAM, built with Rust + Tauri

---

## Cost

Powered by [Groq](https://groq.com)'s Whisper large-v3-turbo — the fastest Whisper inference available.

**$0.04 per hour of audio** (~$0.00067/minute).

For typical use — a few minutes of voice input per day — that's well under **$0.10/month**. The Groq free tier alone covers most personal use.

---

## How It Works

1. Press the global hotkey — a screenshot of the active screen is captured immediately
2. Speak; audio is recorded locally while you hold (or toggle) the hotkey
3. Audio is sent to Groq's Whisper large-v3-turbo for transcription
4. If AI polish is enabled, the transcript + screenshot are sent to a vision LLM (llama-4-scout) to fix technical terms, proper nouns, and punctuation
5. The final text is injected into whatever input is focused via the Accessibility API

---

## Privacy

Audio is sent to [Groq](https://groq.com) for transcription — Groq's data retention policy applies. Screenshots are taken locally and sent to Groq's vision API only when AI polish is enabled; neither audio nor screenshots are stored by this app. No analytics, no telemetry, no account required. See [PRIVACY.md](PRIVACY.md) for full details.

---

## Menu bar states

| Icon | State |
|------|-------|
| Black mic | Idle |
| Red mic | Recording |
| Blue mic | Transcribing |
| Orange mic | Error |

---

## Build from source

### macOS

**Prerequisites:** Node 20+, Rust stable

```bash
# Install Rust if needed
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

git clone https://github.com/tonyyun/audio-input
cd audio-input
npm install
npm run tauri dev    # dev mode
npm run tauri build  # release build → produces .dmg + .app in src-tauri/target/release/bundle/
```

### Windows

**Prerequisites:**

1. **Node.js 20+** — https://nodejs.org (LTS)
2. **Rust** — https://rustup.rs
3. **Microsoft C++ Build Tools** — https://visualstudio.microsoft.com/visual-cpp-build-tools/
   - In the installer select **"Desktop development with C++"**
4. **WebView2 Runtime** — pre-installed on Windows 11; on Windows 10 get it from https://developer.microsoft.com/microsoft-edge/webview2/

```powershell
git clone https://github.com/tonyyun/audio-input
cd audio-input
npm install
npm run tauri dev
```

---

## Stack

Tauri 2 · Rust (cpal, reqwest) · Svelte · Groq API (Whisper large-v3-turbo + LLM polish)

## License

MIT
