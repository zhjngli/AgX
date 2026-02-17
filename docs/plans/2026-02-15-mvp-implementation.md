# Oxiraw MVP Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Implement a working photo editing library + CLI that can decode images, apply basic tone adjustments (exposure, contrast, highlights, shadows, whites, blacks, white balance), save/load TOML presets, and produce edited output files.

**Architecture:** Always-re-render-from-original engine. Original image stored in linear sRGB f32. Render pipeline applies white balance + exposure in linear space, then converts to sRGB gamma space for contrast/highlights/shadows/whites/blacks, then converts back to linear for output. Presets are declarative TOML files.

**Tech Stack:** Rust 2021, image 0.25, imageproc 0.25, palette 0.7, toml 0.8, serde 1, thiserror 2, clap 4

---

## Context

This plan implements the MVP for oxiraw, an open-source photo editing library. The project was scaffolded in the previous session with empty module stubs. The architecture design doc is at `docs/plans/2026-02-14-architecture-design.md`.

Key design decisions already made:
- Engine always re-renders from the original image (order-independent from user's perspective)
- Operations are NOT mathematically commutative — the engine applies them in a fixed internal order but this is transparent to users/presets
- Presets are purely declarative parameter values in TOML
- **Preset format and CLI interface are illustrative and subject to change** — we want flexibility for composable presets, shortcuts, etc.

## Color Space Decision (MVP)

**For the MVP, we use sRGB exclusively.** Here's the rationale:

- **sRGB** is the standard color space for displays, web, and consumer photography. JPEG/PNG files are sRGB by default. Most monitors display sRGB.
- **Adobe RGB** is a wider-gamut space for professional print workflows (more greens/cyans). Not needed for MVP.
- **ProPhoto RGB** is even wider, used internally by Lightroom. Overkill for now.
- **Display P3** is Apple's wide-gamut display standard. Future consideration.
- **Raw files** have no inherent color space — they're sensor data. Color space is applied during demosaicing. When we add LibRaw integration, the raw decoder will output to sRGB.

**What this means for the implementation:**
- Decoded standard images (JPEG/PNG/TIFF) are assumed to be sRGB.
- We convert sRGB → linear sRGB (using palette crate's `Srgb` → `LinSrgb`) for internal processing.
- Exposure and white balance operate in linear sRGB space.
- Contrast, highlights, shadows, whites, blacks operate in sRGB gamma space.
- Output is encoded back to sRGB gamma for saving.
- No ICC profile handling in MVP — we assume sRGB throughout.

**Future:** Support for wider gamuts (Adobe RGB, ProPhoto RGB, Display P3), ICC profile reading/embedding, and color space conversion. This should be tracked in `docs/ideas/future-features.md`.

---

## Critical Files

| File | Purpose |
|------|---------|
| `crates/oxiraw/src/error.rs` | Create: error types |
| `crates/oxiraw/src/lib.rs` | Modify: add error module + re-exports |
| `crates/oxiraw/src/decode/mod.rs` | Modify: image decoding with sRGB→linear conversion |
| `crates/oxiraw/src/encode/mod.rs` | Modify: linear→sRGB conversion and file output |
| `crates/oxiraw/src/engine/mod.rs` | Modify: Engine struct, Parameters, render pipeline |
| `crates/oxiraw/src/adjust/mod.rs` | Modify: all adjustment algorithms |
| `crates/oxiraw/src/preset/mod.rs` | Modify: TOML preset serialization |
| `crates/oxiraw-cli/src/main.rs` | Modify: CLI with clap subcommands |

---

## Phase 1: Core Pipeline (prove end-to-end with exposure)

### Task 1.1: Error types

**Files:**
- Create: `crates/oxiraw/src/error.rs`
- Modify: `crates/oxiraw/src/lib.rs` (add `pub mod error;`)

**Step 1: Write failing tests**

```rust
// crates/oxiraw/src/error.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display_decode() {
        let err = OxirawError::Decode("bad file".into());
        assert_eq!(err.to_string(), "Decode error: bad file");
    }

    #[test]
    fn error_display_encode() {
        let err = OxirawError::Encode("write failed".into());
        assert_eq!(err.to_string(), "Encode error: write failed");
    }

    #[test]
    fn error_display_preset() {
        let err = OxirawError::Preset("parse failed".into());
        assert_eq!(err.to_string(), "Preset error: parse failed");
    }

    #[test]
    fn error_from_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "missing");
        let err: OxirawError = io_err.into();
        assert!(matches!(err, OxirawError::Io(_)));
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p oxiraw error::tests`
Expected: FAIL (module doesn't exist yet)

**Step 3: Write implementation**

```rust
// crates/oxiraw/src/error.rs
use thiserror::Error;

#[derive(Debug, Error)]
pub enum OxirawError {
    #[error("Decode error: {0}")]
    Decode(String),
    #[error("Encode error: {0}")]
    Encode(String),
    #[error("Preset error: {0}")]
    Preset(String),
    #[error("Image error: {0}")]
    Image(#[from] image::ImageError),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, OxirawError>;
```

Update `lib.rs` to add `pub mod error;`

**Step 4: Run tests** → `cargo test -p oxiraw error::tests` → PASS

**Step 5: Stage** → `git add` changed files

---

### Task 1.2: Decode module (standard formats → linear f32)

**Files:**
- Modify: `crates/oxiraw/src/decode/mod.rs`

**Step 1: Write failing tests**

```rust
// crates/oxiraw/src/decode/mod.rs
#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgb};

    #[test]
    fn decode_png_to_linear_f32() {
        let temp_path = std::env::temp_dir().join("oxiraw_test_decode.png");
        let img: ImageBuffer<Rgb<u8>, Vec<u8>> =
            ImageBuffer::from_pixel(2, 2, Rgb([128, 128, 128]));
        img.save(&temp_path).unwrap();

        let result = decode_standard(&temp_path).unwrap();
        assert_eq!(result.width(), 2);
        assert_eq!(result.height(), 2);

        // sRGB 128/255 ≈ 0.502 → linear ≈ 0.2159
        let pixel = result.get_pixel(0, 0);
        assert!((pixel.0[0] - 0.2159).abs() < 0.01,
            "Expected ~0.2159, got {}", pixel.0[0]);

        let _ = std::fs::remove_file(&temp_path);
    }

    #[test]
    fn decode_nonexistent_file_returns_error() {
        let result = decode_standard(std::path::Path::new("/nonexistent/file.png"));
        assert!(result.is_err());
    }
}
```

**Step 2: Run test** → FAIL

**Step 3: Implementation**

Uses `image::ImageReader::open()` → `decode()` → `into_rgb32f()` → pixel-by-pixel `Srgb::into_linear()` via palette crate.

```rust
use image::{Rgb, Rgb32FImage};
use palette::{LinSrgb, Srgb};
use crate::error::{OxirawError, Result};

pub fn decode_standard(path: &std::path::Path) -> Result<Rgb32FImage> {
    let img = image::ImageReader::open(path)
        .map_err(OxirawError::Io)?
        .decode()
        .map_err(OxirawError::Image)?;
    let srgb_f32 = img.into_rgb32f();
    let (w, h) = srgb_f32.dimensions();
    let linear = Rgb32FImage::from_fn(w, h, |x, y| {
        let p = srgb_f32.get_pixel(x, y);
        let lin: LinSrgb<f32> = Srgb::new(p.0[0], p.0[1], p.0[2]).into_linear();
        Rgb([lin.red, lin.green, lin.blue])
    });
    Ok(linear)
}
```

**Step 4:** `cargo test -p oxiraw decode::tests` → PASS

**Step 5: Stage** → `git add` changed files

---

### Task 1.3: Encode module (linear f32 → sRGB output)

**Files:**
- Modify: `crates/oxiraw/src/encode/mod.rs`

**Step 1: Write failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgb};

    #[test]
    fn roundtrip_linear_to_srgb_pixel_values() {
        // linear 0.2159 should round-trip to sRGB ~128
        let linear: Rgb32FImage = ImageBuffer::from_pixel(1, 1, Rgb([0.2159f32, 0.2159, 0.2159]));
        let dynamic = linear_to_srgb_dynamic(&linear);
        let rgb8 = dynamic.to_rgb8();
        let pixel = rgb8.get_pixel(0, 0);
        assert!((pixel.0[0] as i32 - 128).unsigned_abs() <= 1);
    }

    #[test]
    fn encode_saves_file() {
        let temp_path = std::env::temp_dir().join("oxiraw_test_encode.png");
        let linear: Rgb32FImage = ImageBuffer::from_pixel(2, 2, Rgb([0.5f32, 0.5, 0.5]));
        encode_to_file(&linear, &temp_path).unwrap();
        assert!(temp_path.exists());
        let _ = std::fs::remove_file(&temp_path);
    }
}
```

**Step 2:** FAIL → **Step 3: Implement** `linear_to_srgb_dynamic()` and `encode_to_file()` using palette's `LinSrgb::into_encoding()` → **Step 4:** PASS

**Step 5: Stage** → `git add` changed files

---

### Task 1.4: Adjust module — exposure + color space helpers + stubs

**Files:**
- Modify: `crates/oxiraw/src/adjust/mod.rs`

This implements exposure (the first real adjustment), sRGB↔linear helpers, and stubs for all other adjustments so the engine compiles.

**Step 1: Write failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposure_factor_zero_is_one() {
        assert_eq!(exposure_factor(0.0), 1.0);
    }

    #[test]
    fn exposure_factor_one_stop_doubles() {
        assert!((exposure_factor(1.0) - 2.0).abs() < 1e-6);
    }

    #[test]
    fn exposure_factor_neg_one_halves() {
        assert!((exposure_factor(-1.0) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn apply_exposure_multiplies() {
        assert!((apply_exposure(0.25, exposure_factor(1.0)) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn linear_srgb_roundtrip() {
        let (sr, sg, sb) = linear_to_srgb(0.5, 0.3, 0.1);
        let (lr, lg, lb) = srgb_to_linear(sr, sg, sb);
        assert!((lr - 0.5).abs() < 1e-5);
        assert!((lg - 0.3).abs() < 1e-5);
        assert!((lb - 0.1).abs() < 1e-5);
    }
}
```

**Step 3: Implementation**

- `linear_to_srgb(r, g, b)` and `srgb_to_linear(r, g, b)` using palette crate
- `exposure_factor(stops) -> f32` = `2.0f32.powf(stops)`
- `apply_exposure(value, factor) -> f32` = `(value * factor).max(0.0)`
- Stubs for: `white_balance`, `apply_contrast`, `apply_highlights`, `apply_shadows`, `apply_whites`, `apply_blacks` (all return input unchanged)

**Step 5: Stage** → `git add` changed files

---

### Task 1.5: Engine skeleton — Parameters + render pipeline

**Files:**
- Modify: `crates/oxiraw/src/engine/mod.rs`

**Step 1: Write failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgb};

    fn make_test_image(r: f32, g: f32, b: f32) -> Rgb32FImage {
        ImageBuffer::from_pixel(2, 2, Rgb([r, g, b]))
    }

    #[test]
    fn parameters_default_is_neutral() {
        let p = Parameters::default();
        assert_eq!(p.exposure, 0.0);
        assert_eq!(p.contrast, 0.0);
        assert_eq!(p.highlights, 0.0);
        assert_eq!(p.shadows, 0.0);
        assert_eq!(p.whites, 0.0);
        assert_eq!(p.blacks, 0.0);
        assert_eq!(p.temperature, 0.0);
        assert_eq!(p.tint, 0.0);
    }

    #[test]
    fn render_neutral_params_is_identity() {
        let img = make_test_image(0.5, 0.3, 0.1);
        let engine = Engine::new(img);
        let rendered = engine.render();
        let orig = engine.original().get_pixel(0, 0);
        let rend = rendered.get_pixel(0, 0);
        for i in 0..3 {
            assert!((orig.0[i] - rend.0[i]).abs() < 1e-6);
        }
    }

    #[test]
    fn render_exposure_plus_one_doubles() {
        let img = make_test_image(0.25, 0.25, 0.25);
        let mut engine = Engine::new(img);
        engine.params_mut().exposure = 1.0;
        let pixel = engine.render().get_pixel(0, 0).clone();
        for i in 0..3 {
            assert!((pixel.0[i] - 0.5).abs() < 1e-6);
        }
    }
}
```

**Step 3: Implementation**

```rust
use image::{Rgb, Rgb32FImage};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Parameters {
    pub exposure: f32,      // stops, -5.0 to +5.0
    pub contrast: f32,      // -100 to +100
    pub highlights: f32,    // -100 to +100
    pub shadows: f32,       // -100 to +100
    pub whites: f32,        // -100 to +100
    pub blacks: f32,        // -100 to +100
    pub temperature: f32,   // WB temp shift
    pub tint: f32,          // WB tint shift
}

