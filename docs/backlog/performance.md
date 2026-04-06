# Performance Optimizations

Render pipeline performance improvements prioritized by profiling data. The profiling infrastructure is in place (feature-gated, zero overhead in production) and should be re-run after each optimization to measure gains and re-prioritize.

**Profiling baseline (2026-04-01):** [Render Performance Analysis](../plans/2026-04-01-render-performance-analysis.md)

**26MP render times by preset type:**

| Preset type | Baseline | Top bottlenecks |
|-------------|----------|-----------------|
| Heavy (dehaze+detail+grain) | 15-17.5s | dehaze 27%, per_pixel_adjustments 29%, grain 17-19% |
| Detail-heavy (detail+grain) | 13-16.5s | detail 33-41%, per_pixel_adjustments 22-28%, grain 17-21% |
| Denoise (denoise+grain) | 11-15s | denoise 24-32%, per_pixel_adjustments 26-33%, grain 19-24% |
| Light (grain only) | 9-11s | per_pixel_adjustments 31-44%, grain 27-33% |
| Noop (decode+encode) | 4s | decode 64%, encode 15% |

## Sub-tasks

### Parallelization (data-driven priorities)

- [x] **P1: Parallelize per-pixel adjustment loop** — the `linear_to_srgb_and_per_pixel` loop is the most frequent bottleneck (27/32 combos, avg 31%). Each pixel is independent — use rayon `par_chunks_mut`. Estimated 3-5x speedup for this stage, saving 3-4s on heavy presets. Low complexity.
- [x] **P2: Parallelize Gaussian blur** — the separable blur in `detail.rs` is shared by detail, grain, and dehaze. Horizontal pass parallelized by rows, vertical by columns. Estimated 3-5x speedup, saving 2-5s depending on active stages. Medium complexity.
- [x] **P3: Parallelize denoise wavelet passes** — a trous wavelet decomposition processes each pixel independently per iteration. 24-32% of total when active. Medium complexity.
- [x] **P4: Parallelize grain noise generation and application** — embarrassingly parallel. 17-33% of total. Low complexity. (Blur portion covered by P2.)
- [x] **P5: Parallelize dehaze** — dehaze has its own `dark_channel` (separable min filter), `box_filter_2d` (separable box filter), and `guided_filter` (6x box filter + pixel loops) that are all sequential. ~4.6-4.9s at 26MP, the biggest remaining single-stage bottleneck after P1-P4. Same separable row/column pattern as P2. Medium complexity.

### Advanced optimizations (consider after P1-P4)

- [ ] **P6: SIMD for per-pixel adjustments** — vectorize the inner per-pixel loop with explicit SIMD. Additional 2-4x on top of parallelization. High complexity (sRGB gamma `pow` needs fast approximation).
- [ ] **P7: GPU acceleration (compute shaders)** — offload per-pixel and buffer ops to GPU via wgpu. 10-100x potential. Very high complexity (wgpu dependency, shader compilation, fallback path).

### Memory and buffer optimizations

- [ ] **Batch memory pressure with stage-based pipeline** — the pluggable pipeline always materializes intermediate buffers between stages (~300MB per buffer at 26MP). For batch workflows processing many large images in parallel, peak memory could become a bottleneck. Profile memory usage under batch load and consider strategies: buffer pooling, limiting concurrent large-image renders, or lazy buffer allocation. **Note:** P3 parallel channel denoising triples peak memory during wavelet decomposition (~1.8GB vs ~600MB sequential at 26MP). **Note:** `batch-apply --jobs N` clones the decoded image per concurrent render (~300MB/clone at 26MP); `--jobs 11` peaks at ~3.6GB. See [Batch Apply design](../plans/2026-04-05-multi-apply-e2e-speed-design.md). Both should be included in batch memory profiling. **Note:** P5 dehaze parallelization allocates a per-thread `col_buf` for vertical passes (~100KB each at 26MP, negligible) — no meaningful peak memory increase.
- [ ] Decode buffer reduction — convert sRGB-to-linear in-place instead of allocating an intermediate buffer (~1 buffer saved)
- [ ] Encode buffer reduction — go directly from linear f32 to u8 sRGB in a single pass (~1-2 buffers saved)

### Code quality

- [ ] Consolidate dual `BatchOpts` — CLI has `BatchOpts` (clap args) and `batch::BatchOpts` (internal). Could unify.

## How to Re-Profile

Re-profile after each optimization to measure actual gains and decide whether to continue down the priority list.

```bash
# 1. Build with profiling enabled
cargo build --release --features profiling -p agx-cli

# 2. Run the full test matrix (5 images x 6 presets x 3 reps)
./scripts/profile.sh

# 3. Generate the summary with median timings and bottleneck analysis
./scripts/profile_summary.sh
```

The scripts output to `profile_results.json` (gitignored). Compare against the baseline in the [analysis doc](../plans/2026-04-01-render-performance-analysis.md) to measure improvement. Update the baseline table above when a new round of profiling is done.

**Design docs:**
- [Profiling infrastructure design](../plans/2026-03-31-render-performance-profiling-design.md)
- [Render performance analysis](../plans/2026-04-01-render-performance-analysis.md)

## Considerations

- `rayon` is already a dependency of `agx-cli` (for batch parallelism). Adding it to the `agx` library crate would make it a hard dependency — consider a feature flag so library consumers can opt out.
- Batch processing already parallelizes across images. Per-image parallelization (P1-P4) helps most for few-large-image workloads. For many-small-image workloads, cross-image parallelism may already saturate cores.
- P1 and P2 together should reduce typical 26MP render times by 50-60%. Re-profile before deciding whether P3-P6 are worth the complexity.
