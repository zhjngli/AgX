# Batch Processing Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add `batch-apply` and `batch-edit` CLI subcommands that process an entire directory of images in parallel with rayon, fail-fast by default, and optional `--skip-errors` for partial-failure tolerance.

**Architecture:** All batch logic lives in `crates/oxiraw-cli/src/batch.rs`. Each image goes through the same `decode → Engine → render → encode` pipeline as the existing single-file commands. Rayon provides CPU-level parallelism via `par_iter`. A shared `AtomicBool` flag enables fail-fast (default) or skip-errors mode.

**Tech Stack:** Rust, rayon (parallelism), clap (CLI), tempfile (test fixtures)

---

## Background

Read these before starting:
- Design doc: `docs/plans/2026-03-07-batch-processing-design.md` (will be committed in Task 1)
- Architecture: `ARCHITECTURE.md` (module dependency rules, CLI is a thin wrapper)
- Existing CLI: `crates/oxiraw-cli/src/main.rs` (pattern for `run_apply`/`run_edit`)
- Library API: `crates/oxiraw/src/lib.rs` (re-exports: `Engine`, `Preset`, `Parameters`, `decode`, `encode`, `Lut3D`, `metadata`)

Key library APIs used by batch processing:
- `oxiraw::decode::decode(path) -> Result<Rgb32FImage>` — decode any image to linear sRGB
- `oxiraw::decode::is_raw_extension(path) -> bool` — check if file is a raw format
- `oxiraw::Engine::new(image)` — create engine with decoded image
- `oxiraw::Engine::apply_preset(&mut self, preset)` — apply preset parameters
- `oxiraw::Engine::set_params(params)` / `set_lut(lut)` — set inline parameters
- `oxiraw::Engine::render() -> Rgb32FImage` — render with current parameters
- `oxiraw::encode::encode_to_file_with_options(image, path, opts, metadata) -> Result<PathBuf>`
- `oxiraw::encode::OutputFormat` — `Jpeg`, `Png`, `Tiff` with `.extension() -> &str`
- `oxiraw::metadata::extract_metadata(path) -> Option<ImageMetadata>`
- `oxiraw::Preset::load_from_file(path) -> Result<Preset>`
- `oxiraw::Lut3D::from_cube_file(path) -> Result<Lut3D>`

---

### Task 1: Commit the design doc

**Files:**
- Create: `docs/plans/2026-03-07-batch-processing-design.md`

The design doc already exists on the old branch. Copy it to the new branch and commit.

**Step 1: Copy the design doc from the old branch**

```bash
git show claude/review-repo-suggestions:docs/plans/2026-03-07-batch-processing-design.md > docs/plans/2026-03-07-batch-processing-design.md
```

**Step 2: Commit**

```bash
git add docs/plans/2026-03-07-batch-processing-design.md
git commit -m "docs: add batch processing design doc"
```

---

### Task 2: File discovery — `discover_images`

**Files:**
- Create: `crates/oxiraw-cli/src/batch.rs`
- Modify: `crates/oxiraw-cli/src/main.rs` (add `mod batch;`)

**Step 1: Write failing tests**

Create `crates/oxiraw-cli/src/batch.rs` with tests only:

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
pub fn discover_images(_dir: &Path, _recursive: bool) -> Vec<PathBuf> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_files(dir: &Path, names: &[&str]) {
        for name in names {
            fs::write(dir.join(name), b"fake").unwrap();
        }
    }

    #[test]
    fn discover_finds_image_files() {
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
        assert_eq!(flat.len(), 1);

        let recursive = discover_images(dir.path(), true);
        assert_eq!(recursive.len(), 2);
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
        let names: Vec<_> = found
            .iter()
            .map(|p| p.file_name().unwrap().to_str().unwrap())
            .collect();
        assert_eq!(names, vec!["a.jpg", "b.jpg", "c.jpg"]);
    }
}
```

Also add `mod batch;` at the top of `crates/oxiraw-cli/src/main.rs` (after the existing `use` statements).

And add `tempfile = "3"` to `[dev-dependencies]` in `crates/oxiraw-cli/Cargo.toml`.

**Step 2: Run tests to verify they fail**

Run: `cargo test -p oxiraw-cli -- batch::tests --no-capture 2>&1 | tail -5`
Expected: FAIL with "not yet implemented"

**Step 3: Implement discover_images**

Replace the `todo!()` body with:

```rust
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

