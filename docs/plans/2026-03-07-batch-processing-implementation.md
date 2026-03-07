# Batch Processing Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add `batch-apply` and `batch-edit` CLI subcommands that process entire directories of images with rayon parallelism, per-file error handling, and progress reporting.

**Architecture:** CLI-only feature. All batch logic lives in `crates/oxiraw-cli/src/batch.rs`. No changes to the core `oxiraw` library crate. Each image goes through the existing `decode → Engine → render → encode` pipeline independently. Rayon `par_iter` handles parallelism.

**Tech Stack:** rayon 1, clap 4, std::sync::atomic (progress counter)

---

## Context

Design doc: `docs/plans/2026-03-07-batch-processing-design.md`

Key constraints:
- The `oxiraw` library crate is not modified — batch orchestration belongs in the CLI
- The `decode` module exposes `is_raw_extension()` and `RAW_EXTENSIONS` for file filtering
- `Preset` and `Lut3D` are `Sync + Send`, so they can be shared across rayon threads via `&`
- `Engine::new()` takes ownership of the decoded image, so each thread creates its own `Engine`

## Critical Files

| File | Purpose |
|------|---------|
| `crates/oxiraw-cli/src/batch.rs` | NEW: file discovery, output path resolution, parallel batch execution, progress, error collection |
| `crates/oxiraw-cli/src/main.rs` | Modify: add `BatchApply` and `BatchEdit` subcommands, dispatch to batch module |
| `crates/oxiraw-cli/Cargo.toml` | Modify: add `rayon = "1"` dependency |

---

## Phase 1: File Discovery and Output Path Resolution

### Task 1.1: Add `batch.rs` with `discover_images`

**Files:**
- Create: `crates/oxiraw-cli/src/batch.rs`
- Modify: `crates/oxiraw-cli/src/main.rs` (add `mod batch;`)

**Step 1: Write failing tests**

Create `batch.rs` with a `#[cfg(test)]` module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_files(dir: &std::path::Path, names: &[&str]) {
        for name in names {
            fs::write(dir.join(name), b"fake").unwrap();
        }
    }

    #[test]
    fn discover_finds_jpeg_files() {
        let dir = TempDir::new().unwrap();
        create_test_files(dir.path(), &["a.jpg", "b.jpeg", "c.txt", "d.png"]);
        let found = discover_images(dir.path(), false);
        assert_eq!(found.len(), 3); // jpg, jpeg, png
    }

    #[test]
    fn discover_skips_non_image_files() {
        let dir = TempDir::new().unwrap();
        create_test_files(dir.path(), &["readme.md", "notes.txt", ".hidden"]);
        let found = discover_images(dir.path(), false);
        assert!(found.is_empty());
    }

    #[test]
    fn discover_recursive_finds_subdirs() {
        let dir = TempDir::new().unwrap();
        let sub = dir.path().join("sub");
        fs::create_dir(&sub).unwrap();
        create_test_files(dir.path(), &["a.jpg"]);
        create_test_files(&sub, &["b.png"]);

        let flat = discover_images(dir.path(), false);
        assert_eq!(flat.len(), 1); // only top-level

        let recursive = discover_images(dir.path(), true);
        assert_eq!(recursive.len(), 2); // top + sub
    }

    #[test]
    fn discover_case_insensitive_extensions() {
        let dir = TempDir::new().unwrap();
        create_test_files(dir.path(), &["a.JPG", "b.Png", "c.TIFF"]);
        let found = discover_images(dir.path(), false);
        assert_eq!(found.len(), 3);
    }

    #[test]
    fn discover_sorted_by_name() {
        let dir = TempDir::new().unwrap();
        create_test_files(dir.path(), &["c.jpg", "a.jpg", "b.jpg"]);
        let found = discover_images(dir.path(), false);
        let names: Vec<_> = found.iter().map(|p| p.file_name().unwrap().to_str().unwrap()).collect();
        assert_eq!(names, vec!["a.jpg", "b.jpg", "c.jpg"]);
    }
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p oxiraw-cli batch::tests`
Expected: FAIL (module and function don't exist yet)

**Step 3: Write implementation**

Add `mod batch;` to `main.rs` (after the existing `use` statements, before `#[derive(Parser)]`).

