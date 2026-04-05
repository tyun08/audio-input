# Privacy

Audio Input is a local app with no backend of its own. Here is exactly what leaves your device and what doesn't.

---

## Audio

Your voice is recorded locally, then sent to the [Groq API](https://groq.com) (Whisper large-v3-turbo) for transcription. Audio is not stored by this app. Groq's data retention policy governs what Groq does with it: https://groq.com/privacy

## Screenshots

When AI polish is enabled, a screenshot is captured at the moment you start recording. It is sent to Groq's vision API (llama-4-scout) as context to improve transcription accuracy for technical terms and proper nouns. The screenshot is not saved to disk and is not stored by this app.

If AI polish is disabled, no screenshot is taken.

## Everything else

- No analytics or telemetry of any kind.
- No account, no email, no sign-up.
- App settings (shortcut, preferences) are stored locally in the OS app data directory.
- Your Groq API key is stored in the system keychain. It is only ever transmitted to `api.groq.com` — nowhere else.

---

## Planned: local inference

A future release will add a local inference mode (on-device Whisper + LLM). When enabled, no audio or screenshot data will leave your device at all.

---

## Questions

Open an issue or start a discussion on GitHub.