**Step 4: Run tests to verify they pass**

Run: `cargo test -p oxiraw-cli -- batch::tests`
Expected: 5 tests PASS

**Step 5: Commit**

```bash
git add crates/oxiraw-cli/src/batch.rs crates/oxiraw-cli/src/main.rs crates/oxiraw-cli/Cargo.toml
git commit -m "feat: add batch file discovery with extension filtering and recursion"
```

---

### Task 3: Output path resolution — `resolve_output_path`

**Files:**
- Modify: `crates/oxiraw-cli/src/batch.rs`

**Step 1: Write failing tests**

Add to `batch.rs` (above the tests module):

```rust
/// Resolve the output path for a batch-processed image.
///
/// - Mirrors subdirectory structure from `input_dir` into `output_dir`
/// - Appends optional suffix before the extension (e.g., `_edited`)
/// - Overrides extension if `format_ext` is provided
/// - Raw format inputs default to `.jpg` extension when no format override
pub fn resolve_output_path(
    _input: &Path,
    _input_dir: &Path,
    _output_dir: &Path,
    _suffix: Option<&str>,
    _format_ext: Option<&str>,
) -> PathBuf {
    todo!()
}
```

Add these tests inside the existing `tests` module:

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

Run: `cargo test -p oxiraw-cli -- batch::tests::resolve_output --no-capture 2>&1 | tail -5`
Expected: FAIL with "not yet implemented"

**Step 3: Implement resolve_output_path**

Replace the `todo!()` body:

```rust
pub fn resolve_output_path(
    input: &Path,
    input_dir: &Path,
    output_dir: &Path,
    suffix: Option<&str>,
    format_ext: Option<&str>,
) -> PathBuf {
    let relative = input
        .strip_prefix(input_dir)
        .unwrap_or(input.file_name().map(Path::new).unwrap_or(input));

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

    let stem = relative
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");

    let filename = match suffix {
        Some(s) => format!("{stem}{s}.{ext}"),
        None => format!("{stem}.{ext}"),
    };

    let parent = relative.parent().unwrap_or(Path::new(""));
    output_dir.join(parent).join(filename)
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo test -p oxiraw-cli -- batch::tests`
Expected: 11 tests PASS (5 discovery + 6 path resolution)

**Step 5: Commit**

```bash
git add crates/oxiraw-cli/src/batch.rs
git commit -m "feat: add resolve_output_path for batch output naming"
```

---

### Task 4: Batch result types and progress reporting

**Files:**
- Modify: `crates/oxiraw-cli/src/batch.rs`

**Step 1: Add types and progress function**

Add these above the `#[cfg(test)]` module in `batch.rs`. No tests needed — these are simple data types and a side-effect function. They'll be tested through integration in Tasks 5-6.

```rust
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
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
fn report_progress(
    counter: &AtomicUsize,
    total: usize,
    input: &Path,
    outcome: &Result<Duration, String>,
) {
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

    eprintln!(
        "\nBatch complete: {succeeded}/{total} succeeded in {:.1}s",
        elapsed.as_secs_f64()
    );
    if !failed.is_empty() {
        eprintln!("Errors ({}):", failed.len());
        for (path, err) in &failed {
            eprintln!("  {}: {err}", path.display());
        }
    }

    BatchSummary {
        total,
        succeeded,
        failed,
        elapsed,
    }
}
```

**Step 2: Verify it compiles**

Run: `cargo test -p oxiraw-cli -- batch::tests`
Expected: 11 tests PASS (nothing should break)

**Step 3: Commit**

```bash
git add crates/oxiraw-cli/src/batch.rs
git commit -m "feat: add batch result types, progress reporting, and summary"
```

---

### Task 5: Parallel batch-apply — `run_batch_apply`

**Files:**
- Modify: `crates/oxiraw-cli/src/batch.rs`
- Modify: `crates/oxiraw-cli/Cargo.toml` (add `rayon = "1"`)

**Step 1: Add rayon dependency**

Add `rayon = "1"` to `[dependencies]` in `crates/oxiraw-cli/Cargo.toml`.