Create `crates/oxiraw-cli/src/batch.rs`:

```rust
use std::path::{Path, PathBuf};

/// Standard image extensions (always recognized).
const STANDARD_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "tiff", "tif"];

/// Check if a file has a recognized image extension.
fn is_image_file(path: &Path) -> bool {
    let ext = match path.extension().and_then(|e| e.to_str()) {
        Some(e) => e.to_ascii_lowercase(),
        None => return false,
    };
    STANDARD_EXTENSIONS.contains(&ext.as_str()) || oxiraw::decode::is_raw_extension(path)
}

/// Discover image files in a directory, optionally recursing into subdirectories.
/// Returns paths sorted alphabetically for deterministic processing order.
pub fn discover_images(dir: &Path, recursive: bool) -> Vec<PathBuf> {
    let mut images = Vec::new();
    collect_images(dir, recursive, &mut images);
    images.sort();
    images
}

fn collect_images(dir: &Path, recursive: bool, out: &mut Vec<PathBuf>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() && recursive {
            collect_images(&path, true, out);
        } else if path.is_file() && is_image_file(&path) {
            out.push(path);
        }
    }
}
```

Add `tempfile` as a dev dependency to `crates/oxiraw-cli/Cargo.toml`:

```toml
[dev-dependencies]
tempfile = "3"
```

**Step 4: Run tests**

Run: `cargo test -p oxiraw-cli batch::tests`
Expected: PASS

**Step 5: Stage**

```bash
git add crates/oxiraw-cli/src/batch.rs crates/oxiraw-cli/src/main.rs crates/oxiraw-cli/Cargo.toml
git commit -m "feat: add batch file discovery with extension filtering and recursion"
```

---

### Task 1.2: Add `resolve_output_path`

**Files:**
- Modify: `crates/oxiraw-cli/src/batch.rs`

**Step 1: Write failing tests**

Add to the `tests` module in `batch.rs`:

```rust
#[test]
fn resolve_output_preserves_filename() {
    let result = resolve_output_path(
        Path::new("/photos/IMG_001.jpg"),
        Path::new("/photos"),
        Path::new("/edited"),
        None,
        None,
    );
    assert_eq!(result, PathBuf::from("/edited/IMG_001.jpg"));
}

#[test]
fn resolve_output_preserves_subdirectory() {
    let result = resolve_output_path(
        Path::new("/photos/day1/IMG_001.jpg"),
        Path::new("/photos"),
        Path::new("/edited"),
        None,
        None,
    );
    assert_eq!(result, PathBuf::from("/edited/day1/IMG_001.jpg"));
}

#[test]
fn resolve_output_applies_suffix() {
    let result = resolve_output_path(
        Path::new("/photos/IMG_001.jpg"),
        Path::new("/photos"),
        Path::new("/edited"),
        Some("_processed"),
        None,
    );
    assert_eq!(result, PathBuf::from("/edited/IMG_001_processed.jpg"));
}

#[test]
fn resolve_output_overrides_format() {
    let result = resolve_output_path(
        Path::new("/photos/IMG_001.png"),
        Path::new("/photos"),
        Path::new("/edited"),
        None,
        Some("jpeg"),
    );
    assert_eq!(result, PathBuf::from("/edited/IMG_001.jpeg"));
}

#[test]
fn resolve_output_raw_defaults_to_jpg() {
    let result = resolve_output_path(
        Path::new("/photos/IMG_001.cr2"),
        Path::new("/photos"),
        Path::new("/edited"),
        None,
        None,
    );
    assert_eq!(result, PathBuf::from("/edited/IMG_001.jpg"));
}

#[test]
fn resolve_output_suffix_plus_format() {
    let result = resolve_output_path(
        Path::new("/photos/IMG_001.cr2"),
        Path::new("/photos"),
        Path::new("/edited"),
        Some("_edited"),
        Some("tiff"),
    );
    assert_eq!(result, PathBuf::from("/edited/IMG_001_edited.tiff"));
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p oxiraw-cli batch::tests::resolve_output`
Expected: FAIL

**Step 3: Write implementation**

Add to `batch.rs`:

