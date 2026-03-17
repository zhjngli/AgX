use std::path::Path;
use tempfile::TempDir;

use agx_e2e::{assert_golden, fixture_path};

/// Helper: decode a file, run through engine with given params, encode to output.
fn process_with_params(input: &Path, output: &Path, configure: impl FnOnce(&mut agx::Engine)) {
    let image = agx::decode(input).expect("decode failed");
    let mut engine = agx::Engine::new(image);
    configure(&mut engine);
    let rendered = engine.render();
    agx::encode::encode_to_file(&rendered, output).expect("encode failed");
}

/// Helper: decode a file, apply a preset, render, encode.
fn process_with_preset(input: &Path, output: &Path, preset_path: &Path) {
    let image = agx::decode(input).expect("decode failed");
    let mut engine = agx::Engine::new(image);
    let preset = agx::Preset::load_from_file(preset_path).expect("preset load failed");
    engine.apply_preset(&preset);
    let rendered = engine.render();
    agx::encode::encode_to_file(&rendered, output).expect("encode failed");
}

/// Sanity check: output file exists, has non-zero size, dimensions > 0.
fn assert_valid_output(path: &Path) {
    assert!(
        path.exists(),
        "Output file should exist: {}",
        path.display()
    );
    let metadata = std::fs::metadata(path).unwrap();
    assert!(metadata.len() > 0, "Output file should not be empty");
    let img = image::open(path).expect("Output should be a valid image");
    assert!(img.width() > 0 && img.height() > 0);
}

/// Measure average brightness of an image (sRGB u8, all channels averaged).
fn average_brightness(path: &Path) -> f64 {
    let img = image::open(path).unwrap().to_rgb8();
    let total: u64 = img
        .pixels()
        .map(|p| p.0[0] as u64 + p.0[1] as u64 + p.0[2] as u64)
        .sum();
    total as f64 / (img.width() as f64 * img.height() as f64 * 3.0)
}

// ---- JPEG tests (golden comparison — deterministic across platforms) ----

#[test]
fn library_jpeg_default_params() {
    let input = fixture_path("jpeg/temple_blossoms.jpg");
    let dir = TempDir::new().unwrap();
    let output = dir.path().join("output.png");

    process_with_params(&input, &output, |_| {});
    assert_valid_output(&output);
    assert_golden(&output, "library_jpeg_default.png", 2);
}

#[test]
fn library_jpeg_hsl_adjustments() {
    let input = fixture_path("jpeg/temple_blossoms.jpg");
    let dir = TempDir::new().unwrap();
    let output = dir.path().join("output.png");

    process_with_params(&input, &output, |engine| {
        engine.params_mut().hsl.red.saturation = 30.0;
        engine.params_mut().hsl.blue.luminance = 10.0;
    });
    assert_valid_output(&output);
    assert_golden(&output, "library_jpeg_hsl.png", 2);
}

// ---- RAW tests (sanity checks only — LibRaw output varies by platform/version) ----

#[test]
fn library_raf_default_params() {
    let input = fixture_path("raw/night_city_blur.raf");
    let dir = TempDir::new().unwrap();
    let output = dir.path().join("output.png");

    process_with_params(&input, &output, |_| {});
    assert_valid_output(&output);
}

#[test]
fn library_raf_exposure_plus_one() {
    let input = fixture_path("raw/night_city_blur.raf");
    let dir = TempDir::new().unwrap();
    let output_neutral = dir.path().join("neutral.png");
    let output_bright = dir.path().join("bright.png");

    process_with_params(&input, &output_neutral, |_| {});
    process_with_params(&input, &output_bright, |engine| {
        engine.params_mut().exposure = 1.0;
    });

    assert_valid_output(&output_bright);

    // Directional sanity: +1 stop should be brighter
    let brightness_neutral = average_brightness(&output_neutral);
    let brightness_bright = average_brightness(&output_bright);
    assert!(
        brightness_bright > brightness_neutral,
        "Expected brighter after +1 exposure: neutral={:.1} bright={:.1}",
        brightness_neutral,
        brightness_bright
    );
}