impl Default for Parameters {
    fn default() -> Self {
        Self { exposure: 0.0, contrast: 0.0, highlights: 0.0, shadows: 0.0,
               whites: 0.0, blacks: 0.0, temperature: 0.0, tint: 0.0 }
    }
}

pub struct Engine {
    original: Rgb32FImage,
    params: Parameters,
}
```

The `render()` method applies adjustments in fixed order:
1. White balance (linear space) — channel multipliers
2. Exposure (linear space) — multiply by 2^stops
3. Convert to sRGB gamma space
4. Contrast, highlights, shadows, whites, blacks (sRGB space)
5. Convert back to linear space

**Step 5: Stage** → `git add` changed files

---

### Task 1.6: End-to-end pipeline test

**Files:**
- Modify: `crates/oxiraw/src/engine/mod.rs` (add test)

**Test:** Full decode → engine → exposure → render → encode roundtrip.

```rust
#[test]
fn full_pipeline_decode_engine_encode() {
    let temp_dir = std::env::temp_dir();
    let input = temp_dir.join("oxiraw_e2e_in.png");
    let output = temp_dir.join("oxiraw_e2e_out.png");

    // Create sRGB 128,128,128 test image
    let img: image::ImageBuffer<Rgb<u8>, Vec<u8>> =
        image::ImageBuffer::from_pixel(4, 4, Rgb([128u8, 128, 128]));
    img.save(&input).unwrap();

    // Decode → Engine +1 stop → Render → Encode
    let linear = crate::decode::decode_standard(&input).unwrap();
    let mut engine = Engine::new(linear);
    engine.params_mut().exposure = 1.0;
    let rendered = engine.render();
    crate::encode::encode_to_file(&rendered, &output).unwrap();

    // Verify output is brighter (sRGB 128 → linear ~0.216 → *2 → ~0.432 → sRGB ~173)
    let out_img = image::ImageReader::open(&output).unwrap().decode().unwrap().to_rgb8();
    let pixel = out_img.get_pixel(0, 0);
    assert!(pixel.0[0] > 150 && pixel.0[0] < 190,
        "Expected ~173, got {}", pixel.0[0]);

    let _ = std::fs::remove_file(&input);
    let _ = std::fs::remove_file(&output);
}
```

This is the KEY proof-of-concept: proves the entire pipeline works end-to-end.

**Step 5: Stage** → `git add` changed files

---

### Task 1.7: Update lib.rs with re-exports

Add to `lib.rs`: `pub use engine::{Engine, Parameters};` and `pub use error::{OxirawError, Result};`

Run: `cargo test -p oxiraw` → all Phase 1 tests pass (~15+ tests)

**Stage** → `feat: add public re-exports from lib.rs`

---

## Phase 2: Remaining Adjustments

Each task: add tests to `adjust/mod.rs`, replace the stub with real implementation, add engine integration test.

---

### Task 2.1: Contrast

**Formula (sRGB space):** `(0.5 + (value - 0.5) * factor).clamp(0.0, 1.0)` where `factor = (100 + contrast) / 100`

**Tests:** zero=identity, positive increases deviation from midpoint, negative decreases, output clamped

**Stage** → `feat: implement contrast adjustment`

---

### Task 2.2: Highlights

**Formula (sRGB space):** Targets pixels > 0.5. Weight ramps linearly 0→1 over 0.5→1.0. Adjustment = `weight * (highlights/100) * 0.5`

**Tests:** zero=identity, dark pixels unaffected, negative darkens bright, positive brightens bright, brighter pixels affected more

**Stage** → `feat: implement highlights adjustment`

---

### Task 2.3: Shadows

**Formula (sRGB space):** Targets pixels < 0.5. Weight ramps 1→0 over 0.0→0.5. Adjustment = `weight * (shadows/100) * 0.5`

**Tests:** zero=identity, bright pixels unaffected, positive lifts darks, negative crushes darks, darker pixels affected more

**Stage** → `feat: implement shadows adjustment`

---

### Task 2.4: Whites

**Formula (sRGB space):** Targets pixels > 0.75. Weight ramps 0→1 over 0.75→1.0. Adjustment = `weight * (whites/100) * 0.25`

**Tests:** zero=identity, dark pixels unaffected, positive brightens upper range, negative darkens upper range

**Stage** → `feat: implement whites adjustment`

---

### Task 2.5: Blacks

**Formula (sRGB space):** Targets pixels < 0.25. Weight ramps 1→0 over 0.0→0.25. Adjustment = `weight * (blacks/100) * 0.25`

**Tests:** zero=identity, bright pixels unaffected, positive lifts blacks, negative crushes blacks

**Stage** → `feat: implement blacks adjustment`

---

### Task 2.6: White Balance

**Formula (linear space):** Channel multipliers from temperature + tint, normalized to preserve brightness.
- temp > 0 = warm (boost red, reduce blue)
- tint > 0 = magenta (reduce green)
- Normalize: `sum = r_mult + g_mult + b_mult; norm = 3.0 / sum`

**Tests:** zero=identity, warm boosts red/reduces blue, cool opposite, tint positive reduces green, tint negative boosts green, output non-negative

**Stage** → `feat: implement white balance with channel multipliers`

---

### Task 2.7: Engine integration tests for combined adjustments

Add tests to `engine/mod.rs` verifying that combined parameters (exposure + contrast, white balance warm shift, all-neutral identity) produce expected results.

**Stage** → `test: add engine integration tests for combined adjustments`

---

## Phase 3: Presets

### Task 3.1: Preset struct + TOML serialization

**Files:**
- Modify: `crates/oxiraw/src/preset/mod.rs`

**Implementation:**
- `PresetMetadata` struct (name, version, author)
- Internal `PresetRaw` / `ToneSection` / `WhiteBalanceSection` for TOML layout
- Public `Preset` struct with `from_toml()`, `to_toml()` methods
- Missing fields default to neutral (via `#[serde(default)]`)