```rust
/// Resolve the output path for a batch-processed image.
///
/// - Mirrors subdirectory structure from `input_dir` into `output_dir`
/// - Appends optional suffix before the extension (e.g., `_edited`)
/// - Overrides extension if `format_ext` is provided
/// - Raw format inputs default to `.jpg` extension when no format override
pub fn resolve_output_path(
    input: &Path,
    input_dir: &Path,
    output_dir: &Path,
    suffix: Option<&str>,
    format_ext: Option<&str>,
) -> PathBuf {
    // Compute relative path from input_dir
    let relative = input.strip_prefix(input_dir).unwrap_or(input.file_name().map(Path::new).unwrap_or(input));

    // Determine output extension
    let ext = if let Some(fmt) = format_ext {
        fmt.to_string()
    } else if oxiraw::decode::is_raw_extension(input) {
        "jpg".to_string()
    } else {
        input
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("jpg")
            .to_string()
    };

    // Build output filename with optional suffix
    let stem = relative
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");

    let filename = match suffix {
        Some(s) => format!("{stem}{s}.{ext}"),
        None => format!("{stem}.{ext}"),
    };

    // Preserve subdirectory structure
    let parent = relative.parent().unwrap_or(Path::new(""));
    output_dir.join(parent).join(filename)
}
```

**Step 4: Run tests**

Run: `cargo test -p oxiraw-cli batch::tests`
Expected: PASS

**Step 5: Stage**

```bash
git add crates/oxiraw-cli/src/batch.rs
git commit -m "feat: add resolve_output_path for batch output naming"
```

---

## Phase 2: Batch Execution Core

### Task 2.1: Add rayon dependency and `BatchResult` type

**Files:**
- Modify: `crates/oxiraw-cli/Cargo.toml`
- Modify: `crates/oxiraw-cli/src/batch.rs`

**Step 1: Add rayon**

Add to `crates/oxiraw-cli/Cargo.toml` under `[dependencies]`:

```toml
rayon = "1"
```

**Step 2: Add result types and progress to `batch.rs`**

```rust
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

/// Result of processing a single image in a batch.
pub struct BatchResult {
    pub input: PathBuf,
    pub output: PathBuf,
    pub outcome: Result<Duration, String>,
}

/// Summary of a batch run.
pub struct BatchSummary {
    pub total: usize,
    pub succeeded: usize,
    pub failed: Vec<(PathBuf, String)>,
    pub elapsed: Duration,
}

/// Print progress for a completed image. Thread-safe via atomic counter.
fn report_progress(counter: &AtomicUsize, total: usize, input: &Path, outcome: &Result<Duration, String>) {
    let n = counter.fetch_add(1, Ordering::Relaxed) + 1;
    let name = input.file_name().and_then(|f| f.to_str()).unwrap_or("?");
    match outcome {
        Ok(dur) => eprintln!("[{n}/{total}] {name}... done ({:.1}s)", dur.as_secs_f64()),
        Err(e) => eprintln!("[{n}/{total}] {name}... FAILED: {e}"),
    }
}

/// Summarize batch results and print to stderr.
pub fn summarize(results: &[BatchResult], elapsed: Duration) -> BatchSummary {
    let total = results.len();
    let mut succeeded = 0;
    let mut failed = Vec::new();

    for r in results {
        match &r.outcome {
            Ok(_) => succeeded += 1,
            Err(e) => failed.push((r.input.clone(), e.clone())),
        }
    }

    eprintln!("\nBatch complete: {succeeded}/{total} succeeded in {:.1}s", elapsed.as_secs_f64());
    if !failed.is_empty() {
        eprintln!("Errors ({}):", failed.len());
        for (path, err) in &failed {
            eprintln!("  {}: {err}", path.display());
        }
    }

    BatchSummary { total, succeeded, failed, elapsed }
}
```

**Step 3: Stage**

```bash
git add crates/oxiraw-cli/Cargo.toml crates/oxiraw-cli/src/batch.rs
git commit -m "feat: add rayon dependency, BatchResult, progress reporting"
```

---

### Task 2.2: Implement `run_batch_apply`

**Files:**
- Modify: `crates/oxiraw-cli/src/batch.rs`

**Step 1: Write failing test**

Add to the `tests` module:

