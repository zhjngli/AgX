# Batch Processing Design

**Date**: 2026-03-07
**Status**: Draft

## Overview

Add batch processing to oxiraw so users can apply the same preset or inline parameters to an entire folder of images in a single command. Processing uses rayon for CPU-level parallelism, with per-file error handling, progress reporting, and flexible output naming. This is a CLI-only feature — no library API changes to the core engine, decode, or encode modules.

## Motivation

The current CLI processes one image at a time. Photographers routinely apply the same look to hundreds of images from a shoot. Without batch processing, users must write shell loops (`for f in *.jpg; do oxiraw edit ...`), which:

- Processes images sequentially (no parallelism)
- Provides no aggregate progress or error summary
- Requires shell scripting knowledge
- Cannot easily handle output naming conventions

Batch processing is table-stakes for any photo editing CLI.

## CLI Design

Two new subcommands: `batch-apply` and `batch-edit`, mirroring the existing `apply` and `edit` subcommands but accepting a directory (or glob) instead of a single file.

### batch-apply

```bash
oxiraw batch-apply \
  --input-dir ./photos \
  --preset golden-hour.toml \
  --output-dir ./edited \
  --quality 92 \
  --format jpeg \
  --recursive \
  --jobs 4
```

### batch-edit

```bash
oxiraw batch-edit \
  --input-dir ./photos \
  --output-dir ./edited \
  --exposure 1.0 --contrast 25 --highlights -30 \
  --lut film.cube \
  --quality 92 \
  --jobs 0
```

### New flags (shared by both batch subcommands)

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--input-dir` | `PathBuf` | required | Directory containing input images |
| `--output-dir` | `PathBuf` | required | Directory for output images (created if missing) |
| `--recursive` / `-r` | `bool` | `false` | Recurse into subdirectories |
| `--jobs` / `-j` | `usize` | `0` | Number of parallel workers. `0` = number of CPU cores |
| `--skip-errors` | `bool` | `false` | Continue processing remaining files when one fails |
| `--suffix` | `Option<String>` | `None` | Append suffix to output filename (e.g., `_edited`) |

`batch-edit` inherits all the inline parameter flags from `edit` (exposure, contrast, HSL channels, etc.).

### Output naming

Output files mirror input filenames, placed in `--output-dir`:

```
input-dir/IMG_001.jpg  →  output-dir/IMG_001.jpg
input-dir/IMG_002.cr2  →  output-dir/IMG_002.jpg   (format converted)
input-dir/sub/IMG_003.jpg  →  output-dir/sub/IMG_003.jpg  (with --recursive)
```

With `--suffix _edited`:
```
input-dir/IMG_001.jpg  →  output-dir/IMG_001_edited.jpg
```

The output extension is determined by `--format` if specified, otherwise preserved from input (standard formats) or defaulted to `.jpg` (raw formats).

### File discovery

Scan `--input-dir` for files with recognized image extensions:

**Standard:** `.jpg`, `.jpeg`, `.png`, `.tiff`, `.tif`
**Raw (when `raw` feature enabled):** `.cr2`, `.cr3`, `.nef`, `.arw`, `.dng`, `.raf`, `.rw2`, `.orf`, `.pef`, `.srw`, `.dng`, and all others from `decode::is_raw_extension`

Skip non-image files silently. With `--recursive`, preserve subdirectory structure in output.

## Architecture

### No changes to core library modules

The batch feature lives entirely in `oxiraw-cli`. Each image goes through the same `decode → Engine → render → encode` pipeline that `apply` and `edit` use today. The engine is already stateless per-render (immutable original + parameters → output), so parallelism is straightforward.

### Module placement

All batch logic goes in `crates/oxiraw-cli/src/batch.rs`, imported from `main.rs`. This keeps `main.rs` from growing too large while maintaining the "thin CLI wrapper" principle.

```
crates/oxiraw-cli/src/
  main.rs       -- CLI parsing, subcommand dispatch
  batch.rs      -- NEW: file discovery, parallel execution, progress, error collection
```

### Parallelism model

Use `rayon::ThreadPoolBuilder` to create a pool with `--jobs` threads (or `num_cpus` if 0). Each image is an independent work unit dispatched via `rayon::scope` or `par_iter`:

```rust
use rayon::prelude::*;

let pool = rayon::ThreadPoolBuilder::new()
    .num_threads(jobs)
    .build()?;

let results: Vec<BatchResult> = pool.install(|| {
    image_paths
        .par_iter()
        .map(|path| process_single(path, &preset, &opts, &output_dir))
        .collect()
});
```

Each `process_single` call is fully independent — it decodes, renders, and encodes one image. No shared mutable state. The preset and LUT are loaded once and shared via `&` references (both are `Sync`).

### Memory considerations

Each image in-flight holds: decoded original (`Rgb32FImage`) + rendered output (`Rgb32FImage`) + encoded bytes. For a 24MP image, that's roughly 24M pixels * 3 channels * 4 bytes * 2 copies = ~576MB per image. With `--jobs 4`, peak memory is ~2.3GB for 24MP images.

The `--jobs` flag gives users direct control over the memory/parallelism tradeoff. The default of `num_cpus` is appropriate for most workstations. Users with limited RAM can set `--jobs 1` or `--jobs 2`.

### Error handling

Two modes controlled by `--skip-errors`:

**Default (fail-fast):** First error stops all processing. Rayon short-circuits remaining work. Exit code 1, error printed to stderr.

**With `--skip-errors`:** Each file's result is collected independently. At the end, print a summary:

```
Processed 47/50 images successfully.
Errors (3):
  photos/IMG_032.cr2: Decode error: unsupported raw format variant
  photos/corrupt.jpg: Decode error: invalid JPEG marker
  photos/huge.tiff: Encode error: output path already exists
