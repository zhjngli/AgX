#!/usr/bin/env bash
set -euo pipefail

# Verification script for agx.
# Run this before considering work done. Exit code 0 = all checks pass.
#
# Usage:
#   ./scripts/verify.sh              # run all checks (default)
#   ./scripts/verify.sh <check>      # run a single named check
#
# Named checks (used by CI to parallelize):
#   fmt            cargo fmt --check
#   clippy         cargo clippy -D warnings
#   test-lib       cargo test -p agx
#   test-cli       cargo test -p agx-cli
#   test-features  feature-gated tests (docgen, raw)
#   rustdoc        cargo doc with warnings-as-errors
#   doc-links      markdown link validation

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

# --- Markdown link checker ---
# Scans .md files for markdown links to other .md files and verifies they exist.
# Links are resolved relative to the file's parent directory.
#
# Usage: check_md_links <file_or_dir> [<file_or_dir> ...]
check_md_links() {
    local errors=0
    local link_re='\[([^]]*)\]\(([^)]+\.md)\)'

    for target in "$@"; do
        local files=()
        if [ -d "$target" ]; then
            for f in "$target"/*.md; do
                [ -f "$f" ] && files+=("$f")
            done
        elif [ -f "$target" ]; then
            files+=("$target")
        else
            echo "ERROR: $target not found"
            errors=$((errors + 1))
            continue
        fi

        local fence_re='^[[:space:]]*```'
        for file in "${files[@]+"${files[@]}"}"; do
            local dir
            dir="$(dirname "$file")"
            local in_code_block=0
            while IFS= read -r line; do
                # Toggle fenced code block state and skip the fence line itself.
                # Markdown links inside fenced code blocks are example content,
                # not real links — don't validate them.
                if [[ "$line" =~ $fence_re ]]; then
                    in_code_block=$((1 - in_code_block))
                    continue
                fi
                if [ "$in_code_block" -eq 1 ]; then
                    continue
                fi
                local remaining="$line"
                while [[ "$remaining" =~ $link_re ]]; do
                    local link="${BASH_REMATCH[2]}"
                    # Skip external URLs
                    if [[ "$link" != http* ]]; then
                        local resolved="$dir/$link"
                        if [ ! -f "$resolved" ]; then
                            echo "ERROR: Broken link in $file: $link"
                            errors=$((errors + 1))
                        fi
                    fi
                    remaining="${remaining#*"${BASH_REMATCH[0]}"}"
                done
            done < "$file"
        done
    done

    if [ "$errors" -gt 0 ]; then
        echo "$errors broken link(s) found"
        return 1
    fi
    return 0
}

# --- Individual check implementations ---

check_fmt() {
    cargo fmt --check
}

check_clippy() {
    cargo clippy --workspace --all-targets -- -D warnings
}

check_test_lib() {
    cargo test -p agx
}

check_test_cli() {
    cargo test -p agx-cli
}

check_test_features() {
    cargo test -p agx --features docgen
    cargo test -p agx-docgen
    cargo test -p agx --features raw
}

check_rustdoc() {
    env RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace
}

check_doc_links() {
    # Check all committed markdown: root files, docs/ subdirs (excluding gitignored
    # docs/plans/impl/), and per-crate READMEs.
    local targets=(ARCHITECTURE.md CLAUDE.md README.md)
    for d in docs/*/; do
        [[ "$d" == "docs/plans/impl/" ]] && continue
        targets+=("$d")
    done
    # Per-crate and per-module READMEs
    while IFS= read -r f; do
        targets+=("$f")
    done < <(find crates -name "README.md" 2>/dev/null)
    check_md_links "${targets[@]}" || return 1
    echo "All documentation links valid"
}

# --- Single-check dispatch (used by CI for parallel runs) ---
if [ "$#" -gt 0 ]; then
    case "$1" in
        fmt)           check_fmt ;;
        clippy)        check_clippy ;;
        test-lib)      check_test_lib ;;
        test-cli)      check_test_cli ;;
        test-features) check_test_features ;;
        rustdoc)       check_rustdoc ;;
        doc-links)     check_doc_links ;;
        all)           ;;  # fall through to full run below
        *)
            echo "Unknown check: $1"
            echo "Valid checks: fmt, clippy, test-lib, test-cli, test-features, rustdoc, doc-links, all"
            exit 1
            ;;
    esac
    # For any single named check, exit after running it.
    [ "$1" != "all" ] && exit 0
fi

# --- Full run (default) ---

passed=0
failed=0

run_check() {
    local name="$1"
    shift
    echo ""
    echo "=== $name ==="
    if "$@"; then
        echo "--- $name: PASSED ---"
        passed=$((passed + 1))
    else
        echo "--- $name: FAILED ---"
        failed=$((failed + 1))
        echo ""
        echo "VERIFICATION FAILED at: $name"
        echo "Fix the issue above and re-run ./scripts/verify.sh"
        exit 1
    fi
}

# 1. Format check
run_check "Format (cargo fmt)" check_fmt

# 2. Lint check
run_check "Lint (cargo clippy)" check_clippy

# 3. Library tests (unit + architecture)
run_check "Library tests (cargo test -p agx)" check_test_lib

# 4. CLI tests
run_check "CLI tests (cargo test -p agx-cli)" check_test_cli

# 5. Rustdoc build (treats warnings as errors)
run_check "Rustdoc (cargo doc)" check_rustdoc

# 6. Documentation link validation
run_check "Documentation links" check_doc_links

# Summary
echo ""
echo "======================================="
echo "ALL CHECKS PASSED ($passed/$passed)"
echo "======================================="