```rust
#[test]
fn batch_apply_processes_multiple_images() {
    let input_dir = TempDir::new().unwrap();
    let output_dir = TempDir::new().unwrap();

    // Create minimal valid PNG images
    for name in &["a.png", "b.png"] {
        let img: image::ImageBuffer<image::Rgb<u8>, Vec<u8>> =
            image::ImageBuffer::from_pixel(2, 2, image::Rgb([128u8, 64, 32]));
        img.save(input_dir.path().join(name)).unwrap();
    }

    // Create a minimal preset TOML
    let preset_path = input_dir.path().join("test.toml");
    fs::write(&preset_path, "[metadata]\nname = \"test\"\nversion = \"1.0\"\nauthor = \"test\"\n").unwrap();

    let summary = run_batch_apply(
        input_dir.path(),
        &preset_path,
        output_dir.path(),
        false,  // recursive
        92,     // quality
        None,   // format
        None,   // suffix
        1,      // jobs
        false,  // skip_errors
    );

    assert_eq!(summary.total, 2);
    assert_eq!(summary.succeeded, 2);
    assert!(summary.failed.is_empty());
    assert!(output_dir.path().join("a.png").exists());
    assert!(output_dir.path().join("b.png").exists());
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p oxiraw-cli batch::tests::batch_apply`
Expected: FAIL

**Step 3: Write implementation**

Add to `batch.rs`:

```rust
use rayon::prelude::*;

/// Process a single image with a preset (used by batch-apply).
fn process_apply_single(
    input: &Path,
    output: &Path,
    preset: &oxiraw::Preset,
    quality: u8,
    format: Option<oxiraw::encode::OutputFormat>,
) -> std::result::Result<Duration, String> {
    let start = Instant::now();
    let metadata = oxiraw::metadata::extract_metadata(input);
    let linear = oxiraw::decode::decode(input).map_err(|e| e.to_string())?;
    let mut engine = oxiraw::Engine::new(linear);
    engine.apply_preset(preset);
    let rendered = engine.render();
    let opts = oxiraw::encode::EncodeOptions {
        jpeg_quality: quality,
        format,
    };

    // Ensure parent directory exists
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    oxiraw::encode::encode_to_file_with_options(&rendered, output, &opts, metadata.as_ref())
        .map_err(|e| e.to_string())?;
    Ok(start.elapsed())
}

/// Run batch-apply: apply a preset to all images in a directory.
#[allow(clippy::too_many_arguments)]
pub fn run_batch_apply(
    input_dir: &Path,
    preset_path: &Path,
    output_dir: &Path,
    recursive: bool,
    quality: u8,
    format: Option<oxiraw::encode::OutputFormat>,
    suffix: Option<&str>,
    jobs: usize,
    skip_errors: bool,
) -> BatchSummary {
    let batch_start = Instant::now();

    let images = discover_images(input_dir, recursive);
    if images.is_empty() {
        eprintln!("No image files found in {}", input_dir.display());
        return BatchSummary {
            total: 0,
            succeeded: 0,
            failed: Vec::new(),
            elapsed: batch_start.elapsed(),
        };
    }

    let preset = match oxiraw::Preset::load_from_file(preset_path) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Failed to load preset: {e}");
            return BatchSummary {
                total: images.len(),
                succeeded: 0,
                failed: images.iter().map(|p| (p.clone(), format!("preset load failed: {e}"))).collect(),
                elapsed: batch_start.elapsed(),
            };
        }
    };

    let format_ext = format.map(|f| f.extension());
    let total = images.len();
    let counter = AtomicUsize::new(0);

    eprintln!("Processing {total} images with {} workers...", if jobs == 0 { rayon::current_num_threads() } else { jobs });

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(if jobs == 0 { 0 } else { jobs })
        .build()
        .expect("failed to create thread pool");

    let results: Vec<BatchResult> = pool.install(|| {
        images
            .par_iter()
            .map(|input| {
                let output = resolve_output_path(input, input_dir, output_dir, suffix, format_ext);
                let outcome = process_apply_single(input, &output, &preset, quality, format);
                report_progress(&counter, total, input, &outcome);

                BatchResult { input: input.clone(), output, outcome }
            })
            .collect()
    });

    let summary = summarize(&results, batch_start.elapsed());

    if !skip_errors && !summary.failed.is_empty() {
        // In fail-fast mode, we still process all (rayon doesn't support short-circuit easily),
        // but we report the failure clearly
    }

    summary
}
```

