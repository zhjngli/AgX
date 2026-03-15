use std::process::Command;

fn cli_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_agx-cli"))
}

fn create_test_png(path: &std::path::Path) {
    use image::{ImageBuffer, Rgb};
    let img: ImageBuffer<Rgb<u8>, Vec<u8>> = ImageBuffer::from_pixel(4, 4, Rgb([128u8, 128, 128]));
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
        .args([
            "apply",
            "-i",
            input.to_str().unwrap(),
            "-p",
            preset_path.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
        ])
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
            "-i",
            input.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
            "--exposure",
            "-1.0",
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
            "-i",
            "/nonexistent/photo.png",
            "-p",
            "/nonexistent/preset.toml",
            "-o",
            "/tmp/out.png",
        ])
        .output()
        .expect("failed to run CLI");

    assert!(
        !output.status.success(),
        "CLI should fail for missing input"
    );
}

fn create_identity_cube(path: &std::path::Path) {
    let mut lines = String::from("LUT_3D_SIZE 2\n");
    for b in 0..2 {
        for g in 0..2 {
            for r in 0..2 {
                lines.push_str(&format!("{}.0 {}.0 {}.0\n", r, g, b));
            }
        }
    }
    std::fs::write(path, lines).unwrap();
}

#[test]
fn cli_edit_with_lut() {
    let temp_dir = std::env::temp_dir();
    let input = temp_dir.join("oxiraw_cli_lut_in.png");
    let lut_path = temp_dir.join("oxiraw_cli_test.cube");
    let output = temp_dir.join("oxiraw_cli_lut_out.png");

    create_test_png(&input);
    create_identity_cube(&lut_path);

    let status = cli_bin()
        .args([
            "edit",
            "-i",
            input.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
            "--lut",
            lut_path.to_str().unwrap(),
        ])
        .status()
        .expect("failed to run CLI");

    assert!(status.success(), "CLI edit with LUT should succeed");
    assert!(output.exists(), "Output file should exist");

    let _ = std::fs::remove_file(&input);
    let _ = std::fs::remove_file(&lut_path);
    let _ = std::fs::remove_file(&output);
}

#[test]
fn cli_apply_preset_with_lut() {
    let temp_dir = std::env::temp_dir();
    let input = temp_dir.join("oxiraw_cli_preset_lut_in.png");
    let lut_path = temp_dir.join("oxiraw_cli_preset_lut.cube");
    let preset_path = temp_dir.join("oxiraw_cli_preset_lut.toml");
    let output = temp_dir.join("oxiraw_cli_preset_lut_out.png");

    create_test_png(&input);
    create_identity_cube(&lut_path);

    let preset_content = format!(
        "[metadata]\nname = \"LUT Preset\"\n\n[tone]\nexposure = 0.5\n\n[lut]\npath = \"{}\"\n",
        lut_path.file_name().unwrap().to_str().unwrap()
    );
    std::fs::write(&preset_path, &preset_content).unwrap();

    let status = cli_bin()
        .args([
            "apply",
            "-i",
            input.to_str().unwrap(),
            "-p",
            preset_path.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
        ])
        .status()
        .expect("failed to run CLI");

    assert!(status.success(), "CLI apply with LUT preset should succeed");
    assert!(output.exists());

    let _ = std::fs::remove_file(&input);
    let _ = std::fs::remove_file(&lut_path);
    let _ = std::fs::remove_file(&preset_path);
    let _ = std::fs::remove_file(&output);
}

/// Create a larger test PNG with varied pixel values for quality comparison tests.
fn create_test_png_large(path: &std::path::Path) {
    use image::{ImageBuffer, Rgb};
    let img: ImageBuffer<Rgb<u8>, Vec<u8>> = ImageBuffer::from_fn(64, 64, |x, y| {
        Rgb([(x * 4) as u8, (y * 4) as u8, ((x + y) * 2) as u8])
    });
    img.save(path).unwrap();
}

