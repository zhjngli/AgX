# Developer Workflow

This document describes the full development cycle for agx. Follow these steps when implementing features, fixing bugs, or making structural changes. The summary version lives in `CLAUDE.md`; this document has the detail.

## 1. Design

### When to write a design doc

Write a design doc in `docs/plans/YYYY-MM-DD-<feature>-design.md` when the change:

- Adds or modifies a module
- Changes dependency relationships between modules
- Touches 3 or more files
- Has multiple valid implementation approaches
- Introduces a new core invariant or changes an existing one

### When to skip

Skip the design doc for:

- Bug fixes with a clear root cause
- Single-file changes (adding a function, fixing a test)
- Documentation-only updates
- Dependency version bumps

### What goes in a design doc

- **Goal**: One sentence describing what this builds
- **Motivation**: Why this is needed
- **Approach**: How it works, with alternatives considered
- **Dependency changes**: Which modules gain or lose imports (reference `ARCHITECTURE.md`)
- **Negative constraint changes**: Anything a module must now do or stop doing
- **Testing strategy**: How you'll verify it works

### Picking up a backlog item

When starting work on a feature from the backlog (`docs/backlog/`), remove the backlog file as part of the feature branch. The backlog doc captures the initial scope; the design doc in `docs/plans/` captures the actual decisions.

## 2. Implement

### Branch naming

- `feat/<feature-name>` for new features
- `fix/<bug-description>` for bug fixes
- `refactor/<what>` for restructuring without behavior change

### Test-driven development

For new behavior:

1. Write a failing test that describes the expected behavior
2. Run it to confirm it fails for the right reason
3. Write the minimal code to make it pass
4. Run all tests to confirm nothing broke
5. Refactor if needed (tests still pass)

For bug fixes:

1. Write a test that reproduces the bug
2. Confirm it fails
3. Fix the bug
4. Confirm the test passes

### Commit discipline

- One logical change per commit
- Commit messages and PR titles follow conventional format: `feat:`, `fix:`, `refactor:`, `test:`, `docs:`, `style:`, `chore:`, `build:`
- For mixed-scope PRs, use the most representative prefix (e.g. `chore:` for maintenance PRs spanning multiple types)
- Commit after each meaningful step — don't batch unrelated changes

### Module contracts

Before writing code in a module, read its README.md (linked from `ARCHITECTURE.md`). The README documents:

- What the module does and does not do
- Its public API
- How to extend it
- Key design decisions

Follow the existing patterns. If you need to deviate, that's a signal you may need a design doc.

## 3. Verify

Run `scripts/verify.sh` before considering work done. The script runs:

| Check | Command | What it catches |
|-------|---------|-----------------|
| Format | `cargo fmt --check` | Inconsistent formatting |
| Lint | `cargo clippy -p agx -p agx-cli -- -D warnings` | Code quality issues, unused imports, type errors |
| Library tests | `cargo test -p agx` | Unit test failures, architecture violations |
| CLI tests | `cargo test -p agx-cli` | Integration test failures |
| Doc links | Custom check | Broken links in ARCHITECTURE.md |

The script exits on first failure with a message about what went wrong.

### E2E tests

For changes that affect editing, the rendering pipeline, presets, or LUTs, also run `scripts/e2e.sh`. This builds the CLI in release mode and runs the full golden comparison suite (54 image x look tests). See `crates/agx-e2e/README.md` for details.

When adding new editing features, update the e2e test pipeline alongside the implementation: add or update look presets that exercise the feature, regenerate LUTs if applicable (via `agx-lut-gen`), and update golden files with `GOLDEN_UPDATE=1 cargo test -p agx-e2e`.

### When a check fails

- **Format**: Run `cargo fmt` and re-run the script.
- **Clippy**: Read the warning, fix the code. Don't suppress with `#[allow(...)]` unless there's a genuine false positive.
- **Unit tests**: A test you didn't touch is failing — you may have broken an invariant. Read the test to understand what it expects.
- **Architecture tests**: See `docs/contributing/evolving-architecture.md` for the protocol.
- **Doc links**: A README or design doc referenced in ARCHITECTURE.md doesn't exist. Either create the missing file or fix the link.