**Step 4: Run tests**

Run: `cargo test -p oxiraw-cli batch::tests`
Expected: PASS

**Step 5: Stage**

```bash
git add crates/oxiraw-cli/src/batch.rs
git commit -m "feat: implement run_batch_apply with rayon parallelism"
```

---

### Task 2.3: Implement `run_batch_edit`

**Files:**
- Modify: `crates/oxiraw-cli/src/batch.rs`

**Step 1: Write failing test**

Add to the `tests` module:

```rust
#[test]
fn batch_edit_processes_with_params() {
    let input_dir = TempDir::new().unwrap();
    let output_dir = TempDir::new().unwrap();

    let img: image::ImageBuffer<image::Rgb<u8>, Vec<u8>> =
        image::ImageBuffer::from_pixel(2, 2, image::Rgb([128u8, 64, 32]));
    img.save(input_dir.path().join("test.png")).unwrap();

    let params = oxiraw::Parameters::default();
    let summary = run_batch_edit(
        input_dir.path(),
        output_dir.path(),
        false,  // recursive
        &params,
        None,   // lut
        92,     // quality
        None,   // format
        None,   // suffix
        1,      // jobs
        false,  // skip_errors
    );

    assert_eq!(summary.total, 1);
    assert_eq!(summary.succeeded, 1);
    assert!(output_dir.path().join("test.png").exists());
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p oxiraw-cli batch::tests::batch_edit`
Expected: FAIL

**Step 3: Write implementation**

Add to `batch.rs`:

```rust
/// Process a single image with inline parameters (used by batch-edit).
fn process_edit_single(
    input: &Path,
    output: &Path,
    params: &oxiraw::Parameters,
    lut: Option<&oxiraw::Lut3D>,
    quality: u8,
    format: Option<oxiraw::encode::OutputFormat>,
) -> std::result::Result<Duration, String> {
    let start = Instant::now();
    let metadata = oxiraw::metadata::extract_metadata(input);
    let linear = oxiraw::decode::decode(input).map_err(|e| e.to_string())?;
    let mut engine = oxiraw::Engine::new(linear);
    engine.set_params(params.clone());
    if let Some(l) = lut {
        engine.set_lut(Some(l.clone()));
    }
    let rendered = engine.render();
    let opts = oxiraw::encode::EncodeOptions {
        jpeg_quality: quality,
        format,
    };

    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    oxiraw::encode::encode_to_file_with_options(&rendered, output, &opts, metadata.as_ref())
        .map_err(|e| e.to_string())?;
    Ok(start.elapsed())
}

/// Run batch-edit: apply inline parameters to all images in a directory.
#[allow(clippy::too_many_arguments)]
pub fn run_batch_edit(
    input_dir: &Path,
    output_dir: &Path,
    recursive: bool,
    params: &oxiraw::Parameters,
    lut: Option<&oxiraw::Lut3D>,
    quality: u8,
    format: Option<oxiraw::encode::OutputFormat>,
    suffix: Option<&str>,
    jobs: usize,
    skip_errors: bool,
) -> BatchSummary {
    let batch_start = Instant::now();

    let images = discover_images(input_dir, recursive);
    if images.is_empty() {
        eprintln!("No image files found in {}", input_dir.display());
        return BatchSummary {
            total: 0,
            succeeded: 0,
            failed: Vec::new(),
            elapsed: batch_start.elapsed(),
        };
    }

    let format_ext = format.map(|f| f.extension());
    let total = images.len();
    let counter = AtomicUsize::new(0);

    eprintln!("Processing {total} images with {} workers...", if jobs == 0 { rayon::current_num_threads() } else { jobs });

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(if jobs == 0 { 0 } else { jobs })
        .build()
        .expect("failed to create thread pool");

    let results: Vec<BatchResult> = pool.install(|| {
        images
            .par_iter()
            .map(|input| {
                let output = resolve_output_path(input, input_dir, output_dir, suffix, format_ext);
                let outcome = process_edit_single(input, &output, params, lut, quality, format);
                report_progress(&counter, total, input, &outcome);

                BatchResult { input: input.clone(), output, outcome }
            })
            .collect()
    });

    let summary = summarize(&results, batch_start.elapsed());
    summary
}
```

