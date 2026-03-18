use std::process::Command;
use tempfile::TempDir;

use agx_e2e::{assert_golden, assert_valid_output, fixture_path};

// --- Constants ---

const BW_LOOKS: &[&str] = &["bw_high_contrast", "bw_street", "bw_lofi"];

const ALL_LOOKS: &[&str] = &[
    "portra_400",
    "neo_noir",
    "blade_runner",
    "cinema_warm",
    "kodachrome_64",
    "nordic_fade",
    "bw_high_contrast",
    "bw_street",
    "bw_lofi",
];

// --- Helpers ---

fn cli_bin() -> Command {
    // agx-cli is a pure binary crate (no lib target), so CARGO_BIN_EXE is
    // unavailable. Locate it by walking up from the test binary directory.
    let target_dir = std::env::current_exe()
        .unwrap()
        .parent() // deps/
        .unwrap()
        .parent() // debug/ or release/
        .unwrap()
        .parent() // target/
        .unwrap()
        .to_path_buf();

    // Prefer release binary (much faster for image processing)
    let release = target_dir.join("release").join("agx-cli");
    let debug = target_dir.join("debug").join("agx-cli");
    let path = if release.exists() { release } else { debug };

    assert!(
        path.exists(),
        "agx-cli binary not found at {} or {}. Run `cargo build --release -p agx-cli` first.",
        release.display(),
        debug.display(),
    );
    Command::new(path)
}

fn look_preset_path(look: &str) -> std::path::PathBuf {
    fixture_path(&format!("looks/{look}.toml"))
}

/// Run noop + specified looks for a single image. This consolidates all CLI invocations
/// for one image into a single test function, enabling Cargo to parallelize across images.
fn run_image_matrix(
    image_path: &str,
    image_name: &str,
    golden_dir: &str,
    tolerance: u8,
    max_diff_pct: f64,
    looks: &[&str],
) {
    // Noop (no adjustments)
    {
        let input = fixture_path(image_path);
        let dir = TempDir::new().unwrap();
        let output = dir.path().join("output.png");

        let status = cli_bin()
            .args([
                "edit",
                "-i",
                input.to_str().unwrap(),
                "-o",
                output.to_str().unwrap(),
            ])
            .status()
            .expect("failed to run CLI");

        assert!(status.success(), "CLI edit should succeed for {image_name}");
        assert_valid_output(&output);
        assert_golden(
            &output,
            &format!("{golden_dir}/{image_name}_noop.png"),
            tolerance,
            max_diff_pct,
        );
    }

    // Apply each look
    for look in looks {
        let input = fixture_path(image_path);
        let preset = look_preset_path(look);
        let dir = TempDir::new().unwrap();
        let output = dir.path().join("output.png");

        let status = cli_bin()
            .args([
                "apply",
                "-i",
                input.to_str().unwrap(),
                "-p",
                preset.to_str().unwrap(),
                "-o",
                output.to_str().unwrap(),
            ])
            .status()
            .expect("failed to run CLI");

        assert!(
            status.success(),
            "CLI apply should succeed for {image_name} with {look}"
        );
        assert_valid_output(&output);
        assert_golden(
            &output,
            &format!("{golden_dir}/{image_name}_{look}.png"),
            tolerance,
            max_diff_pct,
        );
    }
}

// --- Per-image tests (enables parallelism: each test function runs concurrently) ---

// --- Color images: noop + all looks (color + B&W conversion) ---

#[test]
fn cli_temple_blossoms() {
    run_image_matrix(
        "jpeg/temple_blossoms.jpg",
        "temple_blossoms",
        "jpeg",
        2,
        0.0,
        ALL_LOOKS,
    );
}

#[test]
fn cli_night_city_blur() {
    run_image_matrix(
        "raw/night_city_blur.raf",
        "night_city_blur",
        "raw",
        100,
        25.0,
        ALL_LOOKS,
    );
}

#[test]
fn cli_sunset_river() {
    run_image_matrix(
        "raw/sunset_river.raf",
        "sunset_river",
        "raw",
        100,
        25.0,
        ALL_LOOKS,
    );
}

#[test]
fn cli_foggy_forest() {
    run_image_matrix(
        "raw/foggy_forest.raf",
        "foggy_forest",
        "raw",
        100,
        25.0,
        ALL_LOOKS,
    );
}

#[test]
fn cli_dusk_cityscape() {
    run_image_matrix(
        "raw/dusk_cityscape.raf",
        "dusk_cityscape",
        "raw",
        100,
        25.0,
        ALL_LOOKS,
    );
}

// --- B&W images: noop + B&W looks only (color looks are meaningless on B&W) ---

#[test]
fn cli_night_architecture() {
    run_image_matrix(
        "jpeg/night_architecture.jpg",
        "night_architecture",
        "jpeg",
        2,
        0.0,
        BW_LOOKS,
    );
}

// --- Batch test ---

#[test]
fn cli_batch_edit_mixed_dir() {
    let dir = TempDir::new().unwrap();
    let input_dir = dir.path().join("input");
    let output_dir = dir.path().join("output");
    std::fs::create_dir(&input_dir).unwrap();

    let jpeg_src = fixture_path("jpeg/temple_blossoms.jpg");
    std::fs::copy(&jpeg_src, input_dir.join("temple_blossoms.jpg")).unwrap();

    let status = cli_bin()
        .args([
            "batch-edit",
            "--input-dir",
            input_dir.to_str().unwrap(),
            "--output-dir",
            output_dir.to_str().unwrap(),
            "--exposure",
            "0.5",
            "--jobs",
            "1",
        ])
        .status()
        .expect("failed to run CLI");

    assert!(status.success(), "batch-edit should succeed");
    assert!(
        output_dir.join("temple_blossoms.jpg").exists(),
        "Output file should exist"
    );
}

// --- Error cases ---

#[test]
fn cli_corrupt_file_fails_gracefully() {
    let dir = TempDir::new().unwrap();
    let corrupt = dir.path().join("corrupt.raf");
    let output = dir.path().join("output.png");

    std::fs::write(&corrupt, b"this is not a real RAF file").unwrap();

    let result = cli_bin()
        .args([
            "edit",
            "-i",
            corrupt.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
        ])
        .output()
        .expect("failed to run CLI");

    assert!(!result.status.success(), "CLI should fail for corrupt file");
    assert!(
        !output.exists(),
        "No output should be produced for corrupt file"
    );
}

#[test]
fn cli_nonexistent_input_fails() {
    let dir = TempDir::new().unwrap();
    let output = dir.path().join("output.png");

    let result = cli_bin()
        .args([
            "edit",
            "-i",
            "/nonexistent/photo.raf",
            "-o",
            output.to_str().unwrap(),
        ])
        .output()
        .expect("failed to run CLI");

    assert!(
        !result.status.success(),
        "CLI should fail for nonexistent input"
    );
}
