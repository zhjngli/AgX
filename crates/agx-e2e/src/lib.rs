// E2E test utilities for agx.
// This crate has no library consumers — it exists only to hold integration tests.

use std::path::{Path, PathBuf};

/// Error returned when images don't match within tolerance.
#[derive(Debug)]
pub struct ComparisonError {
    pub differing_pixels: usize,
    pub total_pixels: usize,
    pub max_channel_diff: u8,
    pub diff_percentage: f64,
}

impl std::fmt::Display for ComparisonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} of {} pixels differ ({:.2}%), max channel diff: {}",
            self.differing_pixels, self.total_pixels, self.diff_percentage, self.max_channel_diff
        )
    }
}

/// Compare two images pixel-by-pixel with a per-channel tolerance.
///
/// Returns Ok(()) if all pixels match within tolerance, or a ComparisonError
/// describing the differences.
pub fn compare_images(actual: &Path, golden: &Path, tolerance: u8) -> Result<(), ComparisonError> {
    let actual_img = image::open(actual)
        .unwrap_or_else(|e| panic!("Failed to open actual image {}: {}", actual.display(), e))
        .to_rgb8();
    let golden_img = image::open(golden)
        .unwrap_or_else(|e| panic!("Failed to open golden image {}: {}", golden.display(), e))
        .to_rgb8();

    assert_eq!(
        actual_img.dimensions(),
        golden_img.dimensions(),
        "Image dimensions differ: actual {:?} vs golden {:?}",
        actual_img.dimensions(),
        golden_img.dimensions()
    );

    let mut differing_pixels = 0usize;
    let mut max_channel_diff = 0u8;
    let total_pixels = (actual_img.width() * actual_img.height()) as usize;

    for (a, g) in actual_img.pixels().zip(golden_img.pixels()) {
        let mut pixel_differs = false;
        for ch in 0..3 {
            let diff = (a.0[ch] as i16 - g.0[ch] as i16).unsigned_abs() as u8;
            if diff > max_channel_diff {
                max_channel_diff = diff;
            }
            if diff > tolerance {
                pixel_differs = true;
            }
        }
        if pixel_differs {
            differing_pixels += 1;
        }
    }

    if differing_pixels > 0 {
        Err(ComparisonError {
            differing_pixels,
            total_pixels,
            max_channel_diff,
            diff_percentage: (differing_pixels as f64 / total_pixels as f64) * 100.0,
        })
    } else {
        Ok(())
    }
}

/// Resolve a path relative to the fixtures directory.
pub fn fixture_path(relative: &str) -> PathBuf {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest_dir.join("fixtures").join(relative)
}

/// Resolve a path relative to the golden directory.
pub fn golden_path(name: &str) -> PathBuf {
    fixture_path(&format!("golden/{}", name))
}

/// Check if GOLDEN_UPDATE=1 is set, indicating golden files should be regenerated.
pub fn should_update_golden() -> bool {
    std::env::var("GOLDEN_UPDATE").map_or(false, |v| v == "1")
}

/// Compare actual output against golden file, or update the golden if GOLDEN_UPDATE=1.
///
/// This is the main assertion helper for e2e tests. Usage:
/// ```ignore
/// assert_golden(output_path, "test_name.png", 2);
/// ```
pub fn assert_golden(actual: &Path, golden_name: &str, tolerance: u8) {
    let golden = golden_path(golden_name);

    if should_update_golden() {
        if let Some(parent) = golden.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::copy(actual, &golden).unwrap();
        eprintln!("Updated golden: {}", golden.display());
        return;
    }

    if !golden.exists() {
        panic!(
            "Golden file not found: {}\nRun with GOLDEN_UPDATE=1 to generate it.",
            golden.display()
        );
    }

    if let Err(e) = compare_images(actual, &golden, tolerance) {
        panic!(
            "Golden comparison failed for '{}':\n  {}\nRun with GOLDEN_UPDATE=1 to update.",
            golden_name, e
        );
    }
}
