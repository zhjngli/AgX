# Release Process

This document describes how to ship a release of `agx-photo` or `agx-cli` to crates.io. The version bump and changelog reach `main` through a pull request (`main` is branch-protected — see [Branch protection](#branch-protection-releases-go-through-a-pr)); tagging and the crates.io publish then run from the maintainer's laptop. Per-crate independent versioning: each publishable crate has its own version timeline.

The high-level summary lives in [`developer-workflow.md`](developer-workflow.md) Step 6. This document has the operational detail.

## Branch protection: releases go through a PR

`main` is governed by a ruleset that requires a pull request for every change and has **no bypass actors** — not even repo admins can push to `main` directly. This is deliberate: the maintainer works alongside coding agents that act under the maintainer's identity, and a bypass would let an agent push unreviewed code to `main`. So the version-bump + changelog commits a release produces **cannot be pushed straight to `main`** the way a vanilla `cargo release --execute` expects.

Releases therefore split into two halves:

1. **File edits via a PR.** The version bump, `CHANGELOG.md` stamp, and (for `agx-photo`) the `agx-cli` dep-pin bump are produced on a short-lived release branch — using `cargo-release` purely for the edits, with publishing, tagging, and pushing disabled — then merged to `main` through a normal PR. Required checks must pass, and the ruleset requires linear history, so squash- or rebase-merge.
2. **Tag + publish from merged `main`.** After the PR merges, tag the merged commit and run `cargo publish` locally. Tags are not covered by the branch ruleset, so `git push origin <tag>` works; and `cargo publish` talks to crates.io directly, independent of any git push.

The per-step recipe is in [Release steps](#release-steps-single-crate) below.

> If a future maintainer adds a bypass actor to the ruleset (e.g. a release service account that agents can't impersonate), the original all-in-one `cargo release --execute` flow — bump, tag, publish, and push to `main` in one shot — becomes viable again. The split flow below assumes no bypass.

## When to release

**Soft trigger:** after notable PR merges, dry-run the changelog scaffold for whichever crate the change touched. If meaningful entries appear, ship within a week.

```bash
# For agx-photo:
git cliff --include-path 'crates/agx/**' --tag-pattern 'agx-photo-v.*' --unreleased

# For agx-cli:
git cliff --include-path 'crates/agx-cli/**' --tag-pattern 'agx-cli-v.*' --unreleased
```

The two invocations use different tokens for path (`crates/agx/**` vs `crates/agx-cli/**`) and tag pattern (`agx-photo-v.*` vs `agx-cli-v.*`) — `agx-photo`'s directory is `crates/agx/`, but the package name (and tag prefix) is `agx-photo`. Don't conflate them.

The dry-run is the source of truth. The on-disk `[Unreleased]` section in each `CHANGELOG.md` is normally empty between releases — entries are scaffolded and curated at release time, not appended per-PR. Don't use the on-disk `[Unreleased]` as the trigger; use the dry-run above.

There's no fixed schedule.

**Independent versions:** `agx-photo` and `agx-cli` ship on separate timelines. Bumping one does not require bumping the other — except as noted under [Multi-crate releases](#multi-crate-releases) below.

**Tag prerequisite:** `--unreleased` requires the previous release's tag to exist locally and on the remote. If you see the entire repo history in your scaffold, the tags weren't pushed; check `git tag -l '<crate>-v*'` and re-tag/re-push if needed.

## One-time setup

Install the release tools:

```bash
cargo install cargo-release git-cliff
```

(Or use `cargo binstall` if installed — downloads prebuilt binaries instead of compiling from source.)

Verify your crates.io token is in `~/.cargo/credentials.toml`:

```bash
test -f ~/.cargo/credentials.toml && echo OK || cargo login
```

## Versioning rules

Pre-1.0, Cargo treats `0.x.y` as compatible within `0.x` and incompatible across minors, so the minor axis is the breaking-change axis until a 1.0 cut.

| Change | Bump |
|---|---|
| Public API addition (lib) | minor (0.x.0) |
| Public API breaking change (lib) | minor pre-1.0 (Cargo treats 0.x as the breakage axis) |
| Bug fix (lib or CLI) | patch (0.0.x) |
| New CLI subcommand or flag | minor |
| CLI breaking flag/binary change | minor pre-1.0 |

## Release steps (single crate)

The two publishable crates have different filesystem paths and Cargo package names:

| Crate dir              | Cargo package name | Tag scheme            |
|------------------------|--------------------|-----------------------|
| `crates/agx/`          | `agx-photo`        | `agx-photo-vX.Y.Z`    |
| `crates/agx-cli/`      | `agx-cli`          | `agx-cli-vX.Y.Z`      |

The path fragment goes into `git cliff --include-path` and the `git add` / commit subject. The package name goes into `cargo release -p`. Don't conflate them.

For a release of `agx-cli`:

1. **Confirm main is green.** On the `main` branch, run `./scripts/verify.sh` and `./scripts/e2e.sh`. Both should pass.

2. **Scaffold the changelog draft.** Use `--tag-pattern` so `git-cliff` only considers tags from this crate when computing the "since last release" range:

   ```bash
   git cliff --include-path 'crates/agx-cli/**' --tag-pattern 'agx-cli-v.*' --unreleased > /tmp/draft.md
   cat /tmp/draft.md
   ```

   The output is a Keep-a-Changelog-formatted entry of all `feat:`/`fix:`/`refactor:`/`perf:` commits scoped to `crates/agx-cli/` since the last `agx-cli-v*` tag.

3. **Curate the draft.** Open `/tmp/draft.md`. Condense bullets into narrative, drop noise, group related changes. The output should read as a short summary of the release, not a commit log dump — see existing entries in `crates/agx/CHANGELOG.md` and `crates/agx-cli/CHANGELOG.md` for the voice to match.

   Watch for **scope-mismatched entries.** A scaffold scoped to `crates/agx/**` may include a commit that touched a `crates/agx/` doc file as a side effect but is really about the other crate (e.g., `refactor(cli): ...` that updated a GPU README cross-reference). Drop those rather than fold them in.

4. **Paste curated entry into the changelog.** Open `crates/agx-cli/CHANGELOG.md` and paste the curated content under `## [Unreleased]`. The `[Unreleased]` heading itself stays — `cargo-release` rewrites it in the next step.

   A few additional touches at curate time:
   - Strip any `(#N)` PR references from commit subjects. Project convention bans `#N` in commits and PR bodies because GitHub auto-links every `#N` to whatever issue or PR happens to carry that number.
   - Update the link references at the bottom of `CHANGELOG.md`. Change the existing `[Unreleased]: ...compare/<previous-tag>...HEAD` line to use the new tag (e.g., `agx-cli-v0.2.0` → `agx-cli-v0.3.0`), and add a new `[X.Y.Z]: ...releases/tag/<crate>-vX.Y.Z` line for this release. `cargo-release` does not maintain these references automatically.
   - If `[Unreleased]` would be empty (e.g., a no-source-change re-release of `agx-cli` to pick up a new `agx-photo` — see [Multi-crate releases](#multi-crate-releases)), write a one-line entry under it before continuing. For example:

     ```markdown
     ### Changed

     - Updated agx-photo dependency to X.Y.Z.
     ```

     Otherwise the published changelog will have an entry header with no body.

5. **Make the bump on a release branch.** Releases can't be pushed to `main` directly (see [Branch protection](#branch-protection-releases-go-through-a-pr)), so produce the bump on a branch. Commit the curated changelog, then run `cargo-release` with tag/publish/push disabled — it bumps `Cargo.toml`, rewrites `[Unreleased]` to a dated section, updates dependent dep-pins, and commits, stopping there:

   ```bash
   git checkout -b release/agx-cli-vX.Y.Z
   git add crates/agx-cli/CHANGELOG.md
   git commit -m "docs(agx-cli): changelog for vX.Y.Z"

   # Dry-run the edits:
   cargo release <patch|minor|major> -p agx-cli --allow-branch 'release/*' --no-tag --no-publish --no-push
   # Execute them (commits the bump; no tag, no publish, no push):
   cargo release <patch|minor|major> -p agx-cli --allow-branch 'release/*' --no-tag --no-publish --no-push --execute
   ```

   `--allow-branch 'release/*'` overrides `release.toml`'s `allow-branch = ["main"]` for the release branch. It's safe because `--no-publish` independently removes the risk that gate guards against — an accidental crates.io upload from a non-`main` branch. `cargo-release` adds one `chore: Release` commit (the version bump + changelog date-stamp) on top of the changelog-curation commit you made earlier in this step; nothing is tagged, published, or pushed. **Run `cargo release` exactly once per crate** — re-running bumps again.

6. **Open the release PR and merge it.** Push the branch, open a PR, let the required checks pass, and merge. The ruleset requires linear history, so squash- or rebase-merge (not a merge commit):

   ```bash
   git push -u origin release/agx-cli-vX.Y.Z
   gh pr create --title "release: agx-cli vX.Y.Z" --body "Version bump + changelog for agx-cli vX.Y.Z."
   # after checks pass + review:
   gh pr merge --squash
   ```

   The merged commit on `main` now carries the version bump and the dated changelog entry.

7. **Tag and publish from merged `main`.** Update local `main`, tag the *merged release commit explicitly*, push the tag (tags aren't covered by the branch ruleset), and publish:

   ```bash
   git checkout main && git pull --ff-only
   # Resolve the merged release commit by SHA — don't tag HEAD, which may have
   # advanced if another PR merged in the window between merge and tagging.
   REL=$(gh pr view <release-PR#> --json mergeCommit --jq .mergeCommit.oid)
   git tag agx-cli-vX.Y.Z "$REL"
   git push origin agx-cli-vX.Y.Z   # tag push is allowed by the ruleset
   cargo publish -p agx-cli         # runs cargo publish --verify; irreversible
   ```

   Use plain `cargo publish` here, not `cargo release --execute` — the bump already merged, so cargo-release would bump again. `cargo publish` talks to crates.io directly (independent of git push) and runs a from-scratch verification build inside `target/package/` (1-3 minutes; not hung). Once it uploads, the version is on crates.io permanently — yank-only, not deletable.

   Don't use `git push --tags` — it pushes every local tag, including unvetted ones from a multi-crate workspace.

For a release of `agx-photo`, the same flow applies with substitutions:

| Token in steps above | `agx-photo` substitute |
|---|---|
| `crates/agx-cli/**`  | `crates/agx/**`        |
| `agx-cli-v.*`        | `agx-photo-v.*`        |
| `crates/agx-cli/`    | `crates/agx/`          |
| `docs(agx-cli):`     | `docs(agx):`           |
| `-p agx-cli`         | `-p agx-photo`         |
| `agx-cli-vX.Y.Z`     | `agx-photo-vX.Y.Z`     |

## Multi-crate releases

When `agx-photo` ships changes that `agx-cli` consumes, both crates ship in **one release PR**, and the crates.io publish order matters because `agx-cli`'s dep pin must resolve on the index.

1. **Bump both on one release branch.** On a single `release/...` branch, run the step-5 `cargo-release` edit command for `agx-photo` first, then for `agx-cli`:

   ```bash
   cargo release <bump> -p agx-photo --allow-branch 'release/*' --no-tag --no-publish --no-push --execute
   cargo release <bump> -p agx-cli   --allow-branch 'release/*' --no-tag --no-publish --no-push --execute
   ```

   Running `agx-photo` first matters: its edit rewrites `crates/agx-cli/Cargo.toml`'s `agx-photo` dep pin in place (e.g. `version = "0.1.0"` → `"0.2.0"`) because `release.toml` sets `dependent-version = "upgrade"`. Don't revert that — it keeps the workspace consistent. The `agx-cli` command then adds only the `agx-cli` version bump and CHANGELOG entry. Even if `agx-cli`'s own source didn't change, ship at least a patch bump so `cargo install agx-cli` picks up the new lib transitively (write a one-line `[Unreleased]` entry first — see step 4).

2. **One PR, merge it.** Push the branch and merge the PR (squash/rebase) per step 6. CI builds fine even though `agx-photo`'s new version isn't on crates.io yet: in the workspace, `agx-cli` resolves `agx-photo` via the **path** dependency locally; the `version` pin only matters for the *published* crate.

3. **Publish from merged `main`, `agx-photo` first.** Both tags point at the *same* merged release commit, so resolve it once and reuse it for both — don't let `HEAD` drift between the two publishes if another PR merges in between:

   ```bash
   git checkout main && git pull --ff-only
   REL=$(gh pr view <release-PR#> --json mergeCommit --jq .mergeCommit.oid)
   git tag agx-photo-v0.2.0 "$REL" && git push origin agx-photo-v0.2.0
   cargo publish -p agx-photo
   # wait for the index (sparse: seconds; legacy git index: ~a minute), then:
   git tag agx-cli-v0.3.0 "$REL" && git push origin agx-cli-v0.3.0
   cargo publish -p agx-cli
   ```

   Publishing `agx-cli` before `agx-photo` is on the index fails with "dependency not found in registry."

## Troubleshooting

- **"version X.Y.Z is already published"** — that version is on crates.io (live or yanked). Don't republish; bump again to a fresh version.

- **"dependency not found in registry"** during `agx-cli` publish — `agx-photo` either hasn't been released yet or the index hasn't caught up. Confirm with `cargo search agx-photo`. If it's there, retry.

- **Pre-release replacement fails** ("expected exactly 1 match for `## \[Unreleased\]`") — the changelog file is missing the `## [Unreleased]` heading, or has it more than once. Add or normalize before retrying. Casing matters: the regex matches `Unreleased` exactly, not `unreleased` or `UNRELEASED`.

- **"uncommitted changes detected"** — `cargo-release` refuses to run with any untracked or modified files in the working tree, not just modified-tracked changes. Run `git status`; commit, stash, or `.gitignore` the offender before retrying. Common culprits: editor swap files, local agent/tool directories (`.claude/`, etc.), `.DS_Store`.

- **Forgot to push the tag** — run `git push origin <crate>-vX.Y.Z` after the fact. The crates.io publish is independent of git push state, so the package is fine regardless.