**Step 4: Run tests**

Run: `cargo test -p oxiraw-cli batch::tests`
Expected: PASS

**Step 5: Stage**

```bash
git add crates/oxiraw-cli/src/batch.rs
git commit -m "feat: implement run_batch_edit with rayon parallelism"
```

---

## Phase 3: CLI Integration

### Task 3.1: Add `BatchApply` subcommand to clap

**Files:**
- Modify: `crates/oxiraw-cli/src/main.rs`

**Step 1: Write implementation**

Add two new variants to the `Commands` enum:

```rust
/// Apply a TOML preset to all images in a directory
BatchApply {
    /// Input directory containing images
    #[arg(long)]
    input_dir: PathBuf,
    /// Preset TOML file path
    #[arg(short, long)]
    preset: PathBuf,
    /// Output directory (created if missing)
    #[arg(long)]
    output_dir: PathBuf,
    /// Recurse into subdirectories
    #[arg(short, long, default_value_t = false)]
    recursive: bool,
    /// Number of parallel workers (0 = auto-detect CPU cores)
    #[arg(short, long, default_value_t = 0)]
    jobs: usize,
    /// Continue processing when individual files fail
    #[arg(long, default_value_t = false)]
    skip_errors: bool,
    /// Append suffix to output filenames (e.g., _edited)
    #[arg(long)]
    suffix: Option<String>,
    /// JPEG output quality (1-100, default 92)
    #[arg(long, default_value_t = 92)]
    quality: u8,
    /// Output format (jpeg, png, tiff). Inferred from input if not specified.
    #[arg(long)]
    format: Option<String>,
},
```

Add the dispatch in `main()`:

```rust
Commands::BatchApply {
    input_dir,
    preset,
    output_dir,
    recursive,
    jobs,
    skip_errors,
    suffix,
    quality,
    format,
} => {
    let fmt = format.as_deref().map(parse_output_format).transpose()?;
    let summary = batch::run_batch_apply(
        &input_dir,
        &preset,
        &output_dir,
        recursive,
        quality,
        fmt,
        suffix.as_deref(),
        jobs,
        skip_errors,
    );
    if !summary.failed.is_empty() {
        process::exit(1);
    }
    Ok(())
}
```

**Step 2: Run tests**

Run: `cargo build -p oxiraw-cli`
Expected: PASS (compiles successfully)

**Step 3: Stage**

```bash
git add crates/oxiraw-cli/src/main.rs
git commit -m "feat: add batch-apply CLI subcommand"
```

---

### Task 3.2: Add `BatchEdit` subcommand to clap

**Files:**
- Modify: `crates/oxiraw-cli/src/main.rs`

**Step 1: Write implementation**

Add `BatchEdit` variant to `Commands` with all the same inline parameter flags as `Edit`, plus the batch-specific flags (`input_dir`, `output_dir`, `recursive`, `jobs`, `skip_errors`, `suffix`):

```rust
/// Edit all images in a directory with inline parameters
BatchEdit {
    /// Input directory containing images
    #[arg(long)]
    input_dir: PathBuf,
    /// Output directory (created if missing)
    #[arg(long)]
    output_dir: PathBuf,
    /// Recurse into subdirectories
    #[arg(short, long, default_value_t = false)]
    recursive: bool,
    /// Number of parallel workers (0 = auto-detect CPU cores)
    #[arg(short, long, default_value_t = 0)]
    jobs: usize,
    /// Continue processing when individual files fail
    #[arg(long, default_value_t = false)]
    skip_errors: bool,
    /// Append suffix to output filenames (e.g., _edited)
    #[arg(long)]
    suffix: Option<String>,

    // --- Same parameter flags as Edit ---
    /// Exposure in stops (-5.0 to +5.0)
    #[arg(long, default_value_t = 0.0, allow_hyphen_values = true)]
    exposure: f32,
    // ... (all other tone, white balance, HSL flags identical to Edit) ...

    /// Path to a .cube LUT file
    #[arg(long)]
    lut: Option<PathBuf>,
    /// JPEG output quality (1-100, default 92)
    #[arg(long, default_value_t = 92)]
    quality: u8,
    /// Output format (jpeg, png, tiff). Inferred from input if not specified.
    #[arg(long)]
    format: Option<String>,
}
```

