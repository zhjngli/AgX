#!/usr/bin/env bash
set -euo pipefail

cargo run -p agx-docgen

# mdbook-mermaid vendors mermaid.min.js + mermaid-init.js into docs/book/.
# These are gitignored, so install (or refresh) them on every build.
mdbook-mermaid install docs/book

mdbook build docs/book
