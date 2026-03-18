#!/usr/bin/env bash
set -euo pipefail

# Run the full end-to-end test suite.
# This is separate from verify.sh to keep the fast path fast.
#
# Usage: ./scripts/e2e.sh
# To regenerate golden files: GOLDEN_UPDATE=1 ./scripts/e2e.sh

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

echo "=== Building agx-cli (release) ==="
cargo build --release -p agx-cli

echo ""
echo "=== E2E Tests (cargo test -p agx-e2e) ==="
cargo test -p agx-e2e -- --include-ignored

echo ""
echo "E2E PASSED"