> **Note:** The TOML section structure (`[tone]`, `[white_balance]`) is an initial design. It may change as we explore composable presets, partial presets, or alternative schemas.

**Tests:** default neutral, serialize contains expected keys, deserialize parses values, roundtrip, missing fields default to zero, invalid TOML returns error

**Stage** → `feat: add preset TOML serialization`

---

### Task 3.2: Preset file I/O

Add `save_to_file()` and `load_from_file()` to Preset.

**Tests:** save + load roundtrip, nonexistent file returns error

**Stage** → `feat: add preset file I/O`

---

### Task 3.3: Engine::apply_preset

Add `apply_preset(&preset)` method that replaces engine params with preset params.

**Stage** → `feat: add Engine::apply_preset`

---

### Task 3.4: Update lib.rs

Add `pub use preset::Preset;`

**Stage** → `feat: add Preset re-export`

---

## Phase 4: CLI

### Task 4.1: CLI with clap derive

**Files:**
- Modify: `crates/oxiraw-cli/src/main.rs`
- Modify: `crates/oxiraw-cli/Cargo.toml` (add `[dev-dependencies] image = "0.25"`)

**Subcommands:**
- `apply` — `--input`, `--preset`, `--output`
- `edit` — `--input`, `--output`, `--exposure`, `--contrast`, `--highlights`, `--shadows`, `--whites`, `--blacks`, `--temperature`, `--tint`

