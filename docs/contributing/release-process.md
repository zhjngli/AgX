# Release Process

How to ship `agx-photo` (the library, in `crates/agx/`) and `agx-cli` (the binary, in `crates/agx-cli/`) to crates.io. Releases land on `main` through a **pull request** (`main` is branch-protected), then tagging and `cargo publish` run from your laptop. The two crates version independently. High-level summary: [`developer-workflow.md`](developer-workflow.md) Step 6.

| Package | Directory | Tag scheme |
|---|---|---|
| `agx-photo` | `crates/agx/` | `agx-photo-vX.Y.Z` |
| `agx-cli` | `crates/agx-cli/` | `agx-cli-vX.Y.Z` |

The **package name** (used in `cargo release -p` and the tag prefix) differs from the **directory** (used in `git cliff --include-path`). Don't conflate them.

## Checklist

The fast path. Each step links to its reference section below for the why and the edge cases. For a release that touches both crates, do `agx-photo` first at every step — see [Multi-crate releases](#multi-crate-releases).

1. **Green `main`.** `git checkout main && git pull --ff-only && ./scripts/verify.sh && ./scripts/e2e.sh`

2. **Release branch.** `git checkout -b release/<crate>-vX.Y.Z`

3. **Changelog** — scaffold, curate, commit ([reference](#changelogs-git-cliff--curation)):

   ```bash
   git cliff --include-path 'crates/<dir>/**' --tag-pattern '<crate>-v.*' --unreleased > /tmp/draft.md
   ```

   Edit `/tmp/draft.md` into prose under `## [Unreleased]` in `crates/<dir>/CHANGELOG.md`, update the link refs at the bottom of that file, then `git commit -m "docs(<crate>): changelog for vX.Y.Z"`.

4. **Bump** — once per crate ([reference](#the-bump-cargo-release)):

   ```bash
   cargo release <patch|minor|major> -p <crate> --allow-branch 'release/*' --no-tag --no-publish --no-push --execute
   ```

5. **PR + merge** ([why a PR](#branch-protection)). Push the branch, open a PR, wait for the required checks (`Fast checks`, `E2E tests`; a red `lychee` is expected and non-blocking — see [Troubleshooting](#troubleshooting)), then **squash-merge**.

6. **Tag + publish** from merged `main` ([reference](#the-publish-step)):

   ```bash
   git checkout main && git pull --ff-only
   REL=$(gh pr view <PR#> --json mergeCommit --jq .mergeCommit.oid)
   git tag <crate>-vX.Y.Z "$REL" && git push origin <crate>-vX.Y.Z
   cargo publish -p <crate>
   ```

The rest of this document is reference.

## When to release

**Soft trigger:** after notable PR merges, dry-run the changelog scaffold for whichever crate the change touched. If meaningful entries appear, ship within a week.

```bash
git cliff --include-path 'crates/agx/**'     --tag-pattern 'agx-photo-v.*' --unreleased  # agx-photo
git cliff --include-path 'crates/agx-cli/**' --tag-pattern 'agx-cli-v.*'   --unreleased  # agx-cli
```

The dry-run is the source of truth — the on-disk `[Unreleased]` section is normally empty between releases (curated at release time, not appended per-PR). There's no fixed schedule.

**Tag prerequisite:** `--unreleased` needs the previous release's tag to exist locally and on the remote. If the scaffold shows the entire repo history, the tags weren't pushed — check `git tag -l '<crate>-v*'` and re-push.

## One-time setup

```bash
cargo install cargo-release git-cliff   # (or cargo binstall for prebuilt binaries)
test -f ~/.cargo/credentials.toml && echo OK || cargo login   # crates.io token
```

## Versioning

Pre-1.0, Cargo treats `0.x.y` as compatible within `0.x` and incompatible across minors, so the minor axis is the breaking-change axis until a 1.0 cut.

| Change | Bump |
|---|---|
| Public API addition (lib) | minor (0.x.0) |
| Public API breaking change (lib) | minor pre-1.0 (Cargo treats 0.x as the breakage axis) |
| Bug fix (lib or CLI) | patch (0.0.x) |
| New CLI subcommand or flag | minor |
| CLI breaking flag/binary change | minor pre-1.0 |

---

## Branch protection

`main` is governed by a ruleset that requires a pull request for every change and has **no bypass actors** — not even repo admins can push to `main` directly. This is deliberate: the maintainer works alongside coding agents that act under the maintainer's identity, and a bypass would let an agent push unreviewed code to `main`. So the version-bump + changelog commits that a release produces **cannot be pushed straight to `main`** the way a vanilla `cargo release --execute` expects.

That's why the [checklist](#checklist) splits releasing into two halves:

1. **File edits via a PR** (steps 2–5) — the bump, `CHANGELOG.md` stamp, and (for `agx-photo`) the `agx-cli` dep-pin bump are produced on a release branch and merged through a normal PR. Required checks must pass, and the ruleset requires linear history (no merge commits) — **squash-merge** (a rebase-merge also satisfies it).

2. **Tag + publish from merged `main`** (step 6) — tags are *not* covered by the branch ruleset, so `git push origin <tag>` works; and `cargo publish` talks to crates.io directly, independent of any git push.

> If a future maintainer adds a bypass actor to the ruleset (e.g. a release service account that agents can't impersonate), the all-in-one `cargo release --execute` flow becomes viable again. The split flow assumes no bypass.

## Changelogs: git-cliff + curation

Two files, `crates/agx/CHANGELOG.md` and `crates/agx-cli/CHANGELOG.md`, in [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) format. The content is **hand-curated**; the tooling only scaffolds a draft and (later) stamps the version.

**Scaffold (`git-cliff`).** `--tag-pattern` scopes the "since last release" range to one crate's tags — without it, `--unreleased` anchors to whichever crate's tag is globally most recent:

```bash
git cliff --include-path 'crates/<dir>/**' --tag-pattern '<crate>-v.*' --unreleased > /tmp/draft.md
```

The draft groups `feat:`/`fix:`/`refactor:`/`perf:` commits under Keep-a-Changelog headings (`docs:`/`chore:`/`style:`/`test:`/`build:`/`ci:` are skipped). It's a commit dump, not a release note.

**Curate.** Edit the draft into a short narrative — condense bullets, drop noise, group related changes — and paste it under `## [Unreleased]` in the crate's `CHANGELOG.md`. Match the voice of the existing entries. Watch for **scope-mismatched entries**: a scaffold scoped to one crate can include a commit that touched that crate's files as a side effect but is really about the other crate — drop those.

At curate time, also:

- **Strip any `(#N)` from the curated entry.** `git-cliff` copies commit subjects into the draft verbatim, and a `#N` in a published changelog auto-links to an unrelated issue/PR. (This edits the changelog draft, not the already-merged commits — the same convention keeps `#N` out of commit subjects in the first place.)
- **Update the link refs** at the bottom of `CHANGELOG.md`: point the `[Unreleased]: …compare/<previous-tag>...HEAD` line at the new tag, and add a `[X.Y.Z]: …releases/tag/<crate>-vX.Y.Z` line. `cargo-release` does not maintain these.
- If `[Unreleased]` would otherwise be **empty** (e.g. a no-source-change `agx-cli` re-release to pick up a new `agx-photo`), write a one-line entry so the published section isn't a bare header — for example, an `### Changed` entry reading "Updated agx-photo dependency to X.Y.Z."

Commit the curated changelog before the bump: `git commit -m "docs(<crate>): changelog for vX.Y.Z"`.

## The bump: cargo-release

On the release branch, `cargo-release` does the **mechanical file edits only** — no tag, no publish, no push:

```bash
cargo release <patch|minor|major> -p <crate> --allow-branch 'release/*' --no-tag --no-publish --no-push --execute
```

It:

- bumps the version in the crate's `Cargo.toml`;
- rewrites `## [Unreleased]` → `## [Unreleased]` (fresh, empty) + `## [X.Y.Z] - <date>` in `CHANGELOG.md`, so your curated content slides under the dated heading (driven by `pre-release-replacements` in `release.toml` — a find-and-replace, nothing smart);
- for `agx-photo`, rewrites `agx-cli`'s `agx-photo` dep-pin (via `dependent-version = "upgrade"`);
- and commits all of that as one `chore: Release` commit on top of your changelog-curation commit.

Notes:

- **`--allow-branch 'release/*'`** overrides `release.toml`'s `allow-branch = ["main"]` for the release branch. It's safe because `--no-publish` independently removes the risk that gate guards against — an accidental crates.io upload from a non-`main` branch.
- **Run it exactly once per crate.** `--execute` bumps every time it runs; running it twice over-bumps (reset the branch and redo).
- Drop `--execute` for a dry-run that prints the planned edits without making them.

## The publish step

After the PR merges, publish from `main`. This step changes no files — it tags the merged commit and uploads the already-merged source.

```bash
git checkout main && git pull --ff-only
REL=$(gh pr view <PR#> --json mergeCommit --jq .mergeCommit.oid)   # resolve the merged release commit
git tag <crate>-vX.Y.Z "$REL"
git push origin <crate>-vX.Y.Z
cargo publish -p <crate>
```

- **Tag the merge SHA explicitly (`$REL`), not `HEAD`** — `HEAD` can drift if another PR merges in the window between merge and tagging.
- **Tags aren't covered by the branch ruleset**, so the tag push works. Don't use `git push --tags` — it pushes every local tag, including unvetted ones from the multi-crate workspace.
- **Use plain `cargo publish`, not `cargo release --execute`** — the bump already merged, so cargo-release would bump again.
- `cargo publish` runs a from-scratch `--verify` build inside `target/package/` (1–3 minutes; not hung), then uploads. **Once uploaded, the version is permanent on crates.io — yank-only, not deletable.**

Optionally, cut a GitHub Release from each tag and paste that version's changelog section as the notes.

## Multi-crate releases

When `agx-photo` ships changes that `agx-cli` consumes, both crates ship in **one release PR**, and the publish order matters because `agx-cli`'s dep pin must resolve on the index.

1. **Bump both on one release branch**, `agx-photo` first (its bump rewrites `agx-cli`'s `agx-photo` dep-pin in place — don't revert that):

   ```bash
   cargo release <bump> -p agx-photo --allow-branch 'release/*' --no-tag --no-publish --no-push --execute
   cargo release <bump> -p agx-cli   --allow-branch 'release/*' --no-tag --no-publish --no-push --execute
   ```

   Even if `agx-cli`'s own source didn't change, ship at least a patch bump so `cargo install agx-cli` picks up the new lib transitively (write a one-line `[Unreleased]` entry first).

2. **One PR, merge it.** CI builds fine even though `agx-photo`'s new version isn't on crates.io yet — in the workspace, `agx-cli` resolves `agx-photo` via the **path** dependency; the `version` pin only matters for the *published* crate.

3. **Publish from merged `main`, `agx-photo` first.** Resolve the merge SHA once and reuse it for both tags:

   ```bash
   git checkout main && git pull --ff-only
   REL=$(gh pr view <release-PR#> --json mergeCommit --jq .mergeCommit.oid)
   git tag agx-photo-vX.Y.Z "$REL" && git push origin agx-photo-vX.Y.Z
   cargo publish -p agx-photo
   # wait for the index (sparse: seconds; legacy git index: ~a minute), then:
   git tag agx-cli-vA.B.C "$REL" && git push origin agx-cli-vA.B.C   # independent version
   cargo publish -p agx-cli
   ```

   Publishing `agx-cli` before `agx-photo` is on the index fails with "dependency not found in registry."

## Troubleshooting

- **lychee 404s on the new changelog links** (`…/releases/tag/<crate>-vX.Y.Z`, `…/compare/<crate>-vX.Y.Z...HEAD`) on the release PR. Expected: those URLs don't exist until the tag is pushed at publish time. lychee is a non-required check, so it doesn't block the merge, and it goes green on its own once you publish.
- **"version X.Y.Z is already published"** — that version is on crates.io (live or yanked). Don't republish; bump to a fresh version.
- **"dependency not found in registry"** during `agx-cli` publish — `agx-photo` either hasn't been released yet or the index hasn't caught up. Confirm with `cargo search agx-photo`, then retry.
- **"expected exactly 1 match for `## \[Unreleased\]`"** — the changelog is missing the `## [Unreleased]` heading or has it more than once. Add or normalize. Casing matters (`Unreleased`, not `unreleased`).
- **"uncommitted changes detected"** — `cargo-release` refuses to run with any untracked or modified files, not just tracked ones. `git status`; commit, stash, or `.gitignore` the offender. Common culprits: editor swap files, local tool dirs (`.claude/`, `local/`), `.DS_Store`.

## Automating this (future)

The data model here (tags, changelogs, conventional commits, per-crate timelines) is what [`release-plz`](https://release-plz.dev/) consumes. It fits the PR-based, branch-protected flow natively (it opens a release PR and publishes from CI on merge) and pairs well with crates.io Trusted Publishing (OIDC — no stored token). Adopting it is a workflow file plus auth setup, not a structural rewrite; see the "Future migration" note in `docs/plans/2026-04-30-release-process-design.md`.
