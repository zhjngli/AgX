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
    "p3_heavy_edit",
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
fn cli_cinque_terre_window() {
    run_image_matrix(
        "jpeg/cinque_terre_window.jpg",
        "cinque_terre_window",
        "jpeg",
        2,
        0.0,
        ALL_LOOKS,
    );
}

// HEIC tests share a tolerance of (10, 1.0). libheif/libde265 decode is
// mostly deterministic, but cross-platform version jitter shows up at
// LUT-amplified boundary pixels (~0.07% of pixels, max channel diff ~4 in
// practice). Tighter than the raw path (which absorbs LibRaw demosaicing
// variance).

#[test]
fn cli_marina_sunset() {
    run_image_matrix(
        "heic/marina_sunset.heic",
        "marina_sunset",
        "heic",
        10,
        1.0,
        ALL_LOOKS,
    );
}

#[test]
fn cli_concert_hall() {
    run_image_matrix(
        "heic/concert_hall.heic",
        "concert_hall",
        "heic",
        10,
        1.0,
        ALL_LOOKS,
    );
}

#[test]
fn cli_mountain_valley() {
    run_image_matrix(
        "heic/mountain_valley.heic",
        "mountain_valley",
        "heic",
        10,
        1.0,
        ALL_LOOKS,
    );
}

#[test]
fn cli_sky_moon_wires() {
    run_image_matrix(
        "heic/sky_moon_wires.heic",
        "sky_moon_wires",
        "heic",
        10,
        1.0,
        ALL_LOOKS,
    );
}

#[test]
fn cli_synthetic_p3_red() {
    // Display P3 fixture exercising the wide-gamut decode path (matrix
    // to linear Rec.2020 rather than the prior P3-to-sRGB squash). The
    // synthetic red gradient lives outside the sRGB gamut, so the noop
    // golden captures whether wide-gamut R survives the round trip,
    // and the look goldens capture how each look handles wide-gamut R.
    run_image_matrix(
        "heic/synthetic_p3_red.heic",
        "synthetic_p3_red",
        "heic",
        // Same shared HEIC tolerance — wide-gamut math amplifies tiny
        // cross-version libheif drifts further on this synthetic fixture.
        10,
        1.0,
        ALL_LOOKS,
    );
}

#[test]
fn cli_adobe_rgb_gradient() {
    // Synthetic JPEG tagged with an embedded Adobe RGB (1998) ICC profile,
    // exercising the SP3 input-ICC read path: the decoder must parse the
    // profile and convert wide-gamut color into the working space via lcms2
    // rather than assuming sRGB. JPEG-strict tolerance — the fixture is
    // deterministic. `p3_heavy_edit` saturates the wide-gamut content; the
    // noop golden captures the raw ICC-honored decode.
    run_image_matrix(
        "jpeg/adobe_rgb_gradient.jpg",
        "adobe_rgb_gradient",
        "jpeg",
        2,
        0.0,
        &["p3_heavy_edit", "portra_400"],
    );
}

#[test]
fn cli_prophoto_gradient() {
    // Synthetic PNG tagged with an embedded ProPhoto RGB ICC profile (iCCP
    // chunk). Exercises PNG ICC extraction *and* the widest-gamut input
    // conversion in the suite (ProPhoto extends well beyond Rec.2020; lcms2
    // gamut-maps it into the working space). Lossless format → strict tolerance.
    run_image_matrix(
        "png/prophoto_gradient.png",
        "prophoto_gradient",
        "png",
        2,
        0.0,
        &["p3_heavy_edit", "portra_400"],
    );
}

#[test]
fn cli_adobe_rgb_tiff() {
    // Synthetic TIFF tagged with an embedded Adobe RGB ICC profile (ICCProfile
    // tag 0x8773). Exercises TIFF ICC-tag extraction — the only TIFF *input* in
    // the suite. Lossless format → strict tolerance.
    // `image_name` must match the input file stem — the CLI derives output
    // filenames from it. Goldens live under their own `tiff/` dir, so this does
    // not collide with the JPEG `adobe_rgb_gradient` fixture.
    run_image_matrix(
        "tiff/adobe_rgb_gradient.tiff",
        "adobe_rgb_gradient",
        "tiff",
        2,
        0.0,
        &["p3_heavy_edit", "portra_400"],
    );
}