Add the dispatch in `main()`. Build `Parameters` and `HslChannels` from the CLI args (same pattern as `Edit`), then call:

```rust
Commands::BatchEdit { input_dir, output_dir, recursive, jobs, skip_errors, suffix,
    exposure, contrast, highlights, shadows, whites, blacks, temperature, tint,
    lut, quality, format,
    hsl_red_hue, hsl_red_saturation, hsl_red_luminance,
    /* ... all HSL fields ... */
} => {
    let hsl = oxiraw::engine::HslChannels { /* ... build from args ... */ };
    let mut params = oxiraw::Parameters::default();
    params.exposure = exposure;
    params.contrast = contrast;
    params.highlights = highlights;
    params.shadows = shadows;
    params.whites = whites;
    params.blacks = blacks;
    params.temperature = temperature;
    params.tint = tint;
    params.hsl = hsl;

    let lut_data = lut.as_deref()
        .map(oxiraw::Lut3D::from_cube_file)
        .transpose()?;
    let fmt = format.as_deref().map(parse_output_format).transpose()?;

    let summary = batch::run_batch_edit(
        &input_dir,
        &output_dir,
        recursive,
        &params,
        lut_data.as_ref(),
        quality,
        fmt,
        suffix.as_deref(),
        jobs,
        skip_errors,
    );
    if !summary.failed.is_empty() {
        process::exit(1);
    }
    Ok(())
}
```

**Step 2: Run tests**

Run: `cargo build -p oxiraw-cli`
Expected: PASS

**Step 3: Stage**

```bash
git add crates/oxiraw-cli/src/main.rs
git commit -m "feat: add batch-edit CLI subcommand"
```

---

## Phase 4: Integration Tests

### Task 4.1: CLI integration tests for batch subcommands

**Files:**
- Modify or create: `crates/oxiraw-cli/tests/integration.rs` (or add to existing test file)

**Step 1: Write tests**

