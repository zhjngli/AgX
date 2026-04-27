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
#   test-lib       cargo test -p agx-photo
#   test-cli       cargo test -p agx-cli
#   test-features  feature-gated tests (docgen, raw)
#   rustdoc        cargo doc with warnings-as-errors
#   doc-links      markdown link validation
#   book-linkcheck mdbook build with linkcheck backend enabled
#   external-links external URL validation (opt-in, requires lychee)
#   wgsl-headers   non-common WGSL header validation
#   markdown-lint  markdownlint-cli2 (style, structure, formatting)
#   back-links     verify every explanation wrapper has a "## See also" block
#   sibling-md-clean verify sibling .md files are link-free
#   book-no-internal-refs verify mdbook content does not link to docs/plans, docs/backlog, or docs/contributing

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
    cargo test -p agx-photo
}

check_test_cli() {
    cargo test -p agx-cli
}

check_test_features() {
    cargo test -p agx-photo --features docgen
    cargo test -p agx-docgen
    cargo test -p agx-photo --features raw
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
    # Per-crate READMEs plus per-module READMEs under crates/agx/src, including
    # contributor guides such as crates/agx/src/engine/gpu/README.md.
    while IFS= read -r f; do
        targets+=("$f")
    done < <(
        {
            find crates -name "README.md" 2>/dev/null
            find crates/agx/src -name "README.md" 2>/dev/null
        } | sort -u
    )
    check_md_links "${targets[@]}" || return 1
    echo "All documentation links valid"
}

check_book_linkcheck() {
    # Generate docgen output the book needs.
    cargo run -p agx-docgen
    # mdbook-mermaid's vendored JS assets are gitignored; refresh them.
    mdbook-mermaid install docs/book
    # Build the book with the linkcheck backend enabled.
    mdbook build docs/book
}

check_external_links() {
    if ! command -v lychee >/dev/null 2>&1; then
        echo "lychee not installed. Install with: cargo install lychee"
        return 1
    fi
    lychee --config .lychee.toml --verbose './**/*.md'
}

check_markdown_lint() {
    if ! command -v markdownlint-cli2 >/dev/null 2>&1; then
        if command -v npx >/dev/null 2>&1; then
            # Fall back to npx so contributors don't need a global install.
            npx --yes markdownlint-cli2
            return $?
        fi
        echo "markdownlint-cli2 not installed. Install with: npm install -g markdownlint-cli2"
        echo "(or install Node.js so verify.sh can fall back to 'npx markdownlint-cli2')"
        return 1
    fi
    markdownlint-cli2
}

check_back_links() {
    local errors=0
    for f in docs/book/src/explanation/*.md; do
        [ -f "$f" ] || continue
        case "$f" in
            */index.md) continue ;;
        esac
        if ! grep -q "^## See also" "$f"; then
            echo "ERROR: $f missing '## See also' block"
            errors=$((errors + 1))
        fi
    done

    if [ "$errors" -gt 0 ]; then
        echo "$errors explanation wrapper page(s) missing See also block"
        return 1
    fi

    echo "All explanation wrapper pages have See also blocks"
    return 0
}

