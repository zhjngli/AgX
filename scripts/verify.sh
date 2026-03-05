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

    # Regex patterns stored in variables to avoid bash parsing issues
    local readme_re='\]\(([^)]+README\.md)\)'
    local plans_re='\]\((docs/plans/[^)]+\.md)\)'
    local contrib_re='\]\((docs/contributing/[^)]+\.md)\)'

    # Check per-module README links
    while IFS= read -r line; do
        if [[ "$line" =~ $readme_re ]]; then
            local path="${BASH_REMATCH[1]}"
            if [ ! -f "$path" ]; then
                echo "ERROR: Missing README: $path (referenced in ARCHITECTURE.md)"
                errors=$((errors + 1))
            fi
        fi
    done < "$arch"

    # Check design doc links
    while IFS= read -r line; do
        if [[ "$line" =~ $plans_re ]]; then
            local path="${BASH_REMATCH[1]}"
            if [ ! -f "$path" ]; then
                echo "ERROR: Missing design doc: $path (referenced in ARCHITECTURE.md)"
                errors=$((errors + 1))
            fi
        fi
    done < "$arch"

    # Check contributing doc links
    while IFS= read -r line; do
        if [[ "$line" =~ $contrib_re ]]; then
            local path="${BASH_REMATCH[1]}"
            if [ ! -f "$path" ]; then
                echo "ERROR: Missing contributing doc: $path (referenced in ARCHITECTURE.md)"
                errors=$((errors + 1))
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
