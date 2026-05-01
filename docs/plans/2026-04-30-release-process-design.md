# Release Process and Changelog System

**Status:** approved 2026-04-30

## Goal

Establish a repeatable release process for the two publishable crates
(`agx-photo`, `agx-cli`), backfill changelogs for what has already shipped, and
document the workflow in `docs/contributing/` so future-you and contributors
ship without re-deriving the steps each time.

## Motivation

Both crates are live on crates.io. `agx-photo` 0.1.0 and `agx-cli` 0.1.0
shipped together via the publish-prep PR (#36, commit `84f6dbb`, 2026-04-26).
`agx-cli` 0.2.0 just shipped manually (commit `0bf1649`, 2026-04-29) renaming
the installed binary from `agx-cli` to `agx`.

Pain points the manual process exposed:

- Six discrete steps every release (bump version, edit changelog, commit, tag,
  publish, push tags) — each forgettable.
- Inter-crate dep pin in `agx-cli/Cargo.toml` line 19 (`agx = { ..., version =
  "0.1.0", ... }`) must be hand-updated when the lib bumps. Easy to miss.
- No tags exist yet for the three releases already on crates.io. Without tags,
  any future changelog tooling can't compute "since last release" ranges.
- No `CHANGELOG.md` files exist. Anyone reading the crates.io page sees no
  history of what shipped when.
- `docs/contributing/developer-workflow.md` covers feature work but stops
  before release. The release lifecycle is undocumented.

## Non-goals

- **Full CI-driven release automation** (release-plz, GitHub Actions release
  workflow on tag push). Defer until contributor or cadence pressure exists.
  See "Future migration" below.
- **Lockstep versioning** for the two crates. Independent timelines fit
  separate audiences (library consumers vs CLI users).
- **Pre-PR changelog discipline.** Curated entries are written at release
  time, scaffolded from conventional commit history.
- **Signing commits/tags.** Disabled by default in config; can flip later if
  supply chain provenance becomes important.

## Approach

Adopt local `cargo-release` for the publish dance and `git-cliff` for
scaffolding changelog entries from conventional commits. Both run on the
maintainer's laptop. CI is not involved.

### Versioning policy

Each publishable crate has its own version timeline.

| Change in | Bump |
|---|---|
| Public API addition (lib) | minor (0.x.0) |
| Public API breaking change (lib) | major (x.0.0) — pre-1.0, treated as minor in practice |
| Bug fix (lib or CLI) | patch (0.0.x) |
| New CLI subcommand or flag | minor |
| CLI breaking flag/binary change | minor pre-1.0 (e.g., the binary rename to `agx` was 0.1 → 0.2) |

Pre-1.0, Cargo treats `0.x.y` as compatible within `0.x` and incompatible
across minors, so the minor axis is the breaking-change axis until we cut 1.0.

The internal dep pin `agx-cli/Cargo.toml:19` is rewritten by `cargo-release`
when the lib bumps, via `dependent-version = "upgrade"`. When `agx-photo`
bumps, `agx-cli` should re-release as well so `cargo install agx-cli` pulls
the new lib transitively, even if the CLI's own source did not change.

### Changelog system

Two files: `crates/agx/CHANGELOG.md` and `crates/agx-cli/CHANGELOG.md`. Both
follow [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) format with
sections Added, Changed, Deprecated, Removed, Fixed, Security (skip empty
ones).

Generation flow at release time (the C-tier hybrid):

1. `git cliff --include-path "crates/<crate>/**" --unreleased` produces a
   draft entry from conventional commits since the last tag.
2. Maintainer curates the draft: condenses bullets into narrative, drops
   noise, writes prose for major changes.
3. Curated entry pasted under `## [Unreleased]` in the crate's `CHANGELOG.md`.
4. `cargo release <bump> -p <crate> --execute` runs. As part of its
   `pre-release-replacements`, it rewrites `## [Unreleased]` to
   `## [Unreleased]\n\n## [x.y.z] - YYYY-MM-DD`, leaving a fresh empty
   `[Unreleased]` on top.

Conventional commit prefixes drive scaffold grouping:

| Prefix | Scaffold section | Notes |
|---|---|---|
| `feat:` | Added | |
| `fix:` | Fixed | |
| `refactor:`, `perf:` | Changed | |
| `docs:`, `chore:`, `style:`, `test:`, `build:`, `ci:` | _skipped_ | not surfaced in changelogs; mention notable rustdoc improvements in narrative if relevant |

### Tooling configuration

#### `release.toml` (repo root)

Workspace-aware `cargo-release` config. Key settings:

```toml
sign-commit = false
sign-tag = false

# Tag scheme for multi-crate workspace: agx-cli-v0.2.0, agx-photo-v0.1.0
tag-name = "{{crate_name}}-v{{version}}"

# Auto-rewrite agx-cli's dep pin on agx-photo when agx-photo bumps
dependent-version = "upgrade"

# Don't push automatically — review tag locally before pushing
push = false

# At release time, rewrite [Unreleased] to a dated version section
pre-release-replacements = [
  { file = "CHANGELOG.md",
    search = "## \\[Unreleased\\]",
    replace = "## [Unreleased]\n\n## [{{version}}] - {{date}}",
    exactly = 1 },
]
```

Per-crate overrides go in `crates/<name>/Cargo.toml` under
`[package.metadata.release]` if needed. Default is workspace-wide config above.

#### `cliff.toml` (repo root)

`git-cliff` config. Key settings:

```toml
[changelog]
header = """
# Changelog

All notable changes to this crate are documented here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
"""

body = """
## [{{ version }}] - {{ timestamp | date(format="%Y-%m-%d") }}
{% for group, commits in commits | group_by(attribute="group") %}
### {{ group | upper_first }}
{% for commit in commits %}
- {{ commit.message | upper_first }} ([{{ commit.id | truncate(length=7, end="") }}](https://github.com/zhjngli/AgX/commit/{{ commit.id }}))
{%- endfor %}
{% endfor %}
"""

[git]
filter_unconventional = false
conventional_commits = true
commit_parsers = [
  { message = "^feat",                                   group = "Added"   },
  { message = "^fix",                                    group = "Fixed"   },
  { message = "^refactor|^perf",                         group = "Changed" },
  { message = "^docs|^chore|^style|^test|^build|^ci",    skip = true       },
]
```

Per-crate scoping at invocation time: `git cliff --include-path
"crates/<crate>/**"`.

### Backfill plan

Three retroactive entries written on the implementation branch before any
tooling lands. Changelog files must exist before `cargo-release` can manage
them.

#### `agx-photo` 0.1.0 (commit `84f6dbb`, 2026-04-26)

Scaffold from full history up to `84f6dbb` scoped to `crates/agx/**`
(exact `git-cliff` invocation worked out during implementation). Curate
into a B-style summary covering: render pipeline architecture, decode
(JPEG/PNG/TIFF/RAW), full adjustment suite, presets (TOML + composability),
LUTs (.cube), GPU acceleration, parallelization, profiling infrastructure.

#### `agx-cli` 0.1.0 (commit `84f6dbb`, 2026-04-26)

Same range, scoped to `crates/agx-cli/**`. Curate into a summary covering
subcommands (`edit`, `apply`, `multi-apply`, `batch-apply`, `batch-edit`),
output formats, GPU opt-in, parallel batch.

#### `agx-cli` 0.2.0 (commit `0bf1649`, 2026-04-29)

Hand-write directly — small scope:

> Renamed installed binary from `agx-cli` to `agx`. The crates.io package
> remains `agx-cli`. Migration: scripts referring to `agx-cli` on PATH need
> updating to `agx`. Subcommand interface and flag set unchanged.

#### Tags

After the implementation PR merges to main, tag the historical commits and
push:

```sh
git tag agx-photo-v0.1.0 84f6dbb
git tag agx-cli-v0.1.0   84f6dbb
git tag agx-cli-v0.2.0   0bf1649
git push origin agx-photo-v0.1.0 agx-cli-v0.1.0 agx-cli-v0.2.0
```

Use the per-tag push form rather than `git push --tags`. The `--tags` form
pushes every local tag, including any in-progress or unvetted ones, which
becomes a real footgun in a multi-crate workspace once steady-state release
work begins. Per-tag push is the convention this project uses going forward.

Tags are essential for `git-cliff` going forward — without them, the next
release's scaffold would re-include 0.1.0 commits.

### Documentation updates

#### `docs/contributing/release-process.md` (new file)

Self-contained guide. Sections: when to release, one-time setup
(`cargo install cargo-release git-cliff`), versioning rules, release steps,
multi-crate ordering, troubleshooting.

The "when to release" trigger: glance at the crate's `[Unreleased]` section
after PR merges; if user-visible changes have accumulated, ship within a
week. Soft heuristic, no schedule.

The detailed step list lives in `release-process.md` itself; this design doc
does not duplicate the commands. At a high level: scaffold a draft from
conventional commits with `git cliff` (using `--tag-pattern` so the
"since last release" range stays scoped to one crate's tag history), curate
into the crate's `CHANGELOG.md` under `[Unreleased]`, then run
`cargo release <bump> -p <package> --execute`, then push the specific tag.

Multi-crate ordering: release `agx-photo` first (CLI dep pin must resolve on
crates.io); `cargo-release` handles the dep pin rewrite via
`dependent-version = "upgrade"`; release `agx-cli` after.

#### `docs/contributing/developer-workflow.md` (extend)

- Step 4 (Document) gains a sentence noting that `CHANGELOG.md` exists per
  crate but is curated at release time, not per-PR.
- New Step 7 (Release, when applicable) summarizes the release flow and links
  to `release-process.md`. Reinforces that conventional commit prefixes are
  load-bearing for changelog scaffolding.

#### `CLAUDE.md`

One-sentence addition to the conventional commits section: prefixes determine
changelog grouping at release time.

## Implementation order

Single branch (`chore/release-process`). Commits in order so each stands
alone:

1. `chore: add cargo-release and git-cliff config` — `release.toml`,
   `cliff.toml`. No behavior change.
2. `docs(agx): backfill changelog through 0.1.0` — create
   `crates/agx/CHANGELOG.md`.
3. `docs(agx-cli): backfill changelog through 0.2.0` — create
   `crates/agx-cli/CHANGELOG.md`.
4. `docs(contributing): release process and changelog discipline` —
   new `release-process.md`, extend `developer-workflow.md`, one-sentence
   `CLAUDE.md` touch.

Single PR. Files are tightly coupled (config + changelogs + docs all
establish the same workflow); splitting would force ordering games.

Tag commits happen after PR merges to main, against the existing published
commits (`84f6dbb`, `0bf1649`).

## Verification

Before merge:

- `verify.sh` passes (markdown lint catches malformed changelogs).
- Manual: run `git cliff --include-path "crates/agx/**" --unreleased` against
  a synthetic new feature commit, confirm output groups correctly.
- Manual: `cargo release patch -p agx-cli` (no `--execute`) prints planned
  actions; confirm bumps + replacements look right.

## Out of scope

- **CI-driven release** (release-plz, GitHub Actions on tag push). Revisit
  when contributors land or cadence exceeds ~monthly.
- **Per-PR changelog discipline.** We picked the C tier (curate at release
  time). If `git-cliff` scaffolding feels noisy in practice, revisit.
- **Lockstep versioning** for the two crates. Independent stays.
- **PR template enforcing changelog updates.** Unnecessary under the C tier.
- **Signing commits/tags.** Off by default.
- **`scripts/install-dev-tools.sh`.** Two tools doesn't justify a script.
  Document the `cargo install` line in `release-process.md` and stop.

## Future migration

The data model (tags, changelogs, conventional commits, per-crate timelines)
is identical to what `release-plz` consumes. If/when the project warrants
CI-driven releases — typically when contributors begin landing PRs you didn't
write, or cadence exceeds monthly — migration is a workflow file plus a
GitHub token, not a structural rewrite. Current design intentionally keeps
that door open.

## References

- [Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/)
- [`cargo-release` docs](https://github.com/crate-ci/cargo-release)
- [`git-cliff` docs](https://git-cliff.org/)
- [Conventional Commits](https://www.conventionalcommits.org/)
- `docs/contributing/developer-workflow.md` — existing dev workflow this extends