#[test]
fn cli_cinque_terre_manarola() {
    run_image_matrix(
        "raw/cinque_terre_manarola.raf",
        "cinque_terre_manarola",
        "raw",
        100,
        25.0,
        ALL_LOOKS,
    );
}

#[test]
fn cli_grand_canyon_overlook() {
    run_image_matrix(
        "raw/grand_canyon_overlook.raf",
        "grand_canyon_overlook",
        "raw",
        100,
        25.0,
        ALL_LOOKS,
    );
}

// --- B&W images: noop + B&W looks only (color looks are meaningless on B&W) ---

#[test]
fn cli_geisel_library_bw() {
    run_image_matrix(
        "jpeg/geisel_library_bw.jpg",
        "geisel_library_bw",
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

    let jpeg_src = fixture_path("jpeg/cinque_terre_window.jpg");
    std::fs::copy(&jpeg_src, input_dir.join("cinque_terre_window.jpg")).unwrap();

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
        output_dir.join("cinque_terre_window.jpg").exists(),
        "Output file should exist"
    );
}

// --- Error cases ---

// --- EXIF orientation preservation ---

/// Read the EXIF Orientation tag (0x0112) from an image, or `None` if absent.
fn read_orientation_tag(path: &std::path::Path) -> Option<u16> {
    let file = std::fs::File::open(path).ok()?;
    let mut reader = std::io::BufReader::new(file);
    let exif = exif::Reader::new().read_from_container(&mut reader).ok()?;
    let field = exif.get_field(exif::Tag::Orientation, exif::In::PRIMARY)?;
    field.value.get_uint(0).map(|v| v as u16)
}

/// Source `heic/marina_sunset.heic` carries orientation `Rotate 90 CW`
/// (sensor frame 4032×3024). After the fix, the output must carry
/// orientation 1 with dimensions matching the rotated canonical frame —
/// otherwise EXIF-aware viewers rotate the already-canonical pixels again.
#[test]
fn cli_exif_orientation_normalized_on_heic_output() {
    let dir = TempDir::new().unwrap();
    let input = fixture_path("heic/marina_sunset.heic");

    // Sanity-check the fixture: if the source no longer carries a non-Normal
    // orientation tag, this test is no longer exercising the regression.
    let source_orient = read_orientation_tag(&input);
    assert!(
        matches!(source_orient, Some(o) if o != 1),
        "fixture must carry non-Normal orientation to exercise this test, got {source_orient:?}"
    );

    let output = dir.path().join("out.jpg");
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
    assert!(status.success(), "edit should succeed");

    let output_orient = read_orientation_tag(&output);
    assert_eq!(
        output_orient,
        Some(1),
        "output EXIF orientation must be 1 (Normal), got {output_orient:?}"
    );

    // Output pixel dimensions should match the rotated canonical frame
    // (3024×4032 for a Rotate-90-CW 4032×3024 source). Cheap check that the
    // decode-side rotation actually ran.
    let img = image::open(&output).expect("output should decode");
    assert_eq!(
        (img.width(), img.height()),
        (3024, 4032),
        "output dimensions should match rotated canonical frame"
    );
}

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

/// Output-gamut coverage: render the wide-gamut Display P3 HEIC source through an
/// identity `edit` at each output gamut and pin goldens. Exercises working-space
/// → target conversion end-to-end (srgb baseline + the two wider boxes). HEIC
/// tolerance mirrors the other HEIC tests (10, 1.0) for libheif version jitter.
#[test]
fn cli_output_gamut_matrix() {
    for gamut in ["srgb", "p3", "adobe-rgb"] {
        let dir = TempDir::new().unwrap();
        let input = fixture_path("heic/marina_sunset.heic");
        let out = dir.path().join("out.png");

        let mut cmd = cli_bin();
        cmd.args([
            "edit",
            "-i",
            input.to_str().unwrap(),
            "-o",
            out.to_str().unwrap(),
            "--output-gamut",
            gamut,
        ]);
        let status = cmd.status().expect("failed to run edit");
        assert!(
            status.success(),
            "edit --output-gamut {gamut} should succeed"
        );

        assert_valid_output(&out);
        assert_golden(
            &out,
            &format!("output_gamut/marina_sunset_{gamut}.png"),
            10,
            1.0,
        );
    }
}
