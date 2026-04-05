# Homebrew Tap Setup Guide

This guide explains how to publish Audio Input via a Homebrew tap so users can install it with:

```
brew install --cask tonyyun/tap/audio-input
```

---

## One-Time Setup: Create the tap repository

1. Go to https://github.com/new and create a new **public** repository named exactly `homebrew-tap` under your account (`tonyyun`).
   - The repo must be public for `brew tap` to work without authentication.
   - No need to initialize with a README.

2. Clone the new repo locally:
   ```bash
   git clone https://github.com/tonyyun/homebrew-tap.git
   cd homebrew-tap
   ```

3. Create the `Casks/` directory and copy the formula into it:
   ```bash
   mkdir -p Casks
   cp /path/to/audio-input/Casks/audio-input.rb Casks/
   ```

4. Commit and push:
   ```bash
   git add Casks/audio-input.rb
   git commit -m "feat: add audio-input cask"
   git push origin main
   ```

Users can now add your tap:
```bash
brew tap tonyyun/tap
```

---

## At Release Time: Updating sha256 and version

Every time you publish a new GitHub Release, update the cask formula with the correct version and sha256 hashes.

### Step 1 — Download the DMGs from GitHub Releases

```bash
VERSION="0.2.0"  # replace with the new version

curl -L -O "https://github.com/tonyyun/audio-input/releases/download/v${VERSION}/Audio%20Input_${VERSION}_aarch64.dmg"
curl -L -O "https://github.com/tonyyun/audio-input/releases/download/v${VERSION}/Audio%20Input_${VERSION}_x64.dmg"
```

### Step 2 — Compute sha256 for each DMG

```bash
shasum -a 256 "Audio Input_${VERSION}_aarch64.dmg"
shasum -a 256 "Audio Input_${VERSION}_x64.dmg"
```

### Step 3 — Update the formula

In `Casks/audio-input.rb`:

1. Change `version "0.2.0"` to the new version string.
2. Replace the `PLACEHOLDER_SHA256` in the `on_arm` block with the hash from the `_aarch64.dmg`.
3. Replace the `PLACEHOLDER_SHA256` in the `on_intel` block with the hash from the `_x64.dmg`.

### Step 4 — Test locally before pushing

```bash
brew install --cask Casks/audio-input.rb
```

Or via the tap (after pushing):
```bash
brew update
brew install --cask tonyyun/tap/audio-input
```

### Step 5 — Commit and push to homebrew-tap

```bash
git add Casks/audio-input.rb
git commit -m "chore: bump audio-input to v${VERSION}"
git push origin main
```

---

## Testing the Tap

```bash
# Add the tap (only needed once per machine)
brew tap tonyyun/tap

# Install
brew install --cask tonyyun/tap/audio-input

# Upgrade to a new release after you've pushed the updated formula
brew update && brew upgrade --cask tonyyun/tap/audio-input

# Uninstall
brew uninstall --cask tonyyun/tap/audio-input
```

---

## Notes

- The tap repo **must remain public**. If you make it private, `brew tap` will require credentials.
- Homebrew caches formula files; run `brew update` to pick up formula changes.
- The `uninstall` block in the formula quits the app and removes `/Applications/Audio Input.app` cleanly.
- The `zap` block removes all user data (preferences, logs, support files) when the user runs `brew uninstall --zap`.
- If you ever publish a separate binary-only cask (e.g., `audio-input-bin`), uncomment the `conflicts_with` line in the formula to prevent both from being installed simultaneously.