**Step 2: Write failing test**

Add this helper and test to the `tests` module in `batch.rs`:

```rust
fn write_test_png(path: &Path) {
    use image::ImageBuffer;
    let img: ImageBuffer<image::Rgb<u8>, Vec<u8>> =
        ImageBuffer::from_pixel(2, 2, image::Rgb([128u8, 64, 32]));
    img.save(path).unwrap();
}

#[test]
fn batch_apply_processes_multiple_images() {
    let dir = TempDir::new().unwrap();
    let input_dir = dir.path().join("input");
    let output_dir = dir.path().join("output");
    fs::create_dir(&input_dir).unwrap();

    write_test_png(&input_dir.join("a.png"));
    write_test_png(&input_dir.join("b.png"));

    let preset_path = dir.path().join("test.toml");
    fs::write(
        &preset_path,
        "[metadata]\nname = \"test\"\nversion = \"1.0\"\nauthor = \"test\"\n",
    )
    .unwrap();

    let summary = run_batch_apply(
        &input_dir,
        &preset_path,
        &output_dir,
        false,
        92,
        None,
        None,
        1,
        false,
    );

    assert_eq!(summary.total, 2);
    assert_eq!(summary.succeeded, 2);
    assert!(summary.failed.is_empty());
    assert!(output_dir.join("a.png").exists());
    assert!(output_dir.join("b.png").exists());
}
```

Also add the function stub above the tests module:

```rust
/// Process a single image with a preset (used by batch-apply).
fn process_apply_single(
    _input: &Path,
    _output: &Path,
    _preset: &oxiraw::Preset,
    _quality: u8,
    _format: Option<oxiraw::encode::OutputFormat>,
) -> Result<Duration, String> {
    todo!()
}

/// Run batch-apply: apply a preset to all images in a directory.
///
/// When `skip_errors` is false (the default), the first error stops all processing.
/// When `skip_errors` is true, all images are processed and errors are collected.
#[allow(clippy::too_many_arguments)]
pub fn run_batch_apply(
    _input_dir: &Path,
    _preset_path: &Path,
    _output_dir: &Path,
    _recursive: bool,
    _quality: u8,
    _format: Option<oxiraw::encode::OutputFormat>,
    _suffix: Option<&str>,
    _jobs: usize,
    _skip_errors: bool,
) -> BatchSummary {
    todo!()
}
```

**Step 3: Run test to verify it fails**

Run: `cargo test -p oxiraw-cli -- batch::tests::batch_apply --no-capture 2>&1 | tail -5`
Expected: FAIL with "not yet implemented"

**Step 4: Implement process_apply_single and run_batch_apply**

```rust
use rayon::prelude::*;

fn process_apply_single(
    input: &Path,
    output: &Path,
    preset: &oxiraw::Preset,
    quality: u8,
    format: Option<oxiraw::encode::OutputFormat>,
) -> Result<Duration, String> {
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

    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    oxiraw::encode::encode_to_file_with_options(&rendered, output, &opts, metadata.as_ref())
        .map_err(|e| e.to_string())?;
    Ok(start.elapsed())
}

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
                failed: images
                    .iter()
                    .map(|p| (p.clone(), format!("preset load failed: {e}")))
                    .collect(),
                elapsed: batch_start.elapsed(),
            };
        }
    };

    let format_ext = format.map(|f| f.extension());
    let total = images.len();
    let counter = AtomicUsize::new(0);
    // Shared flag: set to true when any image fails and skip_errors is false (fail-fast).
    let should_stop = AtomicBool::new(false);

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(if jobs == 0 { num_cpus() } else { jobs })
        .build()
        .expect("failed to create thread pool");

    let num_threads = pool.current_num_threads();
    eprintln!("Processing {total} images with {num_threads} workers...");

    let results: Vec<BatchResult> = pool.install(|| {
        images
            .par_iter()
            .map(|input| {
                // Fail-fast: if another thread already failed, skip remaining work.
                if !skip_errors && should_stop.load(Ordering::Relaxed) {
                    return BatchResult {
                        input: input.clone(),
                        output: PathBuf::new(),
                        outcome: Err("skipped (earlier error in fail-fast mode)".to_string()),
                    };
                }

                let output =
                    resolve_output_path(input, input_dir, output_dir, suffix, format_ext);
                let outcome =
                    process_apply_single(input, &output, &preset, quality, format);

                if outcome.is_err() && !skip_errors {
                    should_stop.store(true, Ordering::Relaxed);
                }

                report_progress(&counter, total, input, &outcome);
                BatchResult {
                    input: input.clone(),
                    output,
                    outcome,
                }
            })
            .collect()
    });

    summarize(&results, batch_start.elapsed())
}

/// Get the number of available CPU cores.
fn num_cpus() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
}
```

