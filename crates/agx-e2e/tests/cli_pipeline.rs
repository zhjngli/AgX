use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

use agx_e2e::{assert_golden, fixture_path};

fn cli_bin() -> Command {
    // Locate the agx-cli binary relative to the test binary.
    // Test binary is in target/debug/deps/, CLI binary is in target/debug/.
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

/// Sanity check on output file.
fn assert_valid_output(path: &Path) {
    assert!(
        path.exists(),
        "Output file should exist: {}",
        path.display()
    );
    let metadata = std::fs::metadata(path).unwrap();
    assert!(metadata.len() > 0, "Output file should not be empty");
}

// ---- RAW CLI tests ----

#[test]
#[ignore] // Remove when RAF fixtures are added
fn cli_raf_basic_edit() {
    let input = fixture_path("raw/daylight.raf");
    let dir = TempDir::new().unwrap();
    let output = dir.path().join("output.png");

    let status = cli_bin()
        .args([
            "edit",
            "-i",
            input.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
            "--exposure",
            "1.0",
        ])
        .status()
        .expect("failed to run CLI");

    assert!(status.success(), "CLI edit should succeed");
    assert_valid_output(&output);
    assert_golden(&output, "cli_raf_basic_edit.png", 2);
}

// ---- JPEG CLI tests ----

#[test]
#[ignore] // Remove when JPEG fixtures are added
fn cli_jpeg_apply_preset() {
    let input = fixture_path("jpeg/sample.jpg");
    let preset = fixture_path("presets/warm_exposure.toml");
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

    assert!(status.success(), "CLI apply should succeed");
    assert_valid_output(&output);
    assert_golden(&output, "cli_jpeg_apply_preset.png", 2);
}

// ---- Batch CLI tests ----

#[test]
#[ignore] // Remove when fixtures are added
fn cli_batch_edit_mixed_dir() {
    let dir = TempDir::new().unwrap();
    let input_dir = dir.path().join("input");
    let output_dir = dir.path().join("output");
    std::fs::create_dir(&input_dir).unwrap();

    // Copy fixture files into input dir
    let jpeg_src = fixture_path("jpeg/sample.jpg");
    std::fs::copy(&jpeg_src, input_dir.join("sample.jpg")).unwrap();

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
        output_dir.join("sample.jpg").exists(),
        "Output file should exist"
    );
}

// ---- Error cases ----

#[test]
fn cli_corrupt_file_fails_gracefully() {
    let dir = TempDir::new().unwrap();
    let corrupt = dir.path().join("corrupt.raf");
    let output = dir.path().join("output.png");

    // Write garbage data with a RAW extension
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
