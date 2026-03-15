use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;
use std::time::Instant;

use rayon::prelude::*;

/// Standard (non-raw) image file extensions recognized by the CLI.
const STANDARD_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "tiff", "tif"];

/// Returns `true` if `path` has a standard image extension or a known raw extension.
fn is_image_file(path: &Path) -> bool {
    let has_standard_ext = path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| STANDARD_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()));
    has_standard_ext || agx::decode::is_raw_extension(path)
}

/// Scan `dir` for image files, optionally recursing into subdirectories.
/// Returns a sorted `Vec<PathBuf>` of discovered image files.
pub fn discover_images(dir: &Path, recursive: bool) -> Vec<PathBuf> {
    let mut out = Vec::new();
    collect_images(dir, recursive, &mut out);
    out.sort();
    out
}

/// Recursively (or not) collect image file paths from `dir` into `out`.
fn collect_images(dir: &Path, recursive: bool, out: &mut Vec<PathBuf>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if recursive {
                collect_images(&path, recursive, out);
            }
        } else if path.is_file() && is_image_file(&path) {
            out.push(path);
        }
    }
}

/// Resolve the output path for a processed image.
///
/// Mirrors the subdirectory structure from `input_dir` into `output_dir`,
/// appends an optional suffix before the extension, and overrides the
/// extension when `format_ext` is provided.  Raw-format inputs default to
/// `.jpg` when no explicit format is given.
pub fn resolve_output_path(
    input: &Path,
    input_dir: &Path,
    output_dir: &Path,
    suffix: Option<&str>,
    format_ext: Option<&str>,
) -> PathBuf {
    // 1. Strip the input_dir prefix to get the relative path.
    let relative = input
        .strip_prefix(input_dir)
        .unwrap_or(input.file_name().map(Path::new).unwrap_or(input));

    // 2. Determine extension: explicit format > raw-default "jpg" > original.
    let ext = if let Some(fmt) = format_ext {
        fmt.to_string()
    } else if agx::decode::is_raw_extension(input) {
        "jpg".to_string()
    } else {
        input
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("jpg")
            .to_string()
    };

    // 3. Get the file stem from the relative path's filename.
    let stem = relative
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");

    // 4. Build filename with optional suffix.
    let filename = match suffix {
        Some(s) => format!("{stem}{s}.{ext}"),
        None => format!("{stem}.{ext}"),
    };

    // 5. Join output_dir + parent of relative + filename.
    let parent = relative.parent().unwrap_or(Path::new(""));
    output_dir.join(parent).join(filename)
}

/// Result of processing a single image in a batch.
pub struct BatchResult {
    pub input: PathBuf,
    #[allow(dead_code)]
    pub output: PathBuf,
    pub outcome: Result<Duration, String>,
}

/// Summary of a batch run.
pub struct BatchSummary {
    #[allow(dead_code)]
    pub total: usize,
    #[allow(dead_code)]
    pub succeeded: usize,
    pub failed: Vec<(PathBuf, String)>,
    #[allow(dead_code)]
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

/// Get the number of available CPU cores.
fn num_cpus() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
}

/// Process a single image: decode, configure engine via closure, render, encode.
fn process_single(
    input: &Path,
    output: &Path,
    quality: u8,
    format: Option<agx::encode::OutputFormat>,
    configure: impl FnOnce(&mut agx::Engine),
) -> Result<Duration, String> {
    let start = Instant::now();
    let metadata = agx::metadata::extract_metadata(input);
    let linear = agx::decode::decode(input).map_err(|e| e.to_string())?;
    let mut engine = agx::Engine::new(linear);
    configure(&mut engine);
    let rendered = engine.render();
    let opts = agx::encode::EncodeOptions {
        jpeg_quality: quality,
        format,
    };

    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    agx::encode::encode_to_file_with_options(&rendered, output, &opts, metadata.as_ref())
        .map_err(|e| e.to_string())?;
    Ok(start.elapsed())
}

/// Options shared by all batch operations.
struct BatchOpts<'a> {
    input_dir: &'a Path,
    output_dir: &'a Path,
    recursive: bool,
    format_ext: Option<&'static str>,
    suffix: Option<&'a str>,
    jobs: usize,
    skip_errors: bool,
}

