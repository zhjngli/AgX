#!/usr/bin/env bash
set -euo pipefail

# Grain tuning grid search — renders multiple images with different internal constants.
# Modifies grain.rs constants, rebuilds (incremental, ~3s), renders, restores.

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

GRAIN_RS="crates/agx/src/adjust/grain.rs"
OUTDIR="test_output/grain_tuning"

# Input images
INPUTS=(
  "crates/agx-e2e/fixtures/jpeg/temple_blossoms.jpg"
  "crates/agx-e2e/fixtures/jpeg/night_architecture.jpg"
  "crates/agx-e2e/fixtures/golden/raw/sunset_river_noop.png"
)
IMAGE_NAMES=("blossoms" "night" "sunset")

# Save original
cp "$GRAIN_RS" "$GRAIN_RS.bak"

# Restore on exit
trap 'cp "$GRAIN_RS.bak" "$GRAIN_RS"; rm -f "$GRAIN_RS.bak"' EXIT

# Parameter grid — strength and max_sigma
# Strength controls intensity (lower = more subtle grain)
# Max sigma controls particle size at size=100 (scaled by resolution)
MAX_SIGMAS=(2.0 2.5 3.0 4.0)
STRENGTHS=(0.08 0.12)

# Grain settings for the test render (amount raised for visibility during tuning)
GRAIN_TYPE="silver"
GRAIN_AMOUNT=50
GRAIN_SIZE=50

echo "=== Grain Tuning Grid Search (exp blending, white noise, res-scaled sigma) ==="
echo "Images: ${IMAGE_NAMES[*]}"
echo "Grain: type=$GRAIN_TYPE amount=$GRAIN_AMOUNT size=$GRAIN_SIZE"
echo "Output: $OUTDIR/"
echo ""

count=0
total=$(( ${#MAX_SIGMAS[@]} * ${#STRENGTHS[@]} ))
last_key=""

for ms in "${MAX_SIGMAS[@]}"; do
  for st in "${STRENGTHS[@]}"; do
    count=$((count + 1))
    ms_label=$(printf "%02.0f" "$(echo "$ms * 10" | bc)")
    st_label=$(printf "%03.0f" "$(echo "$st * 1000" | bc)")
    param_name="ms${ms_label}_st${st_label}"
    key="${ms}_${st}"

    echo "[$count/$total] $param_name (max_sigma=$ms, strength=$st)"

    # Only rebuild if constants changed
    if [[ "$key" != "$last_key" ]]; then
      cp "$GRAIN_RS.bak" "$GRAIN_RS"
      sed -i '' "s/const GRAIN_MAX_SIGMA: f32 = [0-9.]*;/const GRAIN_MAX_SIGMA: f32 = ${ms};/" "$GRAIN_RS"
      sed -i '' "s/const GRAIN_STRENGTH_MULT: f32 = [0-9.]*;/const GRAIN_STRENGTH_MULT: f32 = ${st};/" "$GRAIN_RS"
      cargo build --release -p agx-cli 2>&1 | grep -v "Compiling\|Finished\|warning" || true
      last_key="$key"
    fi

    # Render each image
    for i in "${!INPUTS[@]}"; do
      input="${INPUTS[$i]}"
      img="${IMAGE_NAMES[$i]}"
      outfile="$OUTDIR/${img}_${param_name}.png"
      target/release/agx-cli edit --input "$input" --output "$outfile" \
        --grain-type "$GRAIN_TYPE" \
        --grain-amount "$GRAIN_AMOUNT" \
        --grain-size "$GRAIN_SIZE" 2>&1 | grep -v "^$" || true
    done
    echo "  -> rendered ${#INPUTS[@]} images"
  done
done

echo ""
echo "=== Done: $count param sets × ${#INPUTS[@]} images = $((count * ${#INPUTS[@]})) renders ==="
echo ""
echo "File naming: {image}_{params}.png"
echo "  ms = max_sigma × 10   (higher = larger grain particles)"
echo "  st = strength_mult × 1000 (higher = more intense grain)"
echo ""
echo "Compare by rows (same image, different params) to find the best grain character."
