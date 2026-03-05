# Agent Developer Loop Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Encode a complete developer workflow into the repo so any AI agent knows the full design-to-merge cycle.

**Architecture:** Three deliverables — procedural Developer Workflow section in CLAUDE.md, detailed reference in `docs/contributing/developer-workflow.md`, and a `scripts/verify.sh` that runs all checks in one command. Follows the navigable map pattern: CLAUDE.md has the steps, contributing docs have the depth, scripts have the automation.

**Tech Stack:** Bash (verify script), Markdown (docs).

**Design doc:** `docs/plans/2026-03-05-developer-loop-design.md`

---

### Task 1: Create the verification script

Build `scripts/verify.sh` first — the other deliverables reference it.

**Files:**
- Create: `scripts/verify.sh`

**Step 1: Write the verification script**

```bash
#!/usr/bin/env bash
set -euo pipefail

# Verification script for oxiraw.
# Run this before considering work done. Exit code 0 = all checks pass.
#
# Usage: ./scripts/verify.sh

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

passed=0
failed=0

run_check() {
    local name="$1"
    shift
    echo ""
    echo "=== $name ==="
    if "$@"; then
        echo "--- $name: PASSED ---"
        ((passed++))
    else
        echo "--- $name: FAILED ---"
        ((failed++))
        echo ""
        echo "VERIFICATION FAILED at: $name"
        echo "Fix the issue above and re-run ./scripts/verify.sh"
        exit 1
    fi
}

# 1. Format check
run_check "Format (cargo fmt)" cargo fmt --check

# 2. Lint check
run_check "Lint (cargo clippy)" cargo clippy -p oxiraw -p oxiraw-cli -- -D warnings

# 3. Library tests (unit + architecture)
run_check "Library tests (cargo test -p oxiraw)" cargo test -p oxiraw

# 4. CLI tests
run_check "CLI tests (cargo test -p oxiraw-cli)" cargo test -p oxiraw-cli

# 5. Documentation link validation
check_doc_links() {
    local errors=0
    local arch="ARCHITECTURE.md"

    if [ ! -f "$arch" ]; then
        echo "ERROR: ARCHITECTURE.md not found"
        return 1
    fi

    # Check per-module README links
    while IFS= read -r line; do
        # Extract markdown link paths like [text](path)
        if [[ "$line" =~ \]\(([^)]+README\.md)\) ]]; then
            local path="${BASH_REMATCH[1]}"
            if [ ! -f "$path" ]; then
                echo "ERROR: Missing README: $path (referenced in ARCHITECTURE.md)"
                ((errors++))
            fi
        fi
    done < "$arch"

    # Check design doc links
    while IFS= read -r line; do
        if [[ "$line" =~ \]\((docs/plans/[^)]+\.md)\) ]]; then
            local path="${BASH_REMATCH[1]}"
            if [ ! -f "$path" ]; then
                echo "ERROR: Missing design doc: $path (referenced in ARCHITECTURE.md)"
                ((errors++))
            fi
        fi
    done < "$arch"

    # Check contributing doc links
    while IFS= read -r line; do
        if [[ "$line" =~ \]\((docs/contributing/[^)]+\.md)\) ]]; then
            local path="${BASH_REMATCH[1]}"
            if [ ! -f "$path" ]; then
                echo "ERROR: Missing contributing doc: $path (referenced in ARCHITECTURE.md)"
                ((errors++))
            fi
        fi
    done < "$arch"

    if [ "$errors" -gt 0 ]; then
        echo "$errors broken link(s) found in ARCHITECTURE.md"
        return 1
    fi

    echo "All documentation links valid"
    return 0
}
run_check "Documentation links" check_doc_links

# Summary
echo ""
echo "======================================="
echo "ALL CHECKS PASSED ($passed/$passed)"
echo "======================================="
```

**Step 2: Make it executable and test it**

Run: `chmod +x scripts/verify.sh && ./scripts/verify.sh`

Expected: All 5 checks pass (format, lint, library tests, CLI tests, doc links).

If `cargo fmt --check` fails, run `cargo fmt` first, then re-run the script. If clippy fails, fix the warnings. These are pre-existing issues unrelated to this task.

**Step 3: Commit**

```bash
git add scripts/verify.sh
git commit -m "build: add verification script (tests, lint, format, doc links)"
```

---

### Task 2: Create the developer workflow guide

**Files:**
- Create: `docs/contributing/developer-workflow.md`

**Step 1: Write the developer workflow guide**

This is the detailed reference for the Developer Workflow section in CLAUDE.md. Content:

