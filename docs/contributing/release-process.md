# Release Process

This document describes how to ship a release of `agx-photo` or `agx-cli` to crates.io. Releases are local-only — execution runs from the maintainer's laptop, no CI involvement. Per-crate independent versioning: each publishable crate has its own version timeline.

The high-level summary lives in [`developer-workflow.md`](developer-workflow.md) Step 6. This document has the operational detail.

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

5. **Commit the changelog edit:**

   ```bash
   git add crates/agx-cli/CHANGELOG.md
   git commit -m "docs(agx-cli): changelog for vX.Y.Z"
   ```

6. **Run cargo-release.** This bumps `Cargo.toml`, rewrites `[Unreleased]` to a dated section in `CHANGELOG.md`, commits the version bump, tags as `agx-cli-vX.Y.Z`, and publishes to crates.io:

   ```bash
   # Dry run first — prints planned actions without bumping or publishing:
   cargo release <patch|minor|major> -p agx-cli

   # Then commit to it:
   cargo release <patch|minor|major> -p agx-cli --execute
   ```

   Without `--execute`, cargo-release runs in dry-run mode and shows what it would do. With `--execute`, it prompts before each side-effecting step. Once `cargo publish` runs (the last prompt), the version is on crates.io permanently — yank-only, not deletable. Read each prompt before confirming.

   Each release leaves two commits on `main`: your changelog edit from step 5 and `cargo-release`'s auto-generated `chore: Release <crate> version X.Y.Z`. That's expected; `release.toml` does not set `consolidate-commits = true` and no amending is needed.

   `cargo-release` runs `cargo publish --verify` as part of step 6, which performs a from-scratch verification build inside `target/package/`. Expect 1-3 minutes between prompts during this phase — it's not hung. The verify step is intentional (catches "works on my machine, breaks for downstream") and is on by default.

7. **Push the tag.** `release.toml` has `push = false`, so the tag is local only after `cargo-release`. Push the specific tag and the bumped main commit:

   ```bash
   git push origin agx-cli-vX.Y.Z
   git push origin main
   ```

   Don't use `git push --tags` here — it pushes all local tags, including any in-progress or unvetted ones from a multi-crate workspace.

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

When `agx-photo` ships changes that `agx-cli` consumes, both crates ship. The order matters because `agx-cli`'s dep pin must resolve on crates.io.

1. **Release `agx-photo` first.** Follow the single-crate steps above for `agx-photo`. After `cargo publish` succeeds, the new lib version is on crates.io.

   Side effect to expect: the `agx-photo` release commit also rewrites `crates/agx-cli/Cargo.toml`'s `agx-photo` dep pin in place (e.g., `version = "0.1.0"` → `version = "0.1.1"`). This is intentional — `release.toml` sets `dependent-version = "upgrade"`, which keeps the workspace internally consistent. Don't revert the change. The subsequent `agx-cli` release commit will then carry only the `agx-cli` version bump and CHANGELOG rewrite.

2. **Wait for the index.** crates.io's sparse index updates within seconds; the legacy git index can take up to a minute. `cargo-release` for `agx-cli` will retry automatically.

3. **Release `agx-cli`.** Follow the single-crate steps above for `agx-cli`. The dep pin was already updated in step 1, so this commit only adds the `agx-cli` version bump and CHANGELOG entry. Even if `agx-cli`'s own source did not change, ship a patch bump so users running `cargo install agx-cli` pick up the new lib transitively. (See the empty-`[Unreleased]` note in step 4 of the single-crate flow for this exact scenario.)

## Troubleshooting

- **"version X.Y.Z is already published"** — that version is on crates.io (live or yanked). Don't republish; bump again to a fresh version.

- **"dependency not found in registry"** during `agx-cli` publish — `agx-photo` either hasn't been released yet or the index hasn't caught up. Confirm with `cargo search agx-photo`. If it's there, retry.

- **Pre-release replacement fails** ("expected exactly 1 match for `## \[Unreleased\]`") — the changelog file is missing the `## [Unreleased]` heading, or has it more than once. Add or normalize before retrying.

- **Forgot to push the tag** — run `git push origin <crate>-vX.Y.Z` after the fact. The crates.io publish is independent of git push state, so the package is fine regardless.
