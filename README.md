# Audio Input

A lightweight, menu-bar / system-tray voice-to-text app for macOS and Windows.

Press a global shortcut, speak, and your words are transcribed and typed at the cursor — in any app.

## Features

- **Global shortcut recording** — hold Cmd+Shift+Space (macOS) or Ctrl+Shift+Space (Windows) to record, release to transcribe.
- **AI-powered transcription** — supports Groq Whisper (free tier) and Google Vertex AI Gemini.
- **AI Polish** — optional LLM cleanup for punctuation, spelling, and context-aware corrections.
- **Screenshot context** — captures your screen while recording for smarter polish (off by default).
- **Auto-inject text** — types directly at the cursor via clipboard + simulated paste (requires Accessibility permission on macOS).
- **macOS Services integration** — right-click selected text → Services → "Polish with Audio Input".
- **System tray** — always accessible; shows recording state, last result, and quick settings.
- **Onboarding flow** — guides first-time users through API key setup and permissions.
- **i18n** — English and Chinese (中文) UI.
- **Autostart** — optional launch at login.
- **Custom microphone** — pick which input device to use.
- **Vocabulary injection** — custom word list for better Whisper accuracy on names/terms.

## Quick Start

### Prerequisites

- **macOS** 13+ (Ventura) or **Windows** 10+
- [Rust](https://rustup.rs/) (stable)
- [Node.js](https://nodejs.org/) 18+
- A Groq API key (free at [console.groq.com](https://console.groq.com))

### Build from Source

```bash
git clone https://github.com/tyun08/audio-input.git
cd audio-input
npm install
npm run tauri dev          # development mode
npm run tauri build        # production build
```

### Configuration

1. Launch the app — the onboarding flow will guide you.
2. Or click the tray icon → Settings to configure:
   - **Voice Service**: Groq (API key) or Vertex AI (gcloud ADC)
   - **Global Shortcut**: customizable key combo
   - **AI Polish**: toggle on/off
   - **Microphone**: select input device
   - **Screenshot Context**: enable for visual context in polish

## Architecture

```
audio-input/
├── src/                    # Svelte frontend
│   ├── App.svelte          # Main app shell (HUD ↔ Settings ↔ Onboarding)
│   ├── lib/
│   │   ├── SettingsPanel.svelte    # Settings UI
│   │   ├── RecordingIndicator.svelte # Recording HUD
│   │   ├── OnboardingFlow.svelte   # First-run wizard
│   │   ├── providers.ts    # Provider registry (add new providers here)
│   │   └── i18n.ts         # Internationalization
│   └── main.ts
├── src-tauri/              # Rust backend
│   ├── src/
│   │   ├── lib.rs          # App setup, plugin registration, shortcut binding
│   │   ├── commands.rs     # Tauri IPC command handlers
│   │   ├── config.rs       # App configuration (persisted JSON)
│   │   ├── state.rs        # Shared app state
│   │   ├── audio/
│   │   │   ├── recorder.rs # Microphone capture via cpal
│   │   │   └── encoder.rs  # WAV encoding + resampling
│   │   ├── transcription/
│   │   │   ├── groq.rs     # Groq Whisper API client
│   │   │   ├── vertex.rs   # Vertex AI Gemini client
│   │   │   └── polish.rs   # LLM polish (Groq chat API)
│   │   ├── input/
│   │   │   └── injector.rs # Clipboard + simulated paste
│   │   ├── tray.rs         # System tray + menu
│   │   ├── shortcut.rs     # Global shortcut parsing
│   │   ├── macos_shortcut.rs  # CGEventTap (HID-level, overrides other apps)
│   │   ├── macos_service.rs   # macOS Services provider
│   │   └── screenshot.rs   # Screen capture for context
│   └── Cargo.toml
└── Casks/                  # Homebrew Cask formula (for updates)
```

### Adding a New Provider

1. Add an entry to `src/lib/providers.ts` (frontend metadata + fields).
2. Create `src-tauri/src/transcription/<id>.rs` (transcribe + polish).
3. Add a match arm in `commands.rs` → `transcribe_with_provider` / `polish_with_provider`.

No new IPC commands or config schema changes needed.

## Distribution

### macOS (Homebrew)

```bash
brew install --cask tyun08/tap/audio-input
```

### Manual

Download the `.dmg` or `.msi` from [Releases](https://github.com/tyun08/audio-input/releases).

## License

MIT — see [LICENSE](LICENSE).