**Step 5: Run test to verify it passes**

Run: `cargo test -p oxiraw-cli -- batch::tests::batch_apply`
Expected: PASS

**Step 6: Commit**

```bash
git add crates/oxiraw-cli/src/batch.rs crates/oxiraw-cli/Cargo.toml
git commit -m "feat: add rayon-parallel batch-apply with fail-fast support"
```

---

### Task 6: Parallel batch-edit — `run_batch_edit`

**Files:**
- Modify: `crates/oxiraw-cli/src/batch.rs`

**Step 1: Write failing test**

Add to `tests` module:

```rust
#[test]
fn batch_edit_processes_with_params() {
    let dir = TempDir::new().unwrap();
    let input_dir = dir.path().join("input");
    let output_dir = dir.path().join("output");
    fs::create_dir(&input_dir).unwrap();

    write_test_png(&input_dir.join("photo.png"));

    let params = oxiraw::Parameters::default();

    let summary = run_batch_edit(
        &input_dir,
        &output_dir,
        false,
        &params,
        None,
        92,
        None,
        None,
        1,
        false,
    );

    assert_eq!(summary.total, 1);
    assert_eq!(summary.succeeded, 1);
    assert!(summary.failed.is_empty());
    assert!(output_dir.join("photo.png").exists());
}
```

Also add the function stubs:

```rust
fn process_edit_single(
    _input: &Path,
    _output: &Path,
    _params: &oxiraw::Parameters,
    _lut: Option<&oxiraw::Lut3D>,
    _quality: u8,
    _format: Option<oxiraw::encode::OutputFormat>,
) -> Result<Duration, String> {
    todo!()
}

#[allow(clippy::too_many_arguments)]
pub fn run_batch_edit(
    _input_dir: &Path,
    _output_dir: &Path,
    _recursive: bool,
    _params: &oxiraw::Parameters,
    _lut: Option<&oxiraw::Lut3D>,
    _quality: u8,
    _format: Option<oxiraw::encode::OutputFormat>,
    _suffix: Option<&str>,
    _jobs: usize,
    _skip_errors: bool,
) -> BatchSummary {
    todo!()
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p oxiraw-cli -- batch::tests::batch_edit --no-capture 2>&1 | tail -5`
Expected: FAIL with "not yet implemented"

**Step 3: Implement process_edit_single and run_batch_edit**

```rust
fn process_edit_single(
    input: &Path,
    output: &Path,
    params: &oxiraw::Parameters,
    lut: Option<&oxiraw::Lut3D>,
    quality: u8,
    format: Option<oxiraw::encode::OutputFormat>,
) -> Result<Duration, String> {
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
    let should_stop = AtomicBool::new(false);

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(if jobs == 0 { num_cpus() } else { jobs })
        .build()
        .expect("failed to create thread pool");

    let num_threads = pool.current_num_threads();
    eprintln!("Processing {total} images with {num_threads} workers...");

    let results: Vec<BatchResult> = pool.install(|| {
        images
            .par_iter()
            .map(|input| {
                if !skip_errors && should_stop.load(Ordering::Relaxed) {
                    return BatchResult {
                        input: input.clone(),
                        output: PathBuf::new(),
                        outcome: Err("skipped (earlier error in fail-fast mode)".to_string()),
                    };
                }

                let output =
                    resolve_output_path(input, input_dir, output_dir, suffix, format_ext);
                let outcome =
                    process_edit_single(input, &output, params, lut, quality, format);

                if outcome.is_err() && !skip_errors {
                    should_stop.store(true, Ordering::Relaxed);
                }

                report_progress(&counter, total, input, &outcome);
                BatchResult {
                    input: input.clone(),
                    output,
                    outcome,
                }
            })
            .collect()
    });

    summarize(&results, batch_start.elapsed())
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo test -p oxiraw-cli -- batch::tests`
Expected: 13 tests PASS