```rust
#[test]
fn cli_batch_apply_with_example_preset() {
    let temp_in = tempfile::TempDir::new().unwrap();
    let temp_out = tempfile::TempDir::new().unwrap();

    // Create test images
    for name in &["img1.png", "img2.png"] {
        let img: image::ImageBuffer<image::Rgb<u8>, Vec<u8>> =
            image::ImageBuffer::from_pixel(4, 4, image::Rgb([100u8, 150, 200]));
        img.save(temp_in.path().join(name)).unwrap();
    }

    // Write a minimal preset
    let preset = temp_in.path().join("test.toml");
    std::fs::write(&preset, "[metadata]\nname = \"test\"\nversion = \"1.0\"\nauthor = \"t\"\n\n[tone]\nexposure = 0.5\n").unwrap();

    let bin = env!("CARGO_BIN_EXE_oxiraw-cli");
    let status = std::process::Command::new(bin)
        .args([
            "batch-apply",
            "--input-dir", temp_in.path().to_str().unwrap(),
            "--preset", preset.to_str().unwrap(),
            "--output-dir", temp_out.path().to_str().unwrap(),
            "--jobs", "1",
        ])
        .status()
        .unwrap();
    assert!(status.success());
    assert!(temp_out.path().join("img1.png").exists());
    assert!(temp_out.path().join("img2.png").exists());
}

#[test]
fn cli_batch_edit_with_suffix() {
    let temp_in = tempfile::TempDir::new().unwrap();
    let temp_out = tempfile::TempDir::new().unwrap();

    let img: image::ImageBuffer<image::Rgb<u8>, Vec<u8>> =
        image::ImageBuffer::from_pixel(4, 4, image::Rgb([100u8, 150, 200]));
    img.save(temp_in.path().join("photo.png")).unwrap();

    let bin = env!("CARGO_BIN_EXE_oxiraw-cli");
    let status = std::process::Command::new(bin)
        .args([
            "batch-edit",
            "--input-dir", temp_in.path().to_str().unwrap(),
            "--output-dir", temp_out.path().to_str().unwrap(),
            "--exposure", "1.0",
            "--suffix", "_bright",
            "--jobs", "1",
        ])
        .status()
        .unwrap();
    assert!(status.success());
    assert!(temp_out.path().join("photo_bright.png").exists());
}

#[test]
fn cli_batch_apply_recursive() {
    let temp_in = tempfile::TempDir::new().unwrap();
    let temp_out = tempfile::TempDir::new().unwrap();

    let sub = temp_in.path().join("sub");
    std::fs::create_dir(&sub).unwrap();

    let img: image::ImageBuffer<image::Rgb<u8>, Vec<u8>> =
        image::ImageBuffer::from_pixel(2, 2, image::Rgb([128u8, 128, 128]));
    img.save(temp_in.path().join("top.png")).unwrap();
    img.save(sub.join("nested.png")).unwrap();

    let preset = temp_in.path().join("p.toml");
    std::fs::write(&preset, "[metadata]\nname = \"p\"\nversion = \"1.0\"\nauthor = \"t\"\n").unwrap();

    let bin = env!("CARGO_BIN_EXE_oxiraw-cli");
    let status = std::process::Command::new(bin)
        .args([
            "batch-apply",
            "--input-dir", temp_in.path().to_str().unwrap(),
            "--preset", preset.to_str().unwrap(),
            "--output-dir", temp_out.path().to_str().unwrap(),
            "--recursive",
            "--jobs", "1",
        ])
        .status()
        .unwrap();
    assert!(status.success());
    assert!(temp_out.path().join("top.png").exists());
    assert!(temp_out.path().join("sub/nested.png").exists());
}

#[test]
fn cli_batch_apply_empty_dir_succeeds() {
    let temp_in = tempfile::TempDir::new().unwrap();
    let temp_out = tempfile::TempDir::new().unwrap();

    let preset = temp_in.path().join("p.toml");
    std::fs::write(&preset, "[metadata]\nname = \"p\"\nversion = \"1.0\"\nauthor = \"t\"\n").unwrap();

    let bin = env!("CARGO_BIN_EXE_oxiraw-cli");
    let status = std::process::Command::new(bin)
        .args([
            "batch-apply",
            "--input-dir", temp_in.path().to_str().unwrap(),
            "--preset", preset.to_str().unwrap(),
            "--output-dir", temp_out.path().to_str().unwrap(),
        ])
        .status()
        .unwrap();
    // Empty dir = 0 images = success (no failures)
    assert!(status.success());
}
```

Add `tempfile` and `image` to CLI dev-dependencies if not already present:

```toml
[dev-dependencies]
tempfile = "3"
image = "0.25"
```

**Step 2: Run tests**

Run: `cargo test -p oxiraw-cli`
Expected: PASS

**Step 3: Stage**

```bash
git add crates/oxiraw-cli/tests/ crates/oxiraw-cli/Cargo.toml
git commit -m "test: add batch processing CLI integration tests"
```

---

## Phase 5: Verification and Documentation

### Task 5.1: Run full verification

Run: `./scripts/verify.sh`

All checks must pass:
1. Format (`cargo fmt`)
2. Clippy (`cargo clippy`)
3. Library tests (`cargo test -p oxiraw`)
4. CLI tests (`cargo test -p oxiraw-cli`)
5. Documentation links

Fix any issues found. Stage fixes if needed:

```bash
git add -A
git commit -m "chore: fix formatting and clippy warnings for batch processing"
```

(Only if there are fixes. Skip if clean.)

---

## Summary

| Phase | Tasks | What it delivers |
|-------|-------|-----------------|
| 1 | 1.1, 1.2 | File discovery + output path resolution with full test coverage |
| 2 | 2.1, 2.2, 2.3 | Core batch execution with rayon parallelism, progress, error collection |
| 3 | 3.1, 3.2 | CLI subcommands (`batch-apply`, `batch-edit`) wired to batch module |
| 4 | 4.1 | Integration tests proving end-to-end batch workflows |
| 5 | 5.1 | Verification pass, formatting, clippy clean |