/// Generic batch processing: discover images, process in parallel, summarize.
fn run_batch<F>(opts: &BatchOpts<'_>, process: F) -> BatchSummary
where
    F: Fn(&Path, &Path) -> Result<Duration, String> + Sync,
{
    let batch_start = Instant::now();

    let images = discover_images(opts.input_dir, opts.recursive);
    if images.is_empty() {
        eprintln!("No image files found in {}", opts.input_dir.display());
        return BatchSummary {
            total: 0,
            succeeded: 0,
            failed: Vec::new(),
            elapsed: batch_start.elapsed(),
        };
    }
    let total = images.len();
    let counter = AtomicUsize::new(0);
    let should_stop = AtomicBool::new(false);

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(if opts.jobs == 0 {
            num_cpus()
        } else {
            opts.jobs
        })
        .build()
        .expect("failed to create thread pool");

    let num_threads = pool.current_num_threads();
    eprintln!("Processing {total} images with {num_threads} workers...");

    let results: Vec<BatchResult> = pool.install(|| {
        images
            .par_iter()
            .map(|input| {
                if !opts.skip_errors && should_stop.load(Ordering::Relaxed) {
                    return BatchResult {
                        input: input.clone(),
                        output: PathBuf::new(),
                        outcome: Err("skipped (earlier error in fail-fast mode)".to_string()),
                    };
                }

                let output = resolve_output_path(
                    input,
                    opts.input_dir,
                    opts.output_dir,
                    opts.suffix,
                    opts.format_ext,
                );
                let outcome = process(input, &output);

                if outcome.is_err() && !opts.skip_errors {
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

/// Run batch-apply: apply a preset to all images in a directory, in parallel.
#[allow(clippy::too_many_arguments)]
pub fn run_batch_apply(
    input_dir: &Path,
    preset_path: &Path,
    output_dir: &Path,
    recursive: bool,
    quality: u8,
    format: Option<agx::encode::OutputFormat>,
    suffix: Option<&str>,
    jobs: usize,
    skip_errors: bool,
) -> BatchSummary {
    let preset = match agx::Preset::load_from_file(preset_path) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Failed to load preset: {e}");
            let images = discover_images(input_dir, recursive);
            return BatchSummary {
                total: images.len(),
                succeeded: 0,
                failed: images
                    .iter()
                    .map(|p| (p.clone(), format!("preset load failed: {e}")))
                    .collect(),
                elapsed: Duration::ZERO,
            };
        }
    };

    let opts = BatchOpts {
        input_dir,
        output_dir,
        recursive,
        format_ext: format.map(|f| f.extension()),
        suffix,
        jobs,
        skip_errors,
    };
    run_batch(&opts, |input, output| {
        process_single(input, output, quality, format, |engine| {
            engine.apply_preset(&preset);
        })
    })
}

/// Run batch-edit: apply inline parameters to all images in a directory, in parallel.
#[allow(clippy::too_many_arguments)]
pub fn run_batch_edit(
    input_dir: &Path,
    output_dir: &Path,
    recursive: bool,
    params: &agx::Parameters,
    lut: Option<&agx::Lut3D>,
    quality: u8,
    format: Option<agx::encode::OutputFormat>,
    suffix: Option<&str>,
    jobs: usize,
    skip_errors: bool,
) -> BatchSummary {
    let opts = BatchOpts {
        input_dir,
        output_dir,
        recursive,
        format_ext: format.map(|f| f.extension()),
        suffix,
        jobs,
        skip_errors,
    };
    run_batch(&opts, |input, output| {
        process_single(input, output, quality, format, |engine| {
            engine.set_params(params.clone());
            if let Some(l) = lut {
                engine.set_lut(Some(l.clone()));
            }
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn discover_finds_image_files() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("photo.jpg"), b"").unwrap();
        fs::write(tmp.path().join("photo.jpeg"), b"").unwrap();
        fs::write(tmp.path().join("photo.png"), b"").unwrap();
        fs::write(tmp.path().join("notes.txt"), b"").unwrap();

        let found = discover_images(tmp.path(), false);
        assert_eq!(found.len(), 3);
        assert!(found.iter().all(|p| p.extension().unwrap() != "txt"));
    }

    #[test]
    fn discover_skips_non_image_files() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("readme.md"), b"").unwrap();
        fs::write(tmp.path().join("data.txt"), b"").unwrap();
        fs::write(tmp.path().join(".hidden"), b"").unwrap();

        let found = discover_images(tmp.path(), false);
        assert!(found.is_empty());
    }

    #[test]
    fn discover_recursive_finds_subdirs() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("a.jpg"), b"").unwrap();
        let sub = tmp.path().join("sub");
        fs::create_dir(&sub).unwrap();
        fs::write(sub.join("b.png"), b"").unwrap();

        let flat = discover_images(tmp.path(), false);
        assert_eq!(flat.len(), 1);

        let deep = discover_images(tmp.path(), true);
        assert_eq!(deep.len(), 2);
    }

    #[test]
    fn discover_case_insensitive_extensions() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("a.JPG"), b"").unwrap();
        fs::write(tmp.path().join("b.Png"), b"").unwrap();
        fs::write(tmp.path().join("c.TIFF"), b"").unwrap();

        let found = discover_images(tmp.path(), false);
        assert_eq!(found.len(), 3);
    }

    #[test]
    fn discover_sorted_by_name() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("charlie.jpg"), b"").unwrap();
        fs::write(tmp.path().join("alpha.png"), b"").unwrap();
        fs::write(tmp.path().join("bravo.tiff"), b"").unwrap();

        let found = discover_images(tmp.path(), false);
        let names: Vec<&str> = found
            .iter()
            .map(|p| p.file_name().unwrap().to_str().unwrap())
            .collect();
        assert_eq!(names, vec!["alpha.png", "bravo.tiff", "charlie.jpg"]);
    }

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

    #[test]
    fn batch_edit_processes_with_params() {
        let dir = TempDir::new().unwrap();
        let input_dir = dir.path().join("input");
        let output_dir = dir.path().join("output");
        fs::create_dir(&input_dir).unwrap();

        write_test_png(&input_dir.join("photo.png"));

        let params = agx::Parameters::default();

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
}