#[test]
fn cli_edit_with_quality() {
    let temp_dir = std::env::temp_dir();
    let input = temp_dir.join("oxiraw_cli_quality_in.png");
    let output_low = temp_dir.join("oxiraw_cli_q50.jpg");
    let output_high = temp_dir.join("oxiraw_cli_q95.jpg");

    create_test_png_large(&input);

    let status = cli_bin()
        .args([
            "edit",
            "-i",
            input.to_str().unwrap(),
            "-o",
            output_low.to_str().unwrap(),
            "--quality",
            "50",
        ])
        .status()
        .expect("failed to run CLI");
    assert!(status.success());

    let status = cli_bin()
        .args([
            "edit",
            "-i",
            input.to_str().unwrap(),
            "-o",
            output_high.to_str().unwrap(),
            "--quality",
            "95",
        ])
        .status()
        .expect("failed to run CLI");
    assert!(status.success());

    let size_low = std::fs::metadata(&output_low).unwrap().len();
    let size_high = std::fs::metadata(&output_high).unwrap().len();
    assert!(size_high > size_low, "q95 should be larger than q50");

    let _ = std::fs::remove_file(&input);
    let _ = std::fs::remove_file(&output_low);
    let _ = std::fs::remove_file(&output_high);
}

#[test]
fn cli_edit_with_format_override() {
    let temp_dir = std::env::temp_dir();
    let input = temp_dir.join("oxiraw_cli_fmt_in.png");
    let output = temp_dir.join("oxiraw_cli_fmt_out.png");

    create_test_png(&input);

    let status = cli_bin()
        .args([
            "edit",
            "-i",
            input.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
            "--format",
            "jpeg",
        ])
        .status()
        .expect("failed to run CLI");
    assert!(status.success());

    let expected = temp_dir.join("oxiraw_cli_fmt_out.png.jpeg");
    assert!(expected.exists(), "Should have appended .jpeg extension");

    let _ = std::fs::remove_file(&input);
    let _ = std::fs::remove_file(&expected);
}

/// Test that the CLI can process a raw file.
/// This test is ignored by default since it requires a sample raw file.
/// To run: place a .dng file at /tmp/oxiraw_test_sample.dng and run:
///   cargo test -p oxiraw-cli -- --ignored cli_edit_raw_file
#[test]
#[ignore]
fn cli_edit_raw_file() {
    let input = std::path::PathBuf::from("/tmp/oxiraw_test_sample.dng");
    if !input.exists() {
        eprintln!("Skipping: no sample raw file at {}", input.display());
        return;
    }

    let output = std::env::temp_dir().join("oxiraw_cli_raw_out.jpg");

    let status = cli_bin()
        .args([
            "edit",
            "-i",
            input.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
            "--exposure",
            "0.5",
        ])
        .status()
        .expect("failed to run CLI");

    assert!(status.success(), "CLI should process raw file successfully");
    assert!(output.exists(), "Output file should exist");

    let out_img = image::open(&output).unwrap();
    assert!(out_img.width() > 0);
    assert!(out_img.height() > 0);

    let _ = std::fs::remove_file(&output);
}

#[test]
fn cli_edit_with_hsl_flags() {
    let temp_dir = std::env::temp_dir();
    let input = temp_dir.join("oxiraw_cli_hsl_in.png");
    let output = temp_dir.join("oxiraw_cli_hsl_out.png");

    // Create a solid red image so HSL red-saturation changes are visible.
    let img: image::ImageBuffer<image::Rgb<u8>, Vec<u8>> =
        image::ImageBuffer::from_pixel(4, 4, image::Rgb([255u8, 0, 0]));
    img.save(&input).unwrap();

    let status = cli_bin()
        .args([
            "edit",
            "-i",
            input.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
            "--hsl-red-s",
            "-100",
        ])
        .status()
        .unwrap();
    assert!(status.success(), "CLI should succeed with HSL flags");
    assert!(output.exists(), "Output file should exist");

    let _ = std::fs::remove_file(&input);
    let _ = std::fs::remove_file(&output);
}

#[test]
fn cli_batch_apply_with_preset() {
    let dir = tempfile::tempdir().unwrap();
    let input_dir = dir.path().join("input");
    let output_dir = dir.path().join("output");
    std::fs::create_dir(&input_dir).unwrap();

    create_test_png(&input_dir.join("a.png"));
    create_test_png(&input_dir.join("b.png"));

    let preset_path = dir.path().join("test.toml");
    std::fs::write(
        &preset_path,
        "[metadata]\nname = \"test\"\nversion = \"1.0\"\nauthor = \"t\"\n\n[tone]\nexposure = 0.5\n",
    )
    .unwrap();

    let status = cli_bin()
        .args([
            "batch-apply",
            "--input-dir",
            input_dir.to_str().unwrap(),
            "-p",
            preset_path.to_str().unwrap(),
            "--output-dir",
            output_dir.to_str().unwrap(),
            "--jobs",
            "1",
        ])
        .status()
        .expect("failed to run CLI");

    assert!(status.success(), "batch-apply should succeed");
    assert!(output_dir.join("a.png").exists(), "a.png should exist");
    assert!(output_dir.join("b.png").exists(), "b.png should exist");
}