**Step 5: Commit**

```bash
git add crates/oxiraw-cli/src/batch.rs
git commit -m "feat: add rayon-parallel batch-edit with fail-fast support"
```

---

### Task 7: CLI subcommands — `BatchApply` and `BatchEdit`

**Files:**
- Modify: `crates/oxiraw-cli/src/main.rs`

**Step 1: Add BatchApply and BatchEdit variants to Commands enum**

Add these two variants to the `Commands` enum in `main.rs`, after the existing `Edit` variant:

```rust
/// Apply a TOML preset to all images in a directory
BatchApply {
    /// Directory containing input images
    #[arg(long)]
    input_dir: PathBuf,
    /// Preset TOML file path
    #[arg(short, long)]
    preset: PathBuf,
    /// Directory for output images (created if missing)
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
    /// Append suffix to output filenames (e.g., `_edited`)
    #[arg(long)]
    suffix: Option<String>,
    /// JPEG output quality (1-100, default 92)
    #[arg(long, default_value_t = 92)]
    quality: u8,
    /// Output format (jpeg, png, tiff). Preserved from input if not specified.
    #[arg(long)]
    format: Option<String>,
},
/// Edit all images in a directory with inline parameters
BatchEdit {
    /// Directory containing input images
    #[arg(long)]
    input_dir: PathBuf,
    /// Directory for output images (created if missing)
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
    /// Append suffix to output filenames (e.g., `_edited`)
    #[arg(long)]
    suffix: Option<String>,
    /// JPEG output quality (1-100, default 92)
    #[arg(long, default_value_t = 92)]
    quality: u8,
    /// Output format (jpeg, png, tiff). Preserved from input if not specified.
    #[arg(long)]
    format: Option<String>,
    /// Exposure in stops (-5.0 to +5.0)
    #[arg(long, default_value_t = 0.0, allow_hyphen_values = true)]
    exposure: f32,
    /// Contrast (-100 to +100)
    #[arg(long, default_value_t = 0.0, allow_hyphen_values = true)]
    contrast: f32,
    /// Highlights (-100 to +100)
    #[arg(long, default_value_t = 0.0, allow_hyphen_values = true)]
    highlights: f32,
    /// Shadows (-100 to +100)
    #[arg(long, default_value_t = 0.0, allow_hyphen_values = true)]
    shadows: f32,
    /// Whites (-100 to +100)
    #[arg(long, default_value_t = 0.0, allow_hyphen_values = true)]
    whites: f32,
    /// Blacks (-100 to +100)
    #[arg(long, default_value_t = 0.0, allow_hyphen_values = true)]
    blacks: f32,
    /// White balance temperature shift
    #[arg(long, default_value_t = 0.0, allow_hyphen_values = true)]
    temperature: f32,
    /// White balance tint shift
    #[arg(long, default_value_t = 0.0, allow_hyphen_values = true)]
    tint: f32,
    /// Path to a .cube LUT file
    #[arg(long)]
    lut: Option<PathBuf>,
    // --- HSL per-channel adjustments (same flags as `edit`) ---
    #[arg(long = "hsl-red-hue", visible_alias = "hsl-red-h", default_value_t = 0.0, allow_hyphen_values = true)]
    hsl_red_hue: f32,
    #[arg(long = "hsl-red-saturation", visible_alias = "hsl-red-s", default_value_t = 0.0, allow_hyphen_values = true)]
    hsl_red_saturation: f32,
    #[arg(long = "hsl-red-luminance", visible_alias = "hsl-red-l", default_value_t = 0.0, allow_hyphen_values = true)]
    hsl_red_luminance: f32,
    #[arg(long = "hsl-orange-hue", visible_alias = "hsl-orange-h", default_value_t = 0.0, allow_hyphen_values = true)]
    hsl_orange_hue: f32,
    #[arg(long = "hsl-orange-saturation", visible_alias = "hsl-orange-s", default_value_t = 0.0, allow_hyphen_values = true)]
    hsl_orange_saturation: f32,
    #[arg(long = "hsl-orange-luminance", visible_alias = "hsl-orange-l", default_value_t = 0.0, allow_hyphen_values = true)]
    hsl_orange_luminance: f32,
    #[arg(long = "hsl-yellow-hue", visible_alias = "hsl-yellow-h", default_value_t = 0.0, allow_hyphen_values = true)]
    hsl_yellow_hue: f32,
    #[arg(long = "hsl-yellow-saturation", visible_alias = "hsl-yellow-s", default_value_t = 0.0, allow_hyphen_values = true)]
    hsl_yellow_saturation: f32,
    #[arg(long = "hsl-yellow-luminance", visible_alias = "hsl-yellow-l", default_value_t = 0.0, allow_hyphen_values = true)]
    hsl_yellow_luminance: f32,
    #[arg(long = "hsl-green-hue", visible_alias = "hsl-green-h", default_value_t = 0.0, allow_hyphen_values = true)]
    hsl_green_hue: f32,
    #[arg(long = "hsl-green-saturation", visible_alias = "hsl-green-s", default_value_t = 0.0, allow_hyphen_values = true)]
    hsl_green_saturation: f32,
    #[arg(long = "hsl-green-luminance", visible_alias = "hsl-green-l", default_value_t = 0.0, allow_hyphen_values = true)]
    hsl_green_luminance: f32,
    #[arg(long = "hsl-aqua-hue", visible_alias = "hsl-aqua-h", default_value_t = 0.0, allow_hyphen_values = true)]
    hsl_aqua_hue: f32,
    #[arg(long = "hsl-aqua-saturation", visible_alias = "hsl-aqua-s", default_value_t = 0.0, allow_hyphen_values = true)]
    hsl_aqua_saturation: f32,
    #[arg(long = "hsl-aqua-luminance", visible_alias = "hsl-aqua-l", default_value_t = 0.0, allow_hyphen_values = true)]
    hsl_aqua_luminance: f32,
    #[arg(long = "hsl-blue-hue", visible_alias = "hsl-blue-h", default_value_t = 0.0, allow_hyphen_values = true)]
    hsl_blue_hue: f32,
    #[arg(long = "hsl-blue-saturation", visible_alias = "hsl-blue-s", default_value_t = 0.0, allow_hyphen_values = true)]
    hsl_blue_saturation: f32,
    #[arg(long = "hsl-blue-luminance", visible_alias = "hsl-blue-l", default_value_t = 0.0, allow_hyphen_values = true)]
    hsl_blue_luminance: f32,
    #[arg(long = "hsl-purple-hue", visible_alias = "hsl-purple-h", default_value_t = 0.0, allow_hyphen_values = true)]
    hsl_purple_hue: f32,
    #[arg(long = "hsl-purple-saturation", visible_alias = "hsl-purple-s", default_value_t = 0.0, allow_hyphen_values = true)]
    hsl_purple_saturation: f32,
    #[arg(long = "hsl-purple-luminance", visible_alias = "hsl-purple-l", default_value_t = 0.0, allow_hyphen_values = true)]
    hsl_purple_luminance: f32,
    #[arg(long = "hsl-magenta-hue", visible_alias = "hsl-magenta-h", default_value_t = 0.0, allow_hyphen_values = true)]
    hsl_magenta_hue: f32,
    #[arg(long = "hsl-magenta-saturation", visible_alias = "hsl-magenta-s", default_value_t = 0.0, allow_hyphen_values = true)]
    hsl_magenta_saturation: f32,
    #[arg(long = "hsl-magenta-luminance", visible_alias = "hsl-magenta-l", default_value_t = 0.0, allow_hyphen_values = true)]
    hsl_magenta_luminance: f32,
},
```