```

Exit code 0 if all succeed, exit code 1 if any fail (even with `--skip-errors`), so scripts can detect partial failures.

### Progress reporting

Print per-file status to stderr (not stdout, to keep stdout clean for scripting):

```
[1/50] Processing IMG_001.jpg... done (1.2s)
[2/50] Processing IMG_002.jpg... done (1.1s)
[3/50] Processing IMG_003.cr2... FAILED: unsupported format
...
Batch complete: 47/50 succeeded in 28.3s
```

Progress lines from parallel workers are serialized via a simple `AtomicUsize` counter + `eprintln!`. Lines may interleave slightly under high parallelism — this is acceptable for a CLI tool.

## Dependencies

One new dependency for `oxiraw-cli`:

```toml
[dependencies]
rayon = "1"
```

Rayon is the standard Rust library for data parallelism. No changes to the core `oxiraw` crate dependencies.

## Implementation Plan

### Phase 1: File discovery and output naming

1. Add `batch.rs` module with `discover_images(dir, recursive) -> Vec<PathBuf>`
2. Add `resolve_output_path(input, input_dir, output_dir, suffix, format) -> PathBuf`
3. Unit tests for discovery (filters by extension) and path resolution (subdirs, suffix, format override)

### Phase 2: Sequential batch execution

4. Add `batch-apply` subcommand to clap `Commands` enum
5. Implement `run_batch_apply()` — sequential loop over discovered files, calling existing `run_apply` logic per file
6. Add `batch-edit` subcommand with all inline parameter flags
7. Implement `run_batch_edit()` — sequential loop, reusing `run_edit` logic
8. Progress reporting (counter + timing)
9. Error collection and summary printing

### Phase 3: Parallel execution

10. Add `rayon` dependency
11. Replace sequential loop with `par_iter` in a rayon thread pool
12. Wire `--jobs` flag to `ThreadPoolBuilder`
13. Test with multi-core execution (verify no data races, correct output)

### Phase 4: Polish

14. `--skip-errors` flag and partial failure exit codes
15. Create `--output-dir` if it doesn't exist
16. Validate inputs early (input-dir exists, preset exists, LUT exists) before starting batch
17. Integration test: batch-apply with example preset on example images

## Scope

**In scope:**
- `batch-apply` and `batch-edit` CLI subcommands
- File discovery by extension (with `--recursive`)
- Output naming (mirror structure, optional suffix, format override)
- Rayon-based parallelism with `--jobs` control
- Per-file error handling with `--skip-errors`
- Progress reporting to stderr
- `rayon` dependency for `oxiraw-cli` only

**Out of scope:**
- Changes to the core `oxiraw` library crate
- Async I/O (tokio) — rayon's CPU parallelism is sufficient; image processing is CPU-bound
- Glob/pattern matching for input files (use shell globs + `--input-dir` for now)
- Overwrite protection (can be added later; for now, silently overwrites)
- Watch mode / incremental processing
- Dry-run mode
- JSON/structured output

## Key Decisions

| Decision | Rationale |
|----------|-----------|
| Separate `batch-apply` / `batch-edit` subcommands (not flags on existing commands) | Keeps existing `apply`/`edit` clean. Batch has different required args (`--input-dir` vs `--input`). Follows the principle of explicit subcommands over modal flags. |
| Rayon, not tokio | Image processing is CPU-bound. Rayon's `par_iter` maps perfectly to independent per-image work. Tokio adds complexity for no benefit here — there's no async I/O to exploit. |
| CLI-only, no library batch API | The library's job is to process one image. Orchestration (file discovery, parallelism, progress) belongs in the application layer. Keeps the library focused. |
| `--jobs 0` means auto-detect | Follows convention from `make -j`, `cargo build -j`, `parallel`. Users who don't think about parallelism get good defaults. |
| Progress on stderr | Keeps stdout clean for piping/scripting. Standard Unix convention. |
| Default fail-fast, opt-in `--skip-errors` | Safe default — users see the first error immediately. For production pipelines, `--skip-errors` collects everything. |
| Mirror directory structure with `--recursive` | Intuitive: output tree matches input tree. No ambiguity about where files end up. |

## Related

- [Platform and Distribution](../ideas/platform-and-distribution.md) — batch processing listed as a distribution feature
- [Pluggable Pipeline](../ideas/pluggable-pipeline.md) — future stage caching could speed up batch with shared LUTs