## 4. Document

Update documentation alongside code. The rule: if someone reads the docs after your change, they should get an accurate picture.

| What changed | Update |
|---|---|
| Added a public function or type | Module README: Public API section |
| Added a new module | ARCHITECTURE.md: dependency graph, rules table, per-module table. Create module README. |
| Changed module dependencies | ARCHITECTURE.md: dependency graph, rules table. Affected module READMEs. `architecture.rs` structural test. |
| Changed a core invariant | ARCHITECTURE.md: Core Invariants section |
| Wrote a design doc | Save to `docs/plans/YYYY-MM-DD-<topic>-design.md`. No separate index — design docs are discovered by browsing the directory. |
| Changed how to extend a module | Module README: Extension Guide section |

Per-crate `CHANGELOG.md` files exist for both publishable crates (`crates/agx/`, `crates/agx-cli/`) but are **not updated per-PR**. Changelog entries are scaffolded from conventional commit messages by `git-cliff` and curated at release time. Use accurate `feat:` / `fix:` / `refactor:` / `perf:` prefixes — they map directly to changelog sections (Added / Fixed / Changed). `docs:`, `chore:`, `style:`, `test:`, `build:`, `ci:` are skipped from changelogs.

For style guidance on `///` comments, `//!` comments, the shared `.md` file convention for cross-surface explanations, and the active rustdoc lints, see [documentation-conventions.md](documentation-conventions.md).

### New adjustment-bearing modules

When adding a new `adjust/` submodule or any algorithm-bearing module:

1. Create a sibling `.md` next to the `.rs` following the template in [`documentation-conventions.md`](documentation-conventions.md).
2. Add `#![doc = include_str!("<name>.md")]` at the top of the `.rs` file so rustdoc picks up the explanation.
3. Create or update the appropriate wrapping mdbook page under `docs/book/src/explanation/` with `{{#include}}` pointing at the sibling, and add or update the `SUMMARY.md` chapter in pipeline order.
4. If the module ships with WGSL shaders, give each shader the five-line structured header from the WGSL section of `documentation-conventions.md`.
5. If the module ships with GPU code, update `crates/agx/src/engine/gpu/README.md`.

## 5. Self-Review

Before declaring done, re-read your diff and ask:

- **Completeness**: Did I implement everything that was asked? Did I miss edge cases?
- **Scope**: Did I add anything that wasn't asked for? Did I over-engineer?
- **Tests**: Do my tests verify behavior (not just exercise code paths)? Would a bug in my code actually cause a test to fail?
- **Naming**: Are function/type/variable names clear? Do they describe what, not how?
- **Contracts**: Does my code respect the module's negative constraints (Does NOT section in README)?
- **Architecture**: Did I introduce any cross-module dependencies? If so, are they allowed by the rules in ARCHITECTURE.md?

## 6. Release (when applicable)

Most PRs do not trigger a release on merge — releases happen periodically when user-visible changes have accumulated. After a notable merge, decide whether to ship by dry-running the changelog scaffold for the relevant crate. For example, for `agx-cli`:

```bash
git cliff --include-path 'crates/agx-cli/**' --tag-pattern 'agx-cli-v.*' --unreleased
```

For `agx-photo`, swap path to `crates/agx/**` and tag pattern to `agx-photo-v.*` (the package name is `agx-photo` even though the directory is `agx`). The on-disk `[Unreleased]` section in `CHANGELOG.md` stays empty between releases — entries are scaffolded and curated at release time, not appended per-PR. See [release-process.md](release-process.md) for the full operational flow.

If your work is user-visible (new feature, bug fix, breaking change), it will surface in the next release's changelog scaffolded from your conventional commit messages. Use accurate prefixes (`feat:` → Added, `fix:` → Fixed, `refactor:` / `perf:` → Changed) — `docs:`, `chore:`, `style:`, `test:`, `build:`, `ci:` are skipped.

When releasing, follow [release-process.md](release-process.md) for the operational detail: install tooling, scaffold changelog, curate, run `cargo-release`, push tags.
