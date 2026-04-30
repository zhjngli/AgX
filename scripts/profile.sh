#!/usr/bin/env bash
set -euo pipefail

# Render performance profiling runner.
# Runs a matrix of images x presets x repetitions and collects JSON timing data.
#
# Usage: ./scripts/profile.sh [output_file] [repetitions]
#   output_file: JSON file to write results (default: profile_results.json)
#   repetitions: number of runs per combo (default: 3)

OUTPUT="${1:-profile_results.json}"
REPS="${2:-3}"
TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT

echo "=== Building agx-cli with profiling ==="
cargo build --release --features profiling -p agx-cli

BIN="./target/release/agx"
FIXTURES="crates/agx-e2e/fixtures"

# Test images — mix of RAW and JPEG at different sizes
IMAGES=(
    "$FIXTURES/raw/sunset_river.raf"
    "$FIXTURES/raw/dusk_cityscape.raf"
    "$FIXTURES/raw/foggy_forest.raf"
    "$FIXTURES/jpeg/temple_blossoms.jpg"
    "$FIXTURES/golden/raw/sunset_river_noop.png"
)

# Presets — from light to heavy pipeline usage
PRESETS=(
    "$FIXTURES/looks/portra_400.toml"
    "$FIXTURES/looks/cinema_warm.toml"
    "$FIXTURES/looks/blade_runner.toml"
    "$FIXTURES/looks/tri_x_400.toml"
    "$FIXTURES/looks/neo_noir.toml"
    "$FIXTURES/looks/dune.toml"
)

# Also run a noop (no preset) for baseline
NOOP_IMAGES=(
    "$FIXTURES/raw/sunset_river.raf"
    "$FIXTURES/golden/raw/sunset_river_noop.png"
)

# Clean output
rm -f "$OUTPUT"

total_combos=$(( ${#IMAGES[@]} * ${#PRESETS[@]} + ${#NOOP_IMAGES[@]} ))
echo "=== Running $total_combos combos x $REPS reps ==="

# Run preset combos
for img in "${IMAGES[@]}"; do
    for preset in "${PRESETS[@]}"; do
        img_name=$(basename "$img")
        preset_name=$(basename "$preset" .toml)
        echo "  $img_name + $preset_name ($REPS runs)..."
        for ((i=1; i<=REPS; i++)); do
            $BIN apply \
                --preset "$preset" \
                --input "$img" \
                --output "$TMPDIR/out.png" \
                --profile-output "$OUTPUT" \
                2>/dev/null
        done
    done
done

# Run noop baseline (edit with no adjustments)
for img in "${NOOP_IMAGES[@]}"; do
    img_name=$(basename "$img")
    echo "  $img_name + noop ($REPS runs)..."
    for ((i=1; i<=REPS; i++)); do
        $BIN edit \
            --input "$img" \
            --output "$TMPDIR/out.png" \
            --profile-output "$OUTPUT" \
            2>/dev/null
    done
done

echo "=== Done. Results in $OUTPUT ==="
echo "Run: ./scripts/profile_summary.sh $OUTPUT"
