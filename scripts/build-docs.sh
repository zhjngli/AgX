#!/usr/bin/env bash
set -euo pipefail

cargo run -p agx-docgen

mdbook build docs/book