> **Note:** CLI structure is initial and will evolve. Flag names and subcommands may change.

**Verify:** `cargo run -p oxiraw-cli -- --help` shows usage

**Stage** → `feat: add CLI with apply and edit subcommands`

---

### Task 4.2: CLI integration tests

**Files:**
- Create: `crates/oxiraw-cli/tests/integration.rs`

**Tests:**
- `cli_apply_produces_output_file` — create test PNG + preset, run CLI apply, verify output exists and is brighter
- `cli_edit_with_inline_params` — run CLI edit with --exposure -1.0, verify output is darker
- `cli_missing_input_fails` — nonexistent input returns error exit code

**Stage** → `test: add CLI integration tests`

---

### Task 4.3: Final verification

Run: `cargo test --workspace`

All ~59 tests across both crates should pass.

**Stage** → (no changes, just verify)

---

## Verification

After all tasks are complete:

1. **Unit tests:** `cargo test --workspace` — all pass
2. **Manual CLI test:**
   ```bash
   # Create a test preset
   cat > /tmp/test-preset.toml << 'EOF'
   [metadata]
   name = "Test"
   [tone]
   exposure = 1.5
   contrast = 20.0
   [white_balance]
   temperature = 30.0
   EOF

   # Apply it to any JPEG/PNG
   cargo run -p oxiraw-cli -- apply --input photo.jpg --preset /tmp/test-preset.toml --output edited.jpg

   # Or edit inline
   cargo run -p oxiraw-cli -- edit --input photo.jpg --output edited.jpg --exposure 1.0 --contrast 25
   ```
3. **Verify output:** Open the edited image and confirm visible brightness/contrast/color changes
