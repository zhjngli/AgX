use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use rayon::prelude::*;

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
    let name = input
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("?");
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

/// Process a single image with a preset (used by batch-apply).
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
    _skip_errors: bool,
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

    let num_threads = if jobs == 0 {
        rayon::current_num_threads()
    } else {
        jobs
    };
    eprintln!("Processing {total} images with {num_threads} workers...");

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(jobs)
        .build()
        .expect("failed to create thread pool");

    let results: Vec<BatchResult> = pool.install(|| {
        images
            .par_iter()
            .map(|input| {
                let output =
                    resolve_output_path(input, input_dir, output_dir, suffix, format_ext);
                let outcome = process_apply_single(input, &output, &preset, quality, format);
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

/// Process a single image with inline parameters (used by batch-edit).
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
    _skip_errors: bool,
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

    let num_threads = if jobs == 0 {
        rayon::current_num_threads()
    } else {
        jobs
    };
    eprintln!("Processing {total} images with {num_threads} workers...");

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(jobs)
        .build()
        .expect("failed to create thread pool");

    let results: Vec<BatchResult> = pool.install(|| {
        images
            .par_iter()
            .map(|input| {
                let output =
                    resolve_output_path(input, input_dir, output_dir, suffix, format_ext);
                let outcome = process_edit_single(input, &output, params, lut, quality, format);
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
        let names: Vec<_> = found
            .iter()
            .map(|p| p.file_name().unwrap().to_str().unwrap())
            .collect();
        assert_eq!(names, vec!["a.jpg", "b.jpg", "c.jpg"]);
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

    fn write_test_png(path: &std::path::Path) {
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
}