```markdown
# Developer Workflow

This document describes the full development cycle for oxiraw. Follow these steps when implementing features, fixing bugs, or making structural changes. The summary version lives in `CLAUDE.md`; this document has the detail.

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
- Commit messages follow conventional format: `feat:`, `fix:`, `refactor:`, `test:`, `docs:`, `build:`
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
| Lint | `cargo clippy -p oxiraw -p oxiraw-cli -- -D warnings` | Code quality issues, unused imports, type errors |
| Library tests | `cargo test -p oxiraw` | Unit test failures, architecture violations |
| CLI tests | `cargo test -p oxiraw-cli` | Integration test failures |
| Doc links | Custom check | Broken links in ARCHITECTURE.md |

The script exits on first failure with a message about what went wrong.

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
| Wrote a design doc | ARCHITECTURE.md: Design Docs table |
| Changed how to extend a module | Module README: Extension Guide section |

## 5. Self-Review

Before declaring done, re-read your diff and ask:

- **Completeness**: Did I implement everything that was asked? Did I miss edge cases?
- **Scope**: Did I add anything that wasn't asked for? Did I over-engineer?
- **Tests**: Do my tests verify behavior (not just exercise code paths)? Would a bug in my code actually cause a test to fail?
- **Naming**: Are function/type/variable names clear? Do they describe what, not how?
- **Contracts**: Does my code respect the module's negative constraints (Does NOT section in README)?
- **Architecture**: Did I introduce any cross-module dependencies? If so, are they allowed by the rules in ARCHITECTURE.md?
```

**Step 2: Commit**

```bash
git add docs/contributing/developer-workflow.md
git commit -m "docs: add developer workflow guide"
```

---

### Task 3: Update CLAUDE.md with Developer Workflow

**Files:**
- Modify: `CLAUDE.md`

**Step 1: Add Developer Workflow section and simplify Definition of Done**

Replace the current `CLAUDE.md` content with:

```markdown
# Oxiraw

Open-source photo editing library + CLI in Rust with a portable, human-readable preset format.

## Workspace Layout

Cargo workspace with two crates:
- `crates/oxiraw/` -- core library (decode, engine, adjustments, presets, encode)
- `crates/oxiraw-cli/` -- thin CLI wrapper

## Architecture

See [`ARCHITECTURE.md`](ARCHITECTURE.md) for module dependency graph, dependency rules, core invariants, and per-module READMEs.

## Conventions

- Rust 2021 edition
- `thiserror` for error types
- `serde` for all serializable types
- Tests live alongside source in standard Rust `#[cfg(test)]` modules
- Structural tests in `crates/oxiraw/tests/architecture.rs` enforce module dependency rules; see "When a Structural Test Fails" in `ARCHITECTURE.md`

## Developer Workflow

Follow this cycle for features and significant changes. See [`docs/contributing/developer-workflow.md`](docs/contributing/developer-workflow.md) for detailed guidance.

### 1. Design (if needed)
Write a design doc in `docs/plans/` when the change adds/modifies modules, changes dependencies, or touches 3+ files. Skip for bug fixes, single-file changes, and doc updates.

### 2. Implement
Work on a feature branch (`feat/`, `fix/`, `refactor/`). Write tests alongside code. Follow module contracts in per-module READMEs. Commit incrementally.

### 3. Verify
Run `./scripts/verify.sh`. This runs format check, clippy, all tests (unit + architecture + CLI), and doc link validation.

### 4. Document
Update `ARCHITECTURE.md` if modules, dependencies, or invariants changed. Update affected module READMEs (public API, extension guide). Cross-link any new design docs.

### 5. Self-review
Re-read the diff. Check: Did I implement what was asked? Did I add anything extra? Do tests verify behavior?

## Definition of Done

Before merging, verify:
1. `./scripts/verify.sh` passes
2. `ARCHITECTURE.md` updated if modules, dependencies, or invariants changed
3. Affected module `README.md` files updated
4. Design doc cross-linked from `ARCHITECTURE.md` (if applicable)

## Key Docs

- [`ARCHITECTURE.md`](ARCHITECTURE.md) -- module layering, dependency rules, invariants
- [`docs/plans/`](docs/plans/) -- design and implementation plans
- [`docs/ideas/`](docs/ideas/) -- future feature ideas
- [`docs/contributing/`](docs/contributing/) -- developer workflow, evolving architecture
```

**Step 2: Commit**

```bash
git add CLAUDE.md
git commit -m "docs: add Developer Workflow to CLAUDE.md"
```

---

### Task 4: Update ARCHITECTURE.md with design doc link

**Files:**
- Modify: `ARCHITECTURE.md`

**Step 1: Add the developer loop design doc to the Design Docs table**

In `ARCHITECTURE.md`, add this row to the Plans table (after the Harness Engineering Implementation row):

```
| 2026-03-05 | [Developer Loop Design](docs/plans/2026-03-05-developer-loop-design.md)                          |
```

**Step 2: Commit**

```bash
git add ARCHITECTURE.md
git commit -m "docs: add developer loop design doc link to ARCHITECTURE.md"
```

---

### Task 5: Run verification and confirm everything works

**Step 1: Run the verification script**

Run: `./scripts/verify.sh`

Expected: All 5 checks pass — format, lint, library tests (95 + 6 architecture), CLI tests (7), doc links.

**Step 2: Verify the developer workflow is discoverable**

Confirm the following navigation works:
1. `CLAUDE.md` → Developer Workflow section exists with 5 steps
2. `CLAUDE.md` → links to `docs/contributing/developer-workflow.md` → file exists
3. `CLAUDE.md` → mentions `scripts/verify.sh` → file exists and is executable
4. `CLAUDE.md` → Definition of Done references `scripts/verify.sh`
5. `ARCHITECTURE.md` → Design Docs table includes the developer loop design doc → file exists

No commit needed — this is verification only.
