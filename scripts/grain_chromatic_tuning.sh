#!/usr/bin/env bash
set -euo pipefail

# Chromatic grain tuning — renders all 6 grain types at 3 chromatic scale levels.
# Modifies grain.rs chromatic constants, rebuilds, renders, restores.

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

GRAIN_RS="crates/agx/src/adjust/grain.rs"
OUTDIR="test_output/chromatic_tuning"
mkdir -p "$OUTDIR"

# Use colorful image (vivid pinks/greens) to see chromatic shifts
INPUT="crates/agx-e2e/fixtures/jpeg/temple_blossoms.jpg"

# Grain types, const names, and base chromatic values (parallel arrays)
TYPES=(fine silver soft tabular cubic harsh)
CONST_NAMES=(CHROMATIC_FINE CHROMATIC_SILVER CHROMATIC_SOFT CHROMATIC_TABULAR CHROMATIC_CUBIC CHROMATIC_HARSH)
BASE_VALUES=(0.03 0.05 0.05 0.08 0.12 0.15)

# Scale factors to test
SCALES=(0.5 1.0 2.0)

# Save original
cp "$GRAIN_RS" "$GRAIN_RS.bak"
trap 'cp "$GRAIN_RS.bak" "$GRAIN_RS"; rm -f "$GRAIN_RS.bak"' EXIT

echo "=== Chromatic Grain Tuning ==="
echo "Image: temple_blossoms.jpg"
echo "Types: ${TYPES[*]}"
echo "Scales: ${SCALES[*]}"
echo "Grain: amount=60, size=30 (moderate grain for visibility)"
echo "Output: $OUTDIR/"
echo ""

# First build with default values
cargo build --release -p agx-cli 2>&1 | grep -v "Compiling\|Finished\|warning" || true

count=0
total=$(( ${#TYPES[@]} * ${#SCALES[@]} ))

for scale in "${SCALES[@]}"; do
  # Compute scaled values and patch all constants
  cp "$GRAIN_RS.bak" "$GRAIN_RS"
  for i in "${!TYPES[@]}"; do
    base="${BASE_VALUES[$i]}"
    scaled=$(echo "$base * $scale" | bc -l | sed 's/^\./0./')
    const_name="${CONST_NAMES[$i]}"
    sed -i '' "s/const ${const_name}: f32 = [0-9.]*;/const ${const_name}: f32 = ${scaled};/" "$GRAIN_RS"
  done

  # Rebuild once per scale level
  cargo build --release -p agx-cli 2>&1 | grep -v "Compiling\|Finished\|warning" || true

  for i in "${!TYPES[@]}"; do
    t="${TYPES[$i]}"
    count=$((count + 1))
    scale_int=$(printf "%.0f" "$(echo "$scale * 10" | bc)")
    outfile="$OUTDIR/${t}_scale${scale_int}.png"
    echo "[$count/$total] $t @ ${scale}x -> $outfile"
    target/release/agx-cli edit --input "$INPUT" --output "$outfile" \
      --grain-type "$t" \
      --grain-amount 60 \
      --grain-size 30 2>&1 | grep -v "^$" || true
  done
done

echo ""
echo "=== Done: $count renders ==="
echo ""
echo "Compare: {type}_scale5.png (0.5x) vs {type}_scale10.png (1.0x) vs {type}_scale20.png (2.0x)"
echo "Look for: subtle color shifts on saturated areas, no RGB confetti, BW areas unaffected"
