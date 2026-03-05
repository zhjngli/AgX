# Agent Developer Loop for Oxiraw

**Date**: 2026-03-05
**Status**: Approved

## Overview

Encode a complete developer workflow into the repo so that any AI agent reading CLAUDE.md knows the full cycle: design, implement, test, document, verify, self-review. This builds on the harness engineering work (navigable architecture docs, structural tests, module READMEs) by adding the *process* layer — telling agents not just what the codebase looks like, but how to work in it.

## Motivation

The harness engineering initiative gave agents a map of the architecture and mechanical enforcement of module boundaries. But the agent still relies on external skills or human prompting to know the development *process*. A fresh agent session (Claude Code, Codex, or other) that reads CLAUDE.md currently sees a Definition of Done checklist but no procedural workflow. The gap: agents know what to verify but not how to work.

**Goals:**
- Any agent that reads CLAUDE.md knows the full design-to-merge cycle
- A single `scripts/verify.sh` command answers "am I done?"
- Design docs are required for structural changes but not for small fixes
- The workflow is encoded in the repo, not in agent-specific skills

## Deliverables

### 1. CLAUDE.md: Developer Workflow section

Add a **Developer Workflow** section with 5 numbered steps:

1. **Design** (threshold-based) — write a design doc in `docs/plans/` when the change adds/modifies modules, changes dependencies, or touches 3+ files. Skip for bug fixes, single-file changes, and doc updates.
2. **Implement** — feature branch, tests alongside code, follow module contracts, incremental commits.
3. **Verify** — run `scripts/verify.sh`.
4. **Document** — update ARCHITECTURE.md and module READMEs as needed.
5. **Self-review** — re-read the diff, check for scope creep and test quality.

The existing Definition of Done becomes a simplified merge gate:
1. `scripts/verify.sh` passes
2. `ARCHITECTURE.md` updated if needed
3. Affected module READMEs updated
4. Design doc cross-linked from ARCHITECTURE.md if applicable

### 2. `docs/contributing/developer-workflow.md`

Detailed reference expanding each workflow step:

- **Design step**: What belongs in a design doc, examples of changes that do/don't need one.
- **Implementation discipline**: TDD guidance, commit granularity, branch naming (`feat/`, `fix/`, `refactor/`).
- **Verification detail**: What `scripts/verify.sh` runs, how to interpret failures, link to evolving-architecture.md for structural test failures.
- **Documentation obligations**: Table mapping change types to required doc updates.
- **Self-review checklist**: Concrete questions to ask before declaring done.

### 3. `scripts/verify.sh`

Single verification script that runs:

1. `cargo fmt --check` — format check
2. `cargo clippy -p oxiraw -p oxiraw-cli -- -D warnings` — lint (deny warnings)
3. `cargo test -p oxiraw` — unit tests + architecture tests
4. `cargo test -p oxiraw-cli` — CLI integration tests
5. Doc link validation — checks README paths and design doc paths from ARCHITECTURE.md

Exits on first failure with a clear message. Exit code 0 = ready to merge.

Simple, readable bash. No framework, no dependencies beyond cargo and standard tools.

## Design Decisions

### Why threshold-based design docs (not always, not never)

Always requiring design docs adds overhead to trivial changes. Never requiring them means structural changes happen without documentation. The threshold (adds modules, changes deps, touches 3+ files) catches changes that meaningfully affect the architecture while letting small fixes move fast.

### Why a shell script (not a Makefile or cargo xtask)

A shell script is the simplest thing that works. Any agent can read and run it. No Makefile syntax to parse, no xtask crate to compile. If verification needs grow more complex, it can evolve into a Makefile or xtask later.

### Why doc link validation in the script

Broken links in ARCHITECTURE.md silently degrade the navigable map. Since the map is the core harness artifact, validating it mechanically catches drift before it accumulates. The check is cheap (just file existence) and fast.
