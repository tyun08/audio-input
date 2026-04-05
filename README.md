# Audio Input

A lightweight macOS menu bar app for voice input. Press a global hotkey, speak, and your words are transcribed and typed into whatever is focused — instantly.

Free, open-source alternative to [SuperWhisper](https://superwhisper.com).

[![Release](https://img.shields.io/github/v/release/tonyyun/audio-input)](https://github.com/tonyyun/audio-input/releases)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/macOS-Ventura%2B-lightgrey)](#)

---

## Install

```bash
brew install --cask tonyyun/tap/audio-input
```

Or grab the `.dmg` from [Releases](../../releases). First launch: right-click → Open to bypass Gatekeeper.

---

## Setup

1. Get a free API key at [console.groq.com](https://console.groq.com) (no credit card)
2. Right-click the menu bar mic icon → **Configure API Key**
3. Press `⌘⇧Space` anywhere and start talking

---

## Features

- **Global hotkey** — default `⌘⇧Space`, fully customizable
- **Works everywhere** — injects text into any focused input via Accessibility API
- **50+ languages** — Whisper large-v3-turbo auto-detects your language
- **AI polish** — optional LLM pass to clean up filler words and punctuation
- **Tiny footprint** — ~20 MB RAM, built with Rust + Tauri

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

```bash
# Prerequisites: Node 20+, Rust stable
npm install
npm run tauri dev   # dev
npm run tauri build # release
```

---

## Stack

Tauri 2 · Rust (cpal, reqwest) · Svelte · Groq API (Whisper + LLM)

## License

MIT
