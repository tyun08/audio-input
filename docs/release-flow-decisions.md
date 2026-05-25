# Release Flow — Architecture & Decisions

Companion doc to [`RELEASING.md`](../RELEASING.md). `RELEASING.md` tells you **what to do**; this doc tells you **why it's shaped this way** so future-you (or a contributor) doesn't redesign it by accident.

## Final architecture

```
human: open PR development → main                       (step 1 of 2)
  │
  ▼
.github/workflows/auto-bump-version.yml
  │  - computes next version (patch / minor / major from labels)
  │  - runs scripts/bump-version.sh (edits 3 files + Cargo.lock)
  │  - pushes "RELEASE X.Y.Z" commit to PR head (uses GITHUB_TOKEN)
  ▼
human: merge PR                                          (step 2 of 2)
  │
  ▼
push to main triggers .github/workflows/release.yml
  │  - job-level `if: startsWith(head_commit.message, 'RELEASE ')`
  │    → non-release commits skip instantly (no runner, no rebuild)
  │  - extract version from commit message (strict semver regex)
  │  - verify the 3 source files agree with the commit-msg version
  │  - create a draft GitHub release, target_commitish pinned to the
  │    RELEASE commit (so the eventual tag lands on the right commit
  │    even if main moves mid-build)
  │  - build + sign + notarize (macOS arm + x64, Windows MSI + NSIS)
  │  - upload latest.json (updater manifest)
  │  - publish the release → GitHub creates the vX.Y.Z tag at this point
  │  - update the homebrew tap
  ▼
done
```

Two human actions, zero terminal commands required after the PR opens.

## Trigger trade-offs (decision record)

| Option | Chosen? | Why / why not |
|---|---|---|
| Tag push triggers + human pushes tag from terminal | ❌ | One extra manual step after merge. The whole point was to remove manual steps. |
| Tag push triggers + CI auto-pushes tag | ❌ | A CI-pushed tag using `GITHUB_TOKEN` does **not** trigger other workflows (GitHub anti-recursion). Would require a PAT secret. Not worth the secret-management overhead for a single-maintainer project. |
| **Push to main triggers + commit-message gate** | ✅ | Fully automated, zero PAT needed. Cost: non-release pushes to main show a "Skipped" row in the Actions tab — no runner allocated, no compute spent, just one log row. |
| UI Release Tab (Draft a new release button) | ❌ | This is what broke v0.4.11. The UI creates a release object *and* pushes the tag; the tag push triggers `release.yml`, which creates a **second** release object for the same tag. Artifacts split across the two. Commit `ae8be71` added an explicit `gh release view` guard that refuses to run if a release already exists for the tag. |

## Concepts that confused us along the way

These are two **independent** issues that look related but aren't:

### Issue A — duplicate-release bug (v0.4.11)

- **Cause**: UI Release Tab creates a release object *before* `release.yml` runs. The workflow's `create-release` step doesn't know one already exists, so it creates a second one.
- **Has nothing to do with token type.** Even with a PAT, the duplicate would happen.
- **Fix**: don't use the UI; let only `release.yml` create releases. (Plus the `ae8be71` guard catches the case explicitly.)

### Issue B — `GITHUB_TOKEN` push doesn't trigger workflows

- **Cause**: GitHub's anti-recursion protection. If workflow A uses the default `secrets.GITHUB_TOKEN` to push a tag, that tag push **does not** trigger workflow B.
- **Has nothing to do with duplicate release.** It's purely about CI-to-CI chaining.
- **Workarounds when you need CI-pushed tags to trigger other workflows**:
  - Use a PAT (personal access token) instead of `GITHUB_TOKEN`
  - Use a GitHub App
  - Restructure so you don't need CI-to-CI chaining (← what we did)

### Who can push a tag and have it trigger `release.yml`?

| Pusher | Token / mechanism | Triggers workflows? |
|---|---|---|
| Human in terminal `git push origin v0.4.X` | personal credentials | ✅ |
| GitHub UI "Create release" / "Create tag" | GitHub's own infra | ✅ (but UI release path has issue A) |
| CI workflow using `secrets.GITHUB_TOKEN` | default token | ❌ |
| CI workflow using a PAT | personal access token | ✅ |
| CI workflow using a GitHub App | app installation token | ✅ |

In the chosen design, **nothing pushes a tag** — the tag is created by GitHub's REST API when the release is published. The trigger is the commit push, not a tag push, so these distinctions don't apply.

### Side effect of using `GITHUB_TOKEN` for the bump push

The `auto-bump-version` workflow uses `secrets.GITHUB_TOKEN` to push the `RELEASE X.Y.Z` commit to the PR's head branch. The same anti-recursion rule that blocks tag pushes from triggering workflows **also blocks commit pushes from triggering PR-level workflows** (tests, lint, typecheck on `pull_request: synchronize`). The diff is mechanical — a version-string edit and a Cargo.lock entry — so leaving it un-tested is acceptable today. **A future check that depends on the bump commit being CI-validated would silently miss it.** If that becomes a real concern, switch the auto-bump workflow to a PAT (or a GitHub App) and the synchronize event will fire normally.

