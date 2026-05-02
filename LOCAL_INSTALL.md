# Local Installation Guide

This guide explains how to install Audio Input on macOS from a release DMG or from a local build.

## Option 1: Install From the Latest Release

1. Open the latest GitHub release:
   <https://github.com/tyun08/audio-input/releases/latest>
2. Download the latest `.dmg` file.
3. Open the DMG.
4. Install the app by either:
   - Dragging `Audio Input.app` into `/Applications`, or
   - Running the install script from this repository:

```bash
bash install.sh
```

If macOS blocks the first launch because the app is not published with an Apple Developer certificate, right-click `Audio Input.app` and choose **Open**.

## Option 2: Build Locally

Prerequisites:

- Node.js 20+
- Rust stable
- npm dependencies installed with `npm install`

Build the macOS app and DMG:

```bash
npm run tauri build
```

The build outputs the app bundle and DMG under:

```text
src-tauri/target/release/bundle/
```

Open the generated DMG, then install `Audio Input.app` into `/Applications`. You can drag it manually, or run:

```bash
bash install.sh
```

## Required Permission Fixes

After the app is installed into `/Applications`, run these scripts from the repository root:

```bash
bash install.sh
bash fix-permissions.sh
sudo bash fix-accessibility-permissions.sh
```

What they do:

- `install.sh` copies `Audio Input.app` into `/Applications`. This is equivalent to manually dragging the app into `/Applications`, and it also attempts to update microphone permission metadata.
- `fix-permissions.sh` repairs the current user's microphone permission registration. This is needed because the app is not distributed with an Apple Developer-published signature.
- `fix-accessibility-permissions.sh` repairs or opens the macOS Accessibility permission flow. This permission is required so Audio Input can paste transcribed text into the active app through the clipboard/input automation path.

If macOS System Settings opens during the accessibility step, manually enable Audio Input under:

```text
System Settings -> Privacy & Security -> Accessibility
```

Restart Audio Input after changing microphone or accessibility permissions.

## First Run

1. Open Audio Input.
2. Choose your preferred API provider.
3. Enter your API key.
4. Press `Command + Shift + Space` to start recording.
5. Press `Command + Shift + Space` again to stop recording and insert the transcription into the active text field.

After this setup, Audio Input is ready for normal use.