**Step 2: Add match arms in `main()` function**

Add these match arms inside the `let result = match cli.command { ... }` block, after the existing `Commands::Edit { .. }` arm:

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
} => (|| -> oxiraw::Result<()> {
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
})(),
Commands::BatchEdit {
    input_dir,
    output_dir,
    recursive,
    jobs,
    skip_errors,
    suffix,
    quality,
    format,
    exposure,
    contrast,
    highlights,
    shadows,
    whites,
    blacks,
    temperature,
    tint,
    lut,
    hsl_red_hue,
    hsl_red_saturation,
    hsl_red_luminance,
    hsl_orange_hue,
    hsl_orange_saturation,
    hsl_orange_luminance,
    hsl_yellow_hue,
    hsl_yellow_saturation,
    hsl_yellow_luminance,
    hsl_green_hue,
    hsl_green_saturation,
    hsl_green_luminance,
    hsl_aqua_hue,
    hsl_aqua_saturation,
    hsl_aqua_luminance,
    hsl_blue_hue,
    hsl_blue_saturation,
    hsl_blue_luminance,
    hsl_purple_hue,
    hsl_purple_saturation,
    hsl_purple_luminance,
    hsl_magenta_hue,
    hsl_magenta_saturation,
    hsl_magenta_luminance,
} => (|| -> oxiraw::Result<()> {
    let hsl = oxiraw::engine::HslChannels {
        red: oxiraw::engine::HslChannel {
            hue: hsl_red_hue,
            saturation: hsl_red_saturation,
            luminance: hsl_red_luminance,
        },
        orange: oxiraw::engine::HslChannel {
            hue: hsl_orange_hue,
            saturation: hsl_orange_saturation,
            luminance: hsl_orange_luminance,
        },
        yellow: oxiraw::engine::HslChannel {
            hue: hsl_yellow_hue,
            saturation: hsl_yellow_saturation,
            luminance: hsl_yellow_luminance,
        },
        green: oxiraw::engine::HslChannel {
            hue: hsl_green_hue,
            saturation: hsl_green_saturation,
            luminance: hsl_green_luminance,
        },
        aqua: oxiraw::engine::HslChannel {
            hue: hsl_aqua_hue,
            saturation: hsl_aqua_saturation,
            luminance: hsl_aqua_luminance,
        },
        blue: oxiraw::engine::HslChannel {
            hue: hsl_blue_hue,
            saturation: hsl_blue_saturation,
            luminance: hsl_blue_luminance,
        },
        purple: oxiraw::engine::HslChannel {
            hue: hsl_purple_hue,
            saturation: hsl_purple_saturation,
            luminance: hsl_purple_luminance,
        },
        magenta: oxiraw::engine::HslChannel {
            hue: hsl_magenta_hue,
            saturation: hsl_magenta_saturation,
            luminance: hsl_magenta_luminance,
        },
    };
    let params = oxiraw::Parameters {
        exposure,
        contrast,
        highlights,
        shadows,
        whites,
        blacks,
        temperature,
        tint,
        hsl,
    };

    let lut_data = lut
        .as_deref()
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
})(),
```

**Step 3: Verify it compiles and existing tests pass**

Run: `cargo test -p oxiraw-cli`
Expected: All existing tests PASS

**Step 4: Commit**

```bash
git add crates/oxiraw-cli/src/main.rs
git commit -m "feat: add batch-apply and batch-edit CLI subcommands"
```

---

### Task 8: Integration tests

**Files:**
- Modify: `crates/oxiraw-cli/tests/integration.rs`

**Step 1: Write integration tests**

Add these tests to the existing `integration.rs` file. The `create_test_png` helper already exists.

```rust
#[test]
fn cli_batch_apply_with_preset() {
    let temp = tempfile::TempDir::new().unwrap();
    let input_dir = temp.path().join("input");
    let output_dir = temp.path().join("output");
    std::fs::create_dir(&input_dir).unwrap();

    create_test_png(&input_dir.join("img1.png"));
    create_test_png(&input_dir.join("img2.png"));

    let preset = temp.path().join("test.toml");
    std::fs::write(
        &preset,
        "[metadata]\nname = \"test\"\nversion = \"1.0\"\nauthor = \"t\"\n\n[tone]\nexposure = 0.5\n",
    )
    .unwrap();

    let status = cli_bin()
        .args([
            "batch-apply",
            "--input-dir",
            input_dir.to_str().unwrap(),
            "--preset",
            preset.to_str().unwrap(),
            "--output-dir",
            output_dir.to_str().unwrap(),
            "--jobs",
            "1",
        ])
        .status()
        .unwrap();
    assert!(status.success());
    assert!(output_dir.join("img1.png").exists());
    assert!(output_dir.join("img2.png").exists());
}

