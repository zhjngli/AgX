#!/usr/bin/env bash
set -euo pipefail

# Profile GPU vs CPU render performance.
# Compares: Rust CPU pipeline, wgpu hardware GPU, and wgpu software fallback.
#
# On machines with a real GPU, all three paths are profiled.
# On CI (no GPU hardware), wgpu uses Mesa llvmpipe as a software adapter —
# this tells us how the WGSL shaders perform on CPU vs the native Rust code.
#
# Usage: ./scripts/gpu-profile.sh
# Requires: gpu and profiling features (both enabled automatically)

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

echo "=== Building with gpu + profiling features (release) ==="
cargo build --release -p agx-photo --features gpu,profiling

echo ""
echo "=== GPU Adapter Probe ==="
cargo test --release --features gpu,profiling -p agx-photo --test gpu_profiling \
    -- probe_adapter_info --nocapture 2>&1 | grep -E "^\s+(Primary|Fallback|No|Driver|Type|Max)" || true

echo ""
echo "=== Profiling: all stages (1024x768) ==="
cargo test --release --features gpu,profiling -p agx-photo --test gpu_profiling \
    -- profile_all_stages --nocapture --exact 2>&1 | grep -E "^\s+(CPU|GPU|Fallback|===|Speedup|gpu_|linear|gamma|dehaze|denoise|detail|grain|vignette|srgb|per_pixel|white_balance|TOTAL)" || true

echo ""
echo "=== Profiling: large image all stages (4000x3000) ==="
cargo test --release --features gpu,profiling -p agx-photo --test gpu_profiling \
    -- profile_large_image_all_stages --nocapture --exact 2>&1 | grep -E "^\s+(CPU|GPU|Fallback|===|Speedup|gpu_|linear|gamma|dehaze|denoise|detail|grain|vignette|srgb|per_pixel|white_balance|TOTAL)" || true

echo ""
echo "=== Profiling: fallback comparison ==="
cargo test --release --features gpu,profiling -p agx-photo --test gpu_profiling \
    -- profile_fallback_all_stages --nocapture --exact 2>&1 | grep -E "^\s+(CPU|GPU|Fallback|===|Speedup|gpu_|linear|gamma|dehaze|denoise|detail|grain|vignette|srgb|per_pixel|white_balance|TOTAL|adapter)" || true

echo ""
echo "GPU PROFILING COMPLETE"
