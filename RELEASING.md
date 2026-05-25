# Releasing

How code lands in users' hands. **Read this before tagging anything.**

## Branches

- `main` — protected, always shippable. Tags on `main` produce public releases.
- `development` — daily work branch. Short-lived fixes / features that can ship in the next routine release. Should stay close to `main`; merge to `main` whenever a release is cut.
- `feat/<short-name>` or `fix/<short-name>` — per-feature branches off `development`. Merge back into `development` when done.
- `develop-<topic>` — **long-lived parallel feature branches** for work that will span many weeks and shouldn't block `development`'s rapid integration loop. Currently active:
  - **`develop-imk`** — InputMethodKit integration (see [`docs/IMK_PLAN.md`](docs/IMK_PLAN.md)). Rebase onto `main` periodically to absorb shipped fixes; merge to `main` only when a phase is complete + smoke-tested via the test-release workflow.

The "tag from main, develop on development, isolate big features on develop-*" pattern keeps `main`'s history a list of shipped versions, keeps half-broken WIP off the public release timeline, and stops multi-week features from sitting on `development` and blocking small fixes from shipping.

## Day-to-day

```bash
git checkout development
git pull
# ...edit, commit...
git push origin development
```

Push to `development` triggers the regular `Tests` workflow (unit tests, lint, integration tests). It does **not** create a release.

## Long-lived feature branches (`develop-*`)

Use a `develop-<topic>` branch when:

- The feature will take more than a release cycle to finish
- Landing it half-done on `development` would block routine fixes from shipping
- The work has its own test suite / infra (e.g. a separate Swift package, a new daemon)

Example: `develop-imk` carries the InputMethodKit integration. It's branched off `main`, has its own Swift package under `imk-helper/`, and accumulates Phase 2 → Phase 5 of `docs/IMK_PLAN.md` independently of whatever's shipping out of `development`.

**Per-week maintenance on a `develop-*` branch:**

```bash
git checkout develop-imk
git fetch origin
git rebase origin/main      # absorb any shipped fixes; git auto-skips
                            # commits that already landed via cherry-pick
git push --force-with-lease origin develop-imk
```

**Shipping a phase from a `develop-*` branch:**

1. Smoke-test via the test-release workflow (see next section). Pick the `develop-imk` branch in the "Run workflow" dialog.
2. Once green, rebase one more time onto current `main`.
3. Fast-forward `main`:
   ```bash
   git checkout main
   git merge --ff-only develop-imk   # or merge -X theirs if convergent commits exist
   ```
4. Bump version + tag like any other release (see "Shipping a real release").
5. The `develop-*` branch stays alive — Phase N+1 picks up from `main`'s new tip.

## Shipping a beta / RC

Use this when you want testers to download a real signed build before it lands on `main`.

```bash
# 1. Bump version in all three files to the TARGET version (e.g. 0.4.11)
#    package.json  ·  src-tauri/Cargo.toml  ·  src-tauri/tauri.conf.json
( cd src-tauri && cargo build --quiet )   # refresh Cargo.lock
git add package.json src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/tauri.conf.json
git commit -m "chore: bump version to 0.4.11 for beta"
git push

# 2. Tag with the pre-release suffix and push
git tag v0.4.11-beta.1
git push origin v0.4.11-beta.1
```

The `beta-release.yml` workflow fires and:
- Builds + signs + notarizes macOS arm/x86_64 + Windows
- Creates a **persistent** GitHub Prerelease at `v0.4.11-beta.1`
- Uploads `latest-beta.json` to the release (for a future beta-channel updater)
- Does **not** update the Homebrew tap

For a follow-up beta, bump just the suffix: `v0.4.11-beta.2`. For a release candidate, use `v0.4.11-rc.1`.

When the beta is good, ship normally from `main` (see "Shipping a real release"). The `v0.4.11-beta.*` prerelease stays visible on the Releases page so testers can see the changelog history.

## Verifying a build before release

When you've staged changes that touch the release pipeline (workflow YAML, entitlements, bundle config, signing, updater, etc.), don't just tag a real version — smoke-test first.

1. Push your work to `development`.
2. Actions tab → "**Test Release (manual)**" → "Run workflow" → pick `development`.
3. Workflow creates a draft prerelease tagged `testbuild-development-<sha>-<run#>`, builds + signs + notarizes macOS arm/x86_64 + Windows MSI/NSIS, uploads to the prerelease.
4. Verify:
   - All matrix jobs green
   - Release assets show `*.dmg`, `*.msi`, `*-setup.exe`, `*.app.tar.gz`, all the `.sig` files
   - If you toggled `keep_release=true`, download a DMG and install it locally to verify the app actually opens and signed correctly. Otherwise the prerelease is auto-deleted at the end of the run.
5. If anything is wrong, fix it on `development` and re-run. Iterate until clean.