#[test]
fn cli_batch_edit_with_suffix() {
    let temp = tempfile::TempDir::new().unwrap();
    let input_dir = temp.path().join("input");
    let output_dir = temp.path().join("output");
    std::fs::create_dir(&input_dir).unwrap();

    create_test_png(&input_dir.join("photo.png"));

    let status = cli_bin()
        .args([
            "batch-edit",
            "--input-dir",
            input_dir.to_str().unwrap(),
            "--output-dir",
            output_dir.to_str().unwrap(),
            "--exposure",
            "1.0",
            "--suffix",
            "_bright",
            "--jobs",
            "1",
        ])
        .status()
        .unwrap();
    assert!(status.success());
    assert!(output_dir.join("photo_bright.png").exists());
}

#[test]
fn cli_batch_apply_recursive() {
    let temp = tempfile::TempDir::new().unwrap();
    let input_dir = temp.path().join("input");
    let output_dir = temp.path().join("output");
    std::fs::create_dir(&input_dir).unwrap();
    let sub = input_dir.join("sub");
    std::fs::create_dir(&sub).unwrap();

    create_test_png(&input_dir.join("top.png"));
    create_test_png(&sub.join("nested.png"));

    let preset = temp.path().join("p.toml");
    std::fs::write(
        &preset,
        "[metadata]\nname = \"p\"\nversion = \"1.0\"\nauthor = \"t\"\n",
    )
    .unwrap();

    let status = cli_bin()
        .args([
            "batch-apply",
            "--input-dir",
            input_dir.to_str().unwrap(),
            "--preset",
            preset.to_str().unwrap(),
            "--output-dir",
            output_dir.to_str().unwrap(),
            "--recursive",
            "--jobs",
            "1",
        ])
        .status()
        .unwrap();
    assert!(status.success());
    assert!(output_dir.join("top.png").exists());
    assert!(output_dir.join("sub/nested.png").exists());
}

