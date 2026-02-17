use std::process::Command;

fn cli_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_oxiraw-cli"))
}

fn create_test_png(path: &std::path::Path) {
    use image::{ImageBuffer, Rgb};
    let img: ImageBuffer<Rgb<u8>, Vec<u8>> =
        ImageBuffer::from_pixel(4, 4, Rgb([128u8, 128, 128]));
    img.save(path).unwrap();
}

#[test]
fn cli_apply_produces_output_file() {
    let temp_dir = std::env::temp_dir();
    let input = temp_dir.join("oxiraw_cli_apply_in.png");
    let preset_path = temp_dir.join("oxiraw_cli_apply.toml");
    let output = temp_dir.join("oxiraw_cli_apply_out.png");

    create_test_png(&input);
    std::fs::write(
        &preset_path,
        r#"
[metadata]
name = "Test"

[tone]
exposure = 1.0

[white_balance]
"#,
    )
    .unwrap();

    let status = cli_bin()
        .args(["apply", "-i", input.to_str().unwrap(), "-p", preset_path.to_str().unwrap(), "-o", output.to_str().unwrap()])
        .status()
        .expect("failed to run CLI");

    assert!(status.success(), "CLI apply should succeed");
    assert!(output.exists(), "Output file should exist");

    // Verify output is brighter than input
    let out_img = image::open(&output).unwrap().to_rgb8();
    let pixel = out_img.get_pixel(0, 0);
    assert!(
        pixel.0[0] > 140,
        "Expected brighter than 128 after +1 exposure, got {}",
        pixel.0[0]
    );

    let _ = std::fs::remove_file(&input);
    let _ = std::fs::remove_file(&preset_path);
    let _ = std::fs::remove_file(&output);
}

#[test]
fn cli_edit_with_inline_params() {
    let temp_dir = std::env::temp_dir();
    let input = temp_dir.join("oxiraw_cli_edit_in.png");
    let output = temp_dir.join("oxiraw_cli_edit_out.png");

    create_test_png(&input);

    let status = cli_bin()
        .args([
            "edit",
            "-i", input.to_str().unwrap(),
            "-o", output.to_str().unwrap(),
            "--exposure", "-1.0",
        ])
        .status()
        .expect("failed to run CLI");

    assert!(status.success(), "CLI edit should succeed");
    assert!(output.exists(), "Output file should exist");

    // Verify output is darker
    let out_img = image::open(&output).unwrap().to_rgb8();
    let pixel = out_img.get_pixel(0, 0);
    assert!(
        pixel.0[0] < 120,
        "Expected darker than 128 after -1 exposure, got {}",
        pixel.0[0]
    );

    let _ = std::fs::remove_file(&input);
    let _ = std::fs::remove_file(&output);
}

#[test]
fn cli_missing_input_fails() {
    let output = cli_bin()
        .args([
            "apply",
            "-i", "/nonexistent/photo.png",
            "-p", "/nonexistent/preset.toml",
            "-o", "/tmp/out.png",
        ])
        .output()
        .expect("failed to run CLI");

    assert!(!output.status.success(), "CLI should fail for missing input");
}
