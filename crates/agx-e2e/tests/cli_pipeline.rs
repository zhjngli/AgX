use std::process::Command;
use tempfile::TempDir;

use agx_e2e::{assert_golden, assert_valid_output, fixture_path};

// --- Constants ---

const ALL_LOOKS: &[&str] = &[
    "portra_400",
    "kodachrome_64",
    "cinestill_800t",
    "tri_x_400",
    "tmax_100",
    "high_contrast_bw",
    "faded_bw",
    "blade_runner",
    "neo_noir",
    "cinema_warm",
    "dune",
];

const BW_LOOKS: &[&str] = &["tri_x_400", "tmax_100", "high_contrast_bw", "faded_bw"];

// --- Helpers ---

fn cli_bin() -> Command {
    // The agx binary (package agx-cli) has no lib target, so CARGO_BIN_EXE is
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
    let release = target_dir.join("release").join("agx");
    let debug = target_dir.join("debug").join("agx");
    let path = if release.exists() {
        release
    } else if debug.exists() {
        debug
    } else {
        panic!(
            "agx binary not found. Checked:\n  {}\n  {}\nRun `cargo build --release -p agx-cli` first.",
            target_dir.join("release/agx").display(),
            target_dir.join("debug/agx").display(),
        )
    };
    Command::new(path)
}

fn look_preset_path(look: &str) -> std::path::PathBuf {
    fixture_path(&format!("looks/{look}.toml"))
}

/// Run multi-apply for a single image with noop + specified looks, then assert goldens.
/// This calls the CLI once per image (decode-once), not once per preset.
fn run_image_matrix(
    image_path: &str,
    image_name: &str,
    golden_dir: &str,
    tolerance: u8,
    max_diff_pct: f64,
    looks: &[&str],
) {
    let dir = TempDir::new().unwrap();
    let input = fixture_path(image_path);

    // Build multi-apply command: decode once, render all presets + noop
    let mut cmd = cli_bin();
    cmd.args([
        "multi-apply",
        "-i",
        input.to_str().unwrap(),
        "-o",
        dir.path().to_str().unwrap(),
        "--noop",
    ]);
    for look in looks {
        let preset = look_preset_path(look);
        cmd.args(["-p", preset.to_str().unwrap()]);
    }

    let status = cmd.status().expect("failed to run multi-apply");
    assert!(
        status.success(),
        "multi-apply should succeed for {image_name}"
    );

    // Assert noop golden
    let noop_output = dir.path().join(format!("{image_name}_noop.png"));
    assert_valid_output(&noop_output);
    assert_golden(
        &noop_output,
        &format!("{golden_dir}/{image_name}_noop.png"),
        tolerance,
        max_diff_pct,
    );

    // Assert each look golden
    for look in looks {
        let output = dir.path().join(format!("{image_name}_{look}.png"));
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
fn cli_temple_blossoms_heic() {
    run_image_matrix(
        "heic/temple_blossoms.heic",
        "temple_blossoms",
        "heic",
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