#[test]
fn cli_batch_apply_empty_dir_succeeds() {
    let temp = tempfile::TempDir::new().unwrap();
    let input_dir = temp.path().join("input");
    let output_dir = temp.path().join("output");
    std::fs::create_dir(&input_dir).unwrap();

    let preset = temp.path().join("p.toml");
    std::fs::write(
        &preset,
        "[metadata]\nname = \"p\"\nversion = \"1.0\"\nauthor = \"t\"\n",
    )
    .unwrap();

    let status = cli_bin()
        .args([
            "batch-apply",
            "--input-dir",
            input_dir.to_str().unwrap(),
            "--preset",
            preset.to_str().unwrap(),
            "--output-dir",
            output_dir.to_str().unwrap(),
        ])
        .status()
        .unwrap();
    assert!(status.success());
}
```

**Step 2: Run integration tests**

Run: `cargo test -p oxiraw-cli --test integration`
Expected: All tests PASS (existing + 4 new)

**Step 3: Commit**

```bash
git add crates/oxiraw-cli/tests/integration.rs
git commit -m "test: add batch processing integration tests"
```

---

### Task 9: Documentation and ARCHITECTURE.md

**Files:**
- Modify: `ARCHITECTURE.md`

**Step 1: Add batch design doc link to ARCHITECTURE.md**

In the `### Plans` table in `ARCHITECTURE.md`, add a new row after the last entry:

```markdown
| 2026-03-07 | [Batch Processing Design](docs/plans/2026-03-07-batch-processing-design.md)                      |
| 2026-03-12 | [Batch Processing Implementation](docs/plans/2026-03-12-batch-processing-implementation.md)      |
```

**Step 2: Run full verification**

Run: `./scripts/verify.sh`
Expected: ALL CHECKS PASSED (5/5)

**Step 3: Commit**

```bash
git add ARCHITECTURE.md
git commit -m "docs: add batch processing plan links to ARCHITECTURE.md"
```

---

## Verification Checklist

After all tasks:
1. `./scripts/verify.sh` — all 5 checks pass
2. `cargo test -p oxiraw-cli -- batch` — 13 unit tests pass
3. `cargo test -p oxiraw-cli --test integration` — all integration tests pass (including 4 batch tests)
4. Manual smoke test: `cargo run -p oxiraw-cli -- batch-apply --help` shows all flags
5. ARCHITECTURE.md updated with design doc links