#[test]
fn library_raf_warm_white_balance() {
    let input = fixture_path("raw/night_city_blur.raf");
    let dir = TempDir::new().unwrap();
    let output = dir.path().join("output.png");

    process_with_params(&input, &output, |engine| {
        engine.params_mut().temperature = 40.0;
    });
    assert_valid_output(&output);
}

#[test]
fn library_raf_with_preset() {
    let input = fixture_path("raw/night_city_blur.raf");
    let preset = fixture_path("presets/warm_exposure.toml");
    let dir = TempDir::new().unwrap();
    let output = dir.path().join("output.png");

    process_with_preset(&input, &output, &preset);
    assert_valid_output(&output);
}

#[test]
fn library_raf_sunset_river() {
    let input = fixture_path("raw/sunset_river.raf");
    let dir = TempDir::new().unwrap();
    let output = dir.path().join("output.png");

    process_with_params(&input, &output, |_| {});
    assert_valid_output(&output);
}

#[test]
fn library_raf_high_contrast_preset() {
    let input = fixture_path("raw/night_city_blur.raf");
    let preset = fixture_path("presets/high_contrast.toml");
    let dir = TempDir::new().unwrap();
    let output = dir.path().join("output.png");

    process_with_preset(&input, &output, &preset);
    assert_valid_output(&output);
}

// ---- Additional RAW tests ----

#[test]
fn library_raf_foggy_forest() {
    let input = fixture_path("raw/foggy_forest.raf");
    let dir = TempDir::new().unwrap();
    let output = dir.path().join("output.png");

    process_with_params(&input, &output, |_| {});
    assert_valid_output(&output);
}

#[test]
fn library_raf_foggy_forest_with_preset() {
    let input = fixture_path("raw/foggy_forest.raf");
    let preset = fixture_path("presets/warm_exposure.toml");
    let dir = TempDir::new().unwrap();
    let output = dir.path().join("output.png");

    process_with_preset(&input, &output, &preset);
    assert_valid_output(&output);
}

#[test]
fn library_raf_dusk_cityscape() {
    let input = fixture_path("raw/dusk_cityscape.raf");
    let dir = TempDir::new().unwrap();
    let output = dir.path().join("output.png");

    process_with_params(&input, &output, |_| {});
    assert_valid_output(&output);
}

#[test]
fn library_raf_dusk_cityscape_exposure() {
    let input = fixture_path("raw/dusk_cityscape.raf");
    let dir = TempDir::new().unwrap();
    let output_neutral = dir.path().join("neutral.png");
    let output_bright = dir.path().join("bright.png");

    process_with_params(&input, &output_neutral, |_| {});
    process_with_params(&input, &output_bright, |engine| {
        engine.params_mut().exposure = 1.5;
    });

    assert_valid_output(&output_bright);

    let brightness_neutral = average_brightness(&output_neutral);
    let brightness_bright = average_brightness(&output_bright);
    assert!(
        brightness_bright > brightness_neutral,
        "Expected brighter after +1.5 exposure: neutral={:.1} bright={:.1}",
        brightness_neutral,
        brightness_bright
    );
}

// ---- Additional JPEG tests ----

#[test]
fn library_jpeg_night_architecture_default() {
    let input = fixture_path("jpeg/night_architecture.jpg");
    let dir = TempDir::new().unwrap();
    let output = dir.path().join("output.png");

    process_with_params(&input, &output, |_| {});
    assert_valid_output(&output);
    assert_golden(&output, "library_jpeg_night_architecture_default.png", 2);
}

#[test]
fn library_jpeg_night_architecture_exposure() {
    let input = fixture_path("jpeg/night_architecture.jpg");
    let dir = TempDir::new().unwrap();
    let output = dir.path().join("output.png");

    process_with_params(&input, &output, |engine| {
        engine.params_mut().exposure = 2.0;
        engine.params_mut().contrast = 20.0;
    });
    assert_valid_output(&output);
    assert_golden(&output, "library_jpeg_night_architecture_bright.png", 2);
}
