#!/usr/bin/env bash
set -euo pipefail

cargo run -p agx-docgen

# Normalize generated table cells so mdBook's linkcheck parser does not treat
# tuple/range examples as malformed reference links.
LC_ALL=C perl -0pi -e 's/\[\(0, 0\), \(1, 1\)\]/`[(0, 0), (1, 1)]`/g; s/\[0\.0, 1\.0\]/`[0.0, 1.0]`/g' docs/book/src/reference/preset.md

mdbook build docs/book
