#!/usr/bin/env bash
set -euo pipefail

# Quick e2e smoke test for local development.
# Runs a subset: 1 JPEG image matrix, error cases, and library tests.
# Skips RAW tests (slow decode) — those run in the full suite.
# ~15s vs ~3min for the full suite.
#
# Usage: ./scripts/e2e-quick.sh
# Full suite: ./scripts/e2e.sh

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

echo "=== Building agx-cli (release) ==="
cargo build --release -p agx-cli

echo ""
echo "=== E2E Quick: JPEG matrix (temple_blossoms) ==="
cargo test -p agx-e2e --release -- cli_temple_blossoms --test-threads=4

echo ""
echo "=== E2E Quick: error cases + batch ==="
cargo test -p agx-e2e --release -- "cli_corrupt|cli_nonexistent|cli_batch" --test-threads=4

echo ""
echo "=== E2E Quick: library pipeline ==="
cargo test -p agx-e2e --release -- library_ --test-threads=4

echo ""
echo "E2E QUICK PASSED (subset — run ./scripts/e2e.sh for full matrix)"