check_sibling_md_clean() {
    local errors=0
    for f in crates/agx/src/adjust/*.md; do
        [ -f "$f" ] || continue
        # Match markdown links of the form [text](target). External
        # https:// links are allowed (per the umbrella initiative
        # convention); relative cross-references and plain http:// links
        # are forbidden because they don't render uniformly across
        # rustdoc and mdbook.
        local hits
        hits="$(grep -nE '\[[^]]+\]\([^)]+\)' "$f" | grep -vE '\]\(https://' || true)"
        if [ -n "$hits" ]; then
            echo "$hits" | while IFS= read -r line; do
                echo "ERROR: $f: $line"
            done
            errors=$((errors + 1))
        fi
    done

    if [ "$errors" -gt 0 ]; then
        echo "$errors sibling .md file(s) contain forbidden non-HTTPS links"
        return 1
    fi

    echo "All sibling .md files are link-free (or contain only HTTPS links)"
    return 0
}

check_book_no_internal_refs() {
    # The published mdbook site is end-user content. Internal planning
    # (docs/plans/), backlog (docs/backlog/), and contributor guides
    # (docs/contributing/) belong outside the published surface — readers
    # don't need them, and exposing them blurs the public/internal line.
    local hits
    hits="$(grep -rn -E '\]\([^)]*docs/(plans|backlog|contributing)/' docs/book/src/ || true)"
    if [ -n "$hits" ]; then
        echo "ERROR: published mdbook content links to internal docs (plans/backlog/contributing):"
        echo "$hits"
        return 1
    fi
    echo "No internal-doc links in published book content"
    return 0
}

check_wgsl_headers() {
    local errors=0
    local files=()

    while IFS= read -r file; do
        files+=("$file")
    done < <(find crates/agx/src/shaders -type f -name '*.wgsl' ! -path '*/common/*' | sort)

    for file in "${files[@]}"; do
        local header1 header2 header3 header4 header5
        header1="$(sed -n '1p' "$file")"
        header2="$(sed -n '2p' "$file")"
        header3="$(sed -n '3p' "$file")"
        header4="$(sed -n '4p' "$file")"
        header5="$(sed -n '5p' "$file")"

        if [ -z "$header5" ]; then
            echo "ERROR: Missing WGSL header lines in $file"
            errors=$((errors + 1))
            continue
        fi

        if [[ "$header1" != "// Algorithm: "* ]]; then
            echo "ERROR: Missing Algorithm header in $file"
            errors=$((errors + 1))
        fi
        if [[ "$header2" != "// Canonical explanation: "* ]]; then
            echo "ERROR: Missing Canonical explanation header in $file"
            errors=$((errors + 1))
        fi
        if [[ "$header3" != "// CPU equivalent: "* ]]; then
            echo "ERROR: Missing CPU equivalent header in $file"
            errors=$((errors + 1))
        fi
        if [[ "$header4" != "// Bindings: "* ]]; then
            echo "ERROR: Missing Bindings header in $file"
            errors=$((errors + 1))
        fi
        if [[ "$header5" != "// Entry points: main"* ]]; then
            echo "ERROR: Missing Entry points header in $file"
            errors=$((errors + 1))
        fi
    done

    if [ "$errors" -gt 0 ]; then
        echo "$errors WGSL header issue(s) found"
        return 1
    fi

    echo "All non-common WGSL headers valid"
    return 0
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
        book-linkcheck) check_book_linkcheck ;;
        external-links) check_external_links ;;
        wgsl-headers)  check_wgsl_headers ;;
        markdown-lint) check_markdown_lint ;;
        back-links)    check_back_links ;;
        sibling-md-clean) check_sibling_md_clean ;;
        book-no-internal-refs) check_book_no_internal_refs ;;
        all)           ;;  # fall through to full run below
        *)
            echo "Unknown check: $1"
            echo "Valid checks: fmt, clippy, test-lib, test-cli, test-features, rustdoc, doc-links, book-linkcheck, external-links, wgsl-headers, markdown-lint, back-links, sibling-md-clean, book-no-internal-refs, all"
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
run_check "Library tests (cargo test -p agx-photo)" check_test_lib

# 4. CLI tests
run_check "CLI tests (cargo test -p agx-cli)" check_test_cli

# 5. Rustdoc build (treats warnings as errors)
run_check "Rustdoc (cargo doc)" check_rustdoc

# 6. Documentation link validation
run_check "Documentation links" check_doc_links

# 7. Book link validation
run_check "Book linkcheck" check_book_linkcheck

# 8. WGSL header validation
run_check "WGSL headers" check_wgsl_headers

# 9. Markdown linting
run_check "Markdown lint" check_markdown_lint

# 10. Back-links (explanation -> concepts)
run_check "Back-links" check_back_links

# 11. Sibling .md cleanliness
run_check "Sibling .md cleanliness" check_sibling_md_clean

# 12. Book content has no links to internal planning/backlog/contributor docs
run_check "Book content surface boundary" check_book_no_internal_refs

# Summary
echo ""
echo "======================================="
echo "ALL CHECKS PASSED ($passed/$passed)"
echo "======================================="