Test-release runs **never** touch the Homebrew tap and **never** publish to GitHub's public release feed (the prerelease is draft + prerelease-flagged, and the cleanup job deletes it).

## Shipping a real release

> ⚠️ **NEVER use the GitHub web UI's "Draft a new release" button.** It
> creates the release object before the workflow runs, then the workflow's
> `create-release` step makes a SECOND release for the same tag (GitHub
> allows it). Artifacts split between the two; the homebrew tap update
> looks at the wrong one. This is exactly how v0.4.11 broke. The workflow
> now fails fast if a release already exists for the tag — don't bypass it.

Once `development` is verified:

```bash
# 1. Merge to main
git checkout main
git pull
git merge --ff-only development   # fast-forward only — no merge commits on main
git push origin main

# 2. Bump version in 3 files: package.json, src-tauri/Cargo.toml, src-tauri/tauri.conf.json
# Make sure all three match. Then refresh Cargo.lock:
( cd src-tauri && cargo build --quiet )

# 3. Commit + tag + push
git add package.json src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/tauri.conf.json
git commit -m "RELEASE 0.4.X"
git push origin main
git tag -a v0.4.X -m "Release 0.4.X"
git push origin v0.4.X

# 4. CRITICAL: sync development back to main so the next round of dev
#    work doesn't diverge. Without this, your next merge --ff-only will
#    fail because main has the RELEASE commit that development lacks.
git checkout development
git merge --ff-only main
git push origin development
```

The push of `v0.4.X` triggers `release.yml`, which:

1. Creates a draft GitHub release
2. Builds + signs + notarizes macOS arm/x86_64 (Developer ID: Jingtao Yun / V8RQ99X6H4)
3. Builds + bundles Windows MSI + NSIS (WiX installed via choco)
4. Generates `latest.json` (updater manifest) and uploads to the release
5. Publishes the release (lifts the draft flag)
6. Updates the Homebrew tap (`tyun08/homebrew-tap` → `Casks/audio-input.rb`) with new URLs + SHA256s

Pipeline takes ~15 min. Watch via `gh run watch <id>`.

## Tag pattern is strict

`release.yml` only fires for tags matching `v[0-9]+.[0-9]+.[0-9]+` (strict semver, three parts, no suffix). So `v0.4.9-beta`, `v0.4.9-rc1`, `testbuild-anything` all do **not** trigger the production pipeline. That's the safety net — accidentally tagged WIP won't ship.

## If the pipeline fails mid-release

What's already happened by the time a failure surfaces:
- `create-release` succeeded → there's a draft release for the tag
- `build-macos` / `build-windows` may have uploaded some artifacts
- `update-cask` not yet → Homebrew tap NOT updated

Clean up:
```bash
# Delete the draft release + tag
gh release delete v0.4.X --yes --cleanup-tag

# Or, if the tag was on the wrong commit, delete + retag
git push origin :refs/tags/v0.4.X
git tag -d v0.4.X
git tag -a v0.4.X -m "Release 0.4.X"   # ON THE RIGHT COMMIT
git push origin v0.4.X
```

If `update-cask` runs but with a bad sha (e.g., a sed bug in the workflow), patch the cask manually in `tyun08/homebrew-tap` to unblock brew users while you fix the workflow.

## Secrets the workflows need

In repo settings → Secrets and variables → Actions:

| Secret | Purpose |
|---|---|
| `APPLE_CERTIFICATE` | base64 of the single-identity `Developer ID Application` P12 |
| `APPLE_CERTIFICATE_PASSWORD` | password set when exporting the P12 |
| `KEYCHAIN_PASSWORD` | random string; locks the temp keychain in CI |
| `APPLE_ID` | jtcloud2008@outlook.com |
| `APPLE_PASSWORD` | app-specific password for notarytool |
| `APPLE_TEAM_ID` | V8RQ99X6H4 |
| `TAURI_SIGNING_PRIVATE_KEY` | minisign private key for the in-app updater |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | password set when generating the minisign keypair |
| `TAP_TOKEN` | PAT with `repo` scope on `tyun08/homebrew-tap` |

If a secret is rotated, update it in both Repository secrets AND the local backup (Passwords app / encrypted DMG). Losing `TAURI_SIGNING_PRIVATE_KEY` is the worst case — every installed app would lose the ability to auto-update and users would have to download a new DMG manually.

## After shipping

Sanity-check the release before walking away:

```bash
# Assets all present
gh release view v0.4.X --json assets --jq '.assets[].name' | sort

# latest.json platforms not empty
curl -sL https://github.com/tyun08/audio-input/releases/latest/download/latest.json | jq .platforms

# Homebrew tap arm + intel SHA both match the uploaded DMGs
brew info --cask tyun08/tap/audio-input
```

Then `brew upgrade --cask audio-input` on a Mac and click the tray "Check for Updates…" on an already-installed older version to verify the in-app updater picks up the new release.