## Known failure modes

### Squash-merge / merge-commit silently breaks the trigger

`release.yml`'s gate is `startsWith(github.event.head_commit.message, 'RELEASE ')`. "Squash and merge" sets the head commit subject to the PR title (default `Release X.Y.Z (#N)` — lowercase `Release`, parens around the PR number), and "Create a merge commit" sets it to `Merge pull request #N ...`. Both **silently** skip the workflow — no error, no release. **Mitigation: disable squash + merge-commit at the repo level** (Settings → General → Pull Requests), leaving only "Rebase and merge" available. `RELEASING.md` calls this out, but enforcement at the settings layer is the real fix.

### Two release PRs open at once

Both `auto-bump-version` runs compute `NEXT` against `origin/main`, so they produce the same target version. First PR to merge releases `0.4.12` cleanly. Second PR's head still carries `RELEASE 0.4.12`; merging it fires `release.yml`, which fails at the "Refuse if a release already exists" guard. The error message is accurate but not helpful for diagnosis. **Recovery**: on the still-open PR, push any new commit (or close/reopen) — `auto-bump-version` re-runs against the new `origin/main`, bumps to `0.4.13`, amends. Then merge as normal.

### Cargo.lock drift mid-build

`release.yml`'s "Verify source files match commit-message version" step checks `Cargo.toml` but **not** `Cargo.lock`. If a contributor manually bumped `Cargo.toml` without running `cargo update --workspace`, Cargo.lock's `audio-input` entry would be stale; `tauri-action`'s build would either silently update Cargo.lock (producing an undeclared file change in CI) or fail. `scripts/bump-version.sh` post-bump verification catches this for the auto-bump path; manual edits are still vulnerable.

## Key files

| File | Purpose |
|---|---|
| `scripts/bump-version.sh` | Edits `package.json`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, refreshes `Cargo.lock`. Single source of truth for "bump the version in N places." Exposed as `npm run release:bump X.Y.Z` for manual use. |
| `.github/workflows/auto-bump-version.yml` | Triggers on PR open / sync / label change targeting `main`. Pushes (or amends) a `RELEASE X.Y.Z` commit to the PR's head branch. |
| `.github/workflows/release.yml` | Triggers on push to `main`. Gated by `startsWith(head_commit.message, 'RELEASE ')`. Does the full build → publish → tap-update pipeline. |
| `RELEASING.md` | Operator-facing flow doc — what to do when shipping. |
| This file | Decision record — why the flow is shaped this way. |

## Daily-use cheatsheet

**Normal release**:

```bash
gh pr create --base main --head development --title "Release"
# auto-bump-version posts the RELEASE commit to the PR within ~1 min
# (default: patch bump; add label `release:minor` or `release:major` to override)
gh pr merge <pr-number> --rebase
# release.yml fires on the push to main; watch with `gh run watch`
```

**Manual / emergency release** (auto-bump workflow broken, network down, etc.):

```bash
git checkout main && git pull
git merge --ff-only development
npm run release:bump 0.4.X
git commit -am "RELEASE 0.4.X"
git push origin main          # release.yml fires the same way
```

Both paths produce the same `RELEASE X.Y.Z` commit on main; `release.yml` doesn't care which created it.

## Validation after PR #86 merges

Things to verify before relying on the new flow for a real release:

- Open a throwaway PR `development → main`; confirm `auto-bump-version` adds a `RELEASE X.Y.Z` commit within ~1 minute
- Add `release:minor` label, confirm the bump amends from patch to minor
- Add `release:major`, confirm the bump amends again
- Close the throwaway PR without merging
- For the next real release, watch the full pipeline; confirm `vX.Y.Z` tag points at the `RELEASE X.Y.Z` commit and the homebrew tap got the right SHAs

## Future upgrade paths

If the project outgrows the hand-rolled flow:

- **[release-please](https://github.com/googleapis/release-please-action)** (Google) — Reads conventional commits, opens a "release PR" with changelog + version bump. Used by Kubernetes, Node.js, Cloud Native projects. Needs a PAT or GitHub App.
- **[changesets](https://github.com/changesets/changesets)** — Contributors add changeset files to PRs; bot collects them into a "version PR." Used by Tauri itself, Astro, tRPC, Remix. Needs a PAT.
- **[semantic-release](https://github.com/semantic-release/semantic-release)** — Determines version + publishes on every push. Most aggressive automation. Heavy npm-ecosystem flavor.

Trigger to switch: contributor count grows / release cadence ≥ weekly / changelog automation becomes a chore.
