use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

use agx_e2e::{assert_golden, fixture_path};

// --- Constants ---

const JPEG_IMAGES: &[(&str, &str)] = &[
    ("jpeg/temple_blossoms.jpg", "temple_blossoms"),
    ("jpeg/night_architecture.jpg", "night_architecture"),
];

const RAW_IMAGES: &[(&str, &str)] = &[
    ("raw/night_city_blur.raf", "night_city_blur"),
    ("raw/sunset_river.raf", "sunset_river"),
    ("raw/foggy_forest.raf", "foggy_forest"),
    ("raw/dusk_cityscape.raf", "dusk_cityscape"),
];

const LOOKS: &[&str] = &[
    "portra_400",
    "neo_noir",
    "blade_runner",
    "cinema_warm",
    "kodachrome_64",
    "nordic_fade",
];

// --- Helpers ---

fn cli_bin() -> Command {
    let mut path = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    path.push("agx-cli");
    assert!(
        path.exists(),
        "agx-cli binary not found at {}. Run `cargo build -p agx-cli` first.",
        path.display()
    );
    Command::new(path)
}

fn assert_valid_output(path: &Path) {
    assert!(
        path.exists(),
        "Output file should exist: {}",
        path.display()
    );
    let metadata = std::fs::metadata(path).unwrap();
    assert!(metadata.len() > 0, "Output file should not be empty");
}

fn look_preset_path(look: &str) -> std::path::PathBuf {
    fixture_path(&format!("looks/{look}.toml"))
}

// --- Noop tests (no adjustments applied) ---

#[test]
fn cli_jpeg_noop_matrix() {
    for &(image_path, image_name) in JPEG_IMAGES {
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
        assert_golden(&output, &format!("jpeg/{image_name}_noop.png"), 2, 0.0);
    }
}

#[test]
fn cli_raw_noop_matrix() {
    for &(image_path, image_name) in RAW_IMAGES {
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
        assert_golden(&output, &format!("raw/{image_name}_noop.png"), 30, 10.0);
    }
}

// --- JPEG x LOOK matrix ---

#[test]
fn cli_jpeg_look_matrix() {
    for &(image_path, image_name) in JPEG_IMAGES {
        for look in LOOKS {
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
            assert_golden(&output, &format!("jpeg/{image_name}_{look}.png"), 2, 0.0);
        }
    }
}

// --- RAW x LOOK matrix ---

#[test]
fn cli_raw_look_matrix() {
    for &(image_path, image_name) in RAW_IMAGES {
        for look in LOOKS {
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
            assert_golden(&output, &format!("raw/{image_name}_{look}.png"), 30, 10.0);
        }
    }
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
