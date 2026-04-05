# Audio Input

I love the voice input in ChatGPT — but I couldn't find a good standalone tool that works system-wide on macOS. So I built one in Rust.

Press a global hotkey, speak, and your words are transcribed and typed into whatever is focused. Works in every app. Free and open-source alternative to [SuperWhisper](https://superwhisper.com).

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

1. Get a free API key at [console.groq.com](https://console.groq.com) (no credit card required)
2. Right-click the menu bar mic icon → **Configure API Key**
3. Press `⌘⇧Space` anywhere and start talking

---

## Features

- **Global hotkey** — default `⌘⇧Space`, fully customizable
- **Works everywhere** — injects text into any focused input via Accessibility API
- **50+ languages** — Whisper large-v3-turbo auto-detects your language
- **AI polish** — optional LLM pass to clean up filler words and punctuation (toggle from menu bar)
- **Tiny footprint** — ~20 MB RAM, built with Rust + Tauri

---

## Cost

Powered by [Groq](https://groq.com)'s Whisper large-v3-turbo — the fastest Whisper inference available.

**$0.04 per hour of audio** (~$0.00067/minute).

For typical use — a few minutes of voice input per day — that's well under **$0.10/month**. The Groq free tier alone covers most personal use.

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

Tauri 2 · Rust (cpal, reqwest) · Svelte · Groq API (Whisper large-v3-turbo + LLM polish)

## License

MIT