#[test]
fn cli_batch_edit_with_suffix() {
    let dir = tempfile::tempdir().unwrap();
    let input_dir = dir.path().join("input");
    let output_dir = dir.path().join("output");
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
        .expect("failed to run CLI");

    assert!(status.success(), "batch-edit should succeed");
    assert!(
        output_dir.join("photo_bright.png").exists(),
        "photo_bright.png should exist"
    );
}

#[test]
fn cli_batch_apply_recursive() {
    let dir = tempfile::tempdir().unwrap();
    let input_dir = dir.path().join("input");
    let output_dir = dir.path().join("output");
    let sub_dir = input_dir.join("sub");
    std::fs::create_dir_all(&sub_dir).unwrap();

    create_test_png(&input_dir.join("top.png"));
    create_test_png(&sub_dir.join("nested.png"));

    let preset_path = dir.path().join("test.toml");
    std::fs::write(
        &preset_path,
        "[metadata]\nname = \"test\"\nversion = \"1.0\"\nauthor = \"t\"\n\n[tone]\nexposure = 0.5\n",
    )
    .unwrap();

    let status = cli_bin()
        .args([
            "batch-apply",
            "--input-dir",
            input_dir.to_str().unwrap(),
            "-p",
            preset_path.to_str().unwrap(),
            "--output-dir",
            output_dir.to_str().unwrap(),
            "--recursive",
            "--jobs",
            "1",
        ])
        .status()
        .expect("failed to run CLI");

    assert!(status.success(), "batch-apply --recursive should succeed");
    assert!(
        output_dir.join("top.png").exists(),
        "top.png should exist in output"
    );
    assert!(
        output_dir.join("sub").join("nested.png").exists(),
        "sub/nested.png should exist in output"
    );
}

#[test]
fn cli_batch_apply_empty_dir_succeeds() {
    let dir = tempfile::tempdir().unwrap();
    let input_dir = dir.path().join("empty_input");
    let output_dir = dir.path().join("output");
    std::fs::create_dir(&input_dir).unwrap();

    let preset_path = dir.path().join("test.toml");
    std::fs::write(
        &preset_path,
        "[metadata]\nname = \"test\"\nversion = \"1.0\"\nauthor = \"t\"\n\n[tone]\nexposure = 0.5\n",
    )
    .unwrap();

    let status = cli_bin()
        .args([
            "batch-apply",
            "--input-dir",
            input_dir.to_str().unwrap(),
            "-p",
            preset_path.to_str().unwrap(),
            "--output-dir",
            output_dir.to_str().unwrap(),
        ])
        .status()
        .expect("failed to run CLI");

    assert!(
        status.success(),
        "batch-apply on empty dir should succeed gracefully"
    );
}

#[test]
fn cli_apply_multiple_presets() {
    let temp_dir = std::env::temp_dir();
    let input = temp_dir.join("oxiraw_cli_multi_in.png");
    let output = temp_dir.join("oxiraw_cli_multi_out.png");
    let preset1 = temp_dir.join("oxiraw_cli_multi_p1.toml");
    let preset2 = temp_dir.join("oxiraw_cli_multi_p2.toml");

    create_test_png(&input);

    std::fs::write(
        &preset1,
        "[metadata]\nname = \"P1\"\n\n[tone]\nexposure = 1.0\n",
    )
    .unwrap();
    std::fs::write(
        &preset2,
        "[metadata]\nname = \"P2\"\n\n[tone]\ncontrast = 20.0\n",
    )
    .unwrap();

    let status = cli_bin()
        .args([
            "apply",
            "-i",
            input.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
            "--presets",
            preset1.to_str().unwrap(),
            preset2.to_str().unwrap(),
        ])
        .status()
        .unwrap();
    assert!(status.success(), "CLI should succeed with --presets");
    assert!(output.exists(), "Output file should exist");

    let _ = std::fs::remove_file(&input);
    let _ = std::fs::remove_file(&output);
    let _ = std::fs::remove_file(&preset1);
    let _ = std::fs::remove_file(&preset2);
}
