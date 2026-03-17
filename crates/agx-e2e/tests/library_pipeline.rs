use std::path::Path;
use tempfile::TempDir;

use agx_e2e::{assert_valid_output, fixture_path};

/// Helper: decode a file, run through engine with given params, encode to output.
fn process_with_params(input: &Path, output: &Path, configure: impl FnOnce(&mut agx::Engine)) {
    let image = agx::decode(input).expect("decode failed");
    let mut engine = agx::Engine::new(image);
    configure(&mut engine);
    let rendered = engine.render();
    agx::encode::encode_to_file(&rendered, output).expect("encode failed");
}

// --- API smoke tests (one per concern, not a full matrix) ---

#[test]
fn library_jpeg_noop_roundtrip() {
    let input = fixture_path("jpeg/temple_blossoms.jpg");
    let dir = TempDir::new().unwrap();
    let output = dir.path().join("output.png");

    process_with_params(&input, &output, |_| {});
    assert_valid_output(&output);
}

#[test]
fn library_raw_noop_roundtrip() {
    let input = fixture_path("raw/night_city_blur.raf");
    let dir = TempDir::new().unwrap();
    let output = dir.path().join("output.png");

    process_with_params(&input, &output, |_| {});
    assert_valid_output(&output);
}

#[test]
fn library_apply_preset() {
    let input = fixture_path("jpeg/temple_blossoms.jpg");
    let preset_path = fixture_path("looks/portra_400.toml");
    let dir = TempDir::new().unwrap();
    let output = dir.path().join("output.png");

    let image = agx::decode(&input).expect("decode failed");
    let mut engine = agx::Engine::new(image);
    let preset = agx::Preset::load_from_file(&preset_path).expect("preset load failed");
    engine.apply_preset(&preset);
    let rendered = engine.render();
    agx::encode::encode_to_file(&rendered, &output).expect("encode failed");

    assert_valid_output(&output);
}

#[test]
fn library_direct_params() {
    let input = fixture_path("jpeg/temple_blossoms.jpg");
    let dir = TempDir::new().unwrap();
    let output = dir.path().join("output.png");

    process_with_params(&input, &output, |engine| {
        engine.params_mut().exposure = 1.0;
        engine.params_mut().contrast = 15.0;
        engine.params_mut().hsl.red.saturation = 20.0;
    });
    assert_valid_output(&output);
}

#[test]
fn library_lut_load_and_apply() {
    let input = fixture_path("jpeg/temple_blossoms.jpg");
    let lut_path = fixture_path("looks/luts/portra_400.cube");
    let dir = TempDir::new().unwrap();
    let output = dir.path().join("output.png");

    let image = agx::decode(&input).expect("decode failed");
    let mut engine = agx::Engine::new(image);
    let lut = agx::Lut3D::from_cube_file(&lut_path).expect("LUT load failed");
    engine.set_lut(Some(lut));
    let rendered = engine.render();
    agx::encode::encode_to_file(&rendered, &output).expect("encode failed");

    assert_valid_output(&output);
}

#[test]
fn library_preset_with_extends() {
    let input = fixture_path("jpeg/temple_blossoms.jpg");
    let preset_path = fixture_path("looks/blade_runner.toml");
    let dir = TempDir::new().unwrap();
    let output = dir.path().join("output.png");

    let image = agx::decode(&input).expect("decode failed");
    let mut engine = agx::Engine::new(image);
    let preset =
        agx::Preset::load_from_file(&preset_path).expect("preset with extends should load");
    engine.apply_preset(&preset);
    let rendered = engine.render();
    agx::encode::encode_to_file(&rendered, &output).expect("encode failed");

    assert_valid_output(&output);
}
