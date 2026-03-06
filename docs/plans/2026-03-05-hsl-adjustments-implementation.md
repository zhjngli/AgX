# HSL Adjustments Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add per-channel HSL (Hue, Saturation, Luminance) adjustments across the full stack: data model, algorithm, engine, presets, and CLI.

**Architecture:** 8-channel HSL with nested `HslChannels` struct on `Parameters`. The adjust module gets a stateless `apply_hsl` function that takes arrays (not engine types, per architecture rules). Cosine falloff weight function is pluggable via function pointer. Pipeline position: after tone adjustments, before LUT (sRGB gamma space).

**Tech Stack:** palette 0.7 (`Hsl`, `IntoColor`), serde, clap 4

---

## Context

Design doc: `docs/plans/2026-03-05-hsl-adjustments-design.md`

Key architecture constraint: the `adjust` module MUST NOT import from `engine`. This means `apply_hsl` takes `&[f32; 8]` arrays for hue/saturation/luminance shifts, not the `HslChannels` struct. The engine extracts arrays from `HslChannels` before calling.

## Critical Files

| File | Purpose |
|------|---------|
| `crates/oxiraw/src/engine/mod.rs` | Modify: add HslChannel, HslChannels, update Parameters, wire into render() |
| `crates/oxiraw/src/adjust/mod.rs` | Modify: add hue_distance, cosine_weight, apply_hsl |
| `crates/oxiraw/src/preset/mod.rs` | Modify: add hsl field to PresetRaw, map to/from Parameters |
| `crates/oxiraw/src/lib.rs` | Modify: add HslChannel, HslChannels re-exports |
| `crates/oxiraw-cli/src/main.rs` | Modify: add 24 HSL CLI flags with short aliases |
| `crates/oxiraw/src/adjust/README.md` | Modify: document new HSL functions |

---

## Phase 1: Data Model

### Task 1.1: HslChannel, HslChannels, and Parameters update

**Files:**
- Modify: `crates/oxiraw/src/engine/mod.rs`

**Step 1: Write failing tests**

Add to the existing `tests` module in `engine/mod.rs`:

```rust
#[test]
fn hsl_channel_default_is_zero() {
    let ch = super::HslChannel::default();
    assert_eq!(ch.hue, 0.0);
    assert_eq!(ch.saturation, 0.0);
    assert_eq!(ch.luminance, 0.0);
}

#[test]
fn hsl_channels_default_all_zero() {
    let hsl = super::HslChannels::default();
    assert_eq!(hsl.red, super::HslChannel::default());
    assert_eq!(hsl.green, super::HslChannel::default());
    assert_eq!(hsl.magenta, super::HslChannel::default());
}

#[test]
fn hsl_channels_is_default_true_when_default() {
    let hsl = super::HslChannels::default();
    assert!(hsl.is_default());
}

#[test]
fn hsl_channels_is_default_false_when_modified() {
    let mut hsl = super::HslChannels::default();
    hsl.red.hue = 10.0;
    assert!(!hsl.is_default());
}

#[test]
fn hsl_channels_extracts_shift_arrays() {
    let mut hsl = super::HslChannels::default();
    hsl.red.hue = 15.0;
    hsl.green.saturation = -30.0;
    hsl.blue.luminance = 20.0;
    let h = hsl.hue_shifts();
    let s = hsl.saturation_shifts();
    let l = hsl.luminance_shifts();
    assert_eq!(h[0], 15.0); // red
    assert_eq!(s[3], -30.0); // green
    assert_eq!(l[5], 20.0); // blue
}

#[test]
fn parameters_default_hsl_is_default() {
    let p = Parameters::default();
    assert!(p.hsl.is_default());
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p oxiraw engine::tests`
Expected: FAIL (HslChannel doesn't exist yet)

**Step 3: Write implementation**

Add before the `Parameters` struct in `engine/mod.rs`:

```rust
/// Per-channel HSL adjustment (hue shift, saturation, luminance).
///
/// Ranges: hue -180.0 to +180.0 (degrees), saturation/luminance -100.0 to +100.0.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct HslChannel {
    #[serde(default)]
    pub hue: f32,
    #[serde(default)]
    pub saturation: f32,
    #[serde(default)]
    pub luminance: f32,
}

/// HSL adjustments for all 8 color channels.
///
/// Channel order: Red (0°), Orange (30°), Yellow (60°), Green (120°),
/// Aqua (180°), Blue (240°), Purple (270°), Magenta (330°).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct HslChannels {
    #[serde(default)]
    pub red: HslChannel,
    #[serde(default)]
    pub orange: HslChannel,
    #[serde(default)]
    pub yellow: HslChannel,
    #[serde(default)]
    pub green: HslChannel,
    #[serde(default)]
    pub aqua: HslChannel,
    #[serde(default)]
    pub blue: HslChannel,
    #[serde(default)]
    pub purple: HslChannel,
    #[serde(default)]
    pub magenta: HslChannel,
}

impl HslChannels {
    /// Returns true if all channels are at default (zero) values.
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// Extract hue shifts as an array ordered by channel index.
    pub fn hue_shifts(&self) -> [f32; 8] {
        [
            self.red.hue, self.orange.hue, self.yellow.hue, self.green.hue,
            self.aqua.hue, self.blue.hue, self.purple.hue, self.magenta.hue,
        ]
    }

    /// Extract saturation shifts as an array ordered by channel index.
    pub fn saturation_shifts(&self) -> [f32; 8] {
        [
            self.red.saturation, self.orange.saturation, self.yellow.saturation,
            self.green.saturation, self.aqua.saturation, self.blue.saturation,
            self.purple.saturation, self.magenta.saturation,
        ]
    }

    /// Extract luminance shifts as an array ordered by channel index.
    pub fn luminance_shifts(&self) -> [f32; 8] {
        [
            self.red.luminance, self.orange.luminance, self.yellow.luminance,
            self.green.luminance, self.aqua.luminance, self.blue.luminance,
            self.purple.luminance, self.magenta.luminance,
        ]
    }
}
```

Add `hsl` field to `Parameters`:

```rust
pub struct Parameters {
    // ... existing 8 fields ...
    /// Per-channel HSL adjustments
    #[serde(default)]
    pub hsl: HslChannels,
}
```

Update `Parameters::default()` to include `hsl: HslChannels::default()`.

**Step 4: Run tests**

Run: `cargo test -p oxiraw engine::tests`
Expected: PASS

**Step 5: Stage**

```bash
git add crates/oxiraw/src/engine/mod.rs
git commit -m "feat: add HslChannel and HslChannels data types to Parameters"
```

---

## Phase 2: HSL Algorithm

### Task 2.1: hue_distance and cosine_weight helpers

**Files:**
- Modify: `crates/oxiraw/src/adjust/mod.rs`

**Step 1: Write failing tests**

Add to the existing `tests` module in `adjust/mod.rs`:

```rust
// --- HSL helpers ---

#[test]
fn hue_distance_same_is_zero() {
    assert_eq!(hue_distance(120.0, 120.0), 0.0);
}

#[test]
fn hue_distance_opposite_is_180() {
    assert!((hue_distance(0.0, 180.0) - 180.0).abs() < 1e-6);
}

#[test]
fn hue_distance_wraps_around() {
    assert!((hue_distance(350.0, 10.0) - 20.0).abs() < 1e-6);
    assert!((hue_distance(10.0, 350.0) - 20.0).abs() < 1e-6);
}

#[test]
fn hue_distance_is_symmetric() {
    assert!((hue_distance(30.0, 90.0) - hue_distance(90.0, 30.0)).abs() < 1e-6);
}

#[test]
fn cosine_weight_at_center_is_one() {
    assert!((cosine_weight(0.0, 30.0) - 1.0).abs() < 1e-6);
}

#[test]
fn cosine_weight_at_half_width_is_zero() {
    assert!(cosine_weight(30.0, 30.0).abs() < 1e-6);
}

#[test]
fn cosine_weight_beyond_half_width_is_zero() {
    assert_eq!(cosine_weight(45.0, 30.0), 0.0);
}

#[test]
fn cosine_weight_at_half_distance_is_between_zero_and_one() {
    let w = cosine_weight(15.0, 30.0);
    assert!(w > 0.0 && w < 1.0, "Expected 0 < {} < 1", w);
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p oxiraw adjust::tests::hue_distance`
Expected: FAIL

**Step 3: Write implementation**

Add to `adjust/mod.rs` (after the existing blacks function, before `#[cfg(test)]`):

```rust
// --- HSL helpers ---

/// Type alias for HSL weight functions. Takes (hue_distance, half_width) in degrees,
/// returns a 0.0–1.0 weight.
pub type WeightFn = fn(f32, f32) -> f32;

/// Compute the shortest angular distance between two hue angles in degrees.
/// Result is always in [0, 180].
pub fn hue_distance(a: f32, b: f32) -> f32 {
    let d = (a - b).rem_euclid(360.0);
    if d > 180.0 { 360.0 - d } else { d }
}

/// Cosine falloff: smooth bell curve, 1.0 at center, 0.0 at half_width.
/// hue_distance and half_width are in degrees.
pub fn cosine_weight(hue_dist: f32, half_width: f32) -> f32 {
    if hue_dist >= half_width {
        0.0
    } else {
        ((hue_dist / half_width) * std::f32::consts::PI).cos() * 0.5 + 0.5
    }
}
```

**Step 4: Run tests**

Run: `cargo test -p oxiraw adjust::tests`
Expected: PASS

**Step 5: Stage**

```bash
git add crates/oxiraw/src/adjust/mod.rs
git commit -m "feat: add hue_distance and cosine_weight HSL helpers"
```

---

### Task 2.2: apply_hsl core function

**Files:**
- Modify: `crates/oxiraw/src/adjust/mod.rs`

**Step 1: Write failing tests**

Add to `tests` module:

```rust
// --- apply_hsl tests ---

#[test]
fn apply_hsl_all_zeros_is_identity() {
    let zeros = [0.0f32; 8];
    let (r, g, b) = apply_hsl(1.0, 0.0, 0.0, &zeros, &zeros, &zeros, cosine_weight);
    assert!((r - 1.0).abs() < 1e-4, "r: expected ~1.0, got {r}");
    assert!(g.abs() < 1e-4, "g: expected ~0.0, got {g}");
    assert!(b.abs() < 1e-4, "b: expected ~0.0, got {b}");
}

#[test]
fn apply_hsl_red_hue_shift_rotates_red() {
    // Pure red (hue 0°), shift hue +120° → should become green-ish
    let mut hue = [0.0f32; 8];
    hue[0] = 120.0; // red channel hue shift
    let zeros = [0.0f32; 8];
    let (r, g, b) = apply_hsl(1.0, 0.0, 0.0, &hue, &zeros, &zeros, cosine_weight);
    assert!(g > r, "Expected green > red after +120° hue shift, got r={r} g={g}");
}

#[test]
fn apply_hsl_red_saturation_decrease_desaturates() {
    let zeros = [0.0f32; 8];
    let mut sat = [0.0f32; 8];
    sat[0] = -100.0; // red channel full desaturate
    let (r, g, b) = apply_hsl(1.0, 0.0, 0.0, &zeros, &sat, &zeros, cosine_weight);
    // Desaturated red → gray-ish, channels should be closer together
    assert!((r - g).abs() < (1.0 - 0.0), "Expected channels closer together");
    assert!((r - b).abs() < (1.0 - 0.0), "Expected channels closer together");
}

#[test]
fn apply_hsl_green_shift_does_not_affect_red() {
    // Pure red pixel, only green channel has a shift → red should be unaffected
    let zeros = [0.0f32; 8];
    let mut sat = [0.0f32; 8];
    sat[3] = -100.0; // green channel (index 3)
    let (r, g, b) = apply_hsl(1.0, 0.0, 0.0, &zeros, &sat, &zeros, cosine_weight);
    assert!((r - 1.0).abs() < 1e-3, "Red pixel should be unaffected by green channel");
    assert!(g.abs() < 1e-3);
    assert!(b.abs() < 1e-3);
}

#[test]
fn apply_hsl_gray_pixel_unaffected() {
    // Gray pixel (saturation ≈ 0) should not be affected by HSL
    let mut hue = [0.0f32; 8];
    let mut sat = [0.0f32; 8];
    let mut lum = [0.0f32; 8];
    hue[0] = 90.0;
    sat[0] = 50.0;
    lum[0] = 50.0;
    let (r, g, b) = apply_hsl(0.5, 0.5, 0.5, &hue, &sat, &lum, cosine_weight);
    assert!((r - 0.5).abs() < 1e-3, "Gray should be unaffected, got r={r}");
    assert!((g - 0.5).abs() < 1e-3, "Gray should be unaffected, got g={g}");
    assert!((b - 0.5).abs() < 1e-3, "Gray should be unaffected, got b={b}");
}

#[test]
fn apply_hsl_luminance_brightens() {
    let zeros = [0.0f32; 8];
    let mut lum = [0.0f32; 8];
    lum[0] = 50.0; // brighten reds
    let (r, g, b) = apply_hsl(1.0, 0.0, 0.0, &zeros, &zeros, &lum, cosine_weight);
    // Pure red lightness increases → r should increase (or at least rgb sum increases)
    let orig_sum: f32 = 1.0 + 0.0 + 0.0;
    let new_sum = r + g + b;
    assert!(new_sum > orig_sum, "Expected brighter, got sum={new_sum} vs {orig_sum}");
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p oxiraw adjust::tests::apply_hsl`
Expected: FAIL

**Step 3: Write implementation**

Add to `adjust/mod.rs`, after `cosine_weight`:

```rust
use palette::{Hsl, IntoColor};

/// Channel center hues in degrees.
/// Order: Red, Orange, Yellow, Green, Aqua, Blue, Purple, Magenta.
const CHANNEL_CENTERS: [f32; 8] = [0.0, 30.0, 60.0, 120.0, 180.0, 240.0, 270.0, 330.0];

/// Half-width of each channel's influence zone in degrees.
/// Derived from distance to nearest neighbor.
const CHANNEL_HALF_WIDTHS: [f32; 8] = [30.0, 30.0, 30.0, 60.0, 60.0, 30.0, 30.0, 30.0];

/// Apply per-channel HSL adjustments to an sRGB gamma pixel.
///
/// Takes 3 arrays of 8 values each (one per channel, ordered Red through Magenta):
/// - `hue_shifts`: degrees, -180 to +180
/// - `saturation_shifts`: -100 to +100
/// - `luminance_shifts`: -100 to +100
///
/// The `weight_fn(hue_distance, half_width) -> weight` is pluggable.
pub fn apply_hsl(
    r: f32, g: f32, b: f32,
    hue_shifts: &[f32; 8],
    saturation_shifts: &[f32; 8],
    luminance_shifts: &[f32; 8],
    weight_fn: WeightFn,
) -> (f32, f32, f32) {
    let srgb = Srgb::new(r, g, b);
    let hsl: Hsl = srgb.into_color();
    let pixel_hue = hsl.hue.into_positive_degrees();
    let pixel_sat = hsl.saturation;

    // Gray/near-gray pixels: hue is undefined, skip HSL adjustments
    if pixel_sat < 1e-4 {
        return (r, g, b);
    }

    let mut total_hue_shift = 0.0f32;
    let mut total_sat_shift = 0.0f32;
    let mut total_lum_shift = 0.0f32;

    for i in 0..8 {
        let dist = hue_distance(pixel_hue, CHANNEL_CENTERS[i]);
        // Scale weight by pixel saturation to fade effect for low-saturation pixels
        let weight = weight_fn(dist, CHANNEL_HALF_WIDTHS[i]) * pixel_sat;
        if weight > 0.0 {
            total_hue_shift += weight * hue_shifts[i];
            total_sat_shift += weight * (saturation_shifts[i] / 100.0);
            total_lum_shift += weight * (luminance_shifts[i] / 100.0);
        }
    }

    let new_hue = (pixel_hue + total_hue_shift).rem_euclid(360.0);
    let new_sat = (hsl.saturation + total_sat_shift).clamp(0.0, 1.0);
    let new_lum = (hsl.lightness + total_lum_shift).clamp(0.0, 1.0);

    let new_hsl = Hsl::new(new_hue, new_sat, new_lum);
    let rgb: Srgb<f32> = new_hsl.into_color();
    (rgb.red, rgb.green, rgb.blue)
}
```

Update the import at the top of `adjust/mod.rs`:

```rust
use palette::{Hsl, IntoColor, LinSrgb, Srgb};
```

**Step 4: Run tests**

Run: `cargo test -p oxiraw adjust::tests`
Expected: PASS

**Step 5: Stage**

```bash
git add crates/oxiraw/src/adjust/mod.rs
git commit -m "feat: add apply_hsl with pluggable weight function"
```

---

## Phase 3: Engine Integration

### Task 3.1: Wire apply_hsl into render()

**Files:**
- Modify: `crates/oxiraw/src/engine/mod.rs`

**Step 1: Write failing tests**

Add to the existing `tests` module in `engine/mod.rs`:

```rust
#[test]
fn render_hsl_neutral_is_identity() {
    // Red-ish pixel in linear space
    let img = make_test_image(0.5, 0.01, 0.01);
    let engine = Engine::new(img);
    // HSL defaults to all zeros, so render should be identity
    let orig = engine.original().get_pixel(0, 0);
    let rend = engine.render().get_pixel(0, 0).clone();
    for i in 0..3 {
        assert!(
            (orig.0[i] - rend.0[i]).abs() < 1e-4,
            "Channel {i}: expected {}, got {}",
            orig.0[i], rend.0[i]
        );
    }
}

#[test]
fn render_hsl_red_saturation_decrease() {
    // Pure-ish red in linear space
    let img = make_test_image(0.5, 0.01, 0.01);
    let mut engine = Engine::new(img);
    engine.params_mut().hsl.red.saturation = -100.0;
    let rendered = engine.render();
    let p = rendered.get_pixel(0, 0);
    // Desaturated: channels should be closer together than original
    let spread = (p.0[0] - p.0[1]).abs() + (p.0[0] - p.0[2]).abs();
    let orig = engine.original().get_pixel(0, 0);
    let orig_spread = (orig.0[0] - orig.0[1]).abs() + (orig.0[0] - orig.0[2]).abs();
    assert!(
        spread < orig_spread,
        "Expected less spread after desaturation: {spread} vs {orig_spread}"
    );
}

#[test]
fn render_hsl_green_shift_does_not_affect_red_image() {
    let img = make_test_image(0.5, 0.01, 0.01);
    let mut engine = Engine::new(img);
    engine.params_mut().hsl.green.saturation = -100.0;
    let rendered = engine.render();
    let orig = engine.original().get_pixel(0, 0);
    let rend = rendered.get_pixel(0, 0);
    for i in 0..3 {
        assert!(
            (orig.0[i] - rend.0[i]).abs() < 1e-3,
            "Channel {i}: red image should be unaffected by green HSL"
        );
    }
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p oxiraw engine::tests::render_hsl`
Expected: FAIL (apply_hsl not called in render yet)

**Step 3: Write implementation**

In `Engine::render()`, add HSL step between blacks (step 8) and LUT (step 9). Add the early-exit check before the per-pixel loop:

```rust
// Before the Rgb32FImage::from_fn closure, compute HSL arrays once:
let hsl_active = !self.params.hsl.is_default();
let hue_shifts = self.params.hsl.hue_shifts();
let sat_shifts = self.params.hsl.saturation_shifts();
let lum_shifts = self.params.hsl.luminance_shifts();

// Inside the per-pixel closure, after blacks and before LUT:

// 9. HSL adjustments (sRGB gamma space)
if hsl_active {
    let (hr, hg, hb) = adjust::apply_hsl(
        sr, sg, sb,
        &hue_shifts, &sat_shifts, &lum_shifts,
        adjust::cosine_weight,
    );
    sr = hr;
    sg = hg;
    sb = hb;
}
```

Update the render() doc comment to include HSL as step 9 and renumber LUT to 10.

**Step 4: Run tests**

Run: `cargo test -p oxiraw engine::tests`
Expected: PASS (all existing + new HSL tests)

**Step 5: Stage**

```bash
git add crates/oxiraw/src/engine/mod.rs
git commit -m "feat: integrate HSL adjustments into render pipeline"
```

---

## Phase 4: Preset Integration

### Task 4.1: Add HSL to preset TOML format

**Files:**
- Modify: `crates/oxiraw/src/preset/mod.rs`

**Step 1: Write failing tests**

Add to the existing `tests` module:

```rust
#[test]
fn preset_hsl_roundtrip() {
    let mut preset = Preset::default();
    preset.params.hsl.red.hue = 15.0;
    preset.params.hsl.green.saturation = -30.0;
    preset.params.hsl.blue.luminance = 20.0;
    let toml_str = preset.to_toml().unwrap();
    let parsed = Preset::from_toml(&toml_str).unwrap();
    assert_eq!(preset.params.hsl, parsed.params.hsl);
}

#[test]
fn preset_missing_hsl_defaults_to_zero() {
    let toml_str = "[metadata]\nname = \"No HSL\"\n\n[tone]\nexposure = 1.0\n";
    let preset = Preset::from_toml(toml_str).unwrap();
    assert!(preset.params.hsl.is_default());
}

#[test]
fn preset_partial_hsl_channels_default() {
    let toml_str = r#"
[metadata]
name = "Partial HSL"

[hsl.red]
hue = 10.0
"#;
    let preset = Preset::from_toml(toml_str).unwrap();
    assert_eq!(preset.params.hsl.red.hue, 10.0);
    assert_eq!(preset.params.hsl.red.saturation, 0.0);
    assert!(preset.params.hsl.green == HslChannel::default());
}
```

Note: the test uses `HslChannel` — add `use crate::engine::HslChannel;` to the test module.

**Step 2: Run tests to verify they fail**

Run: `cargo test -p oxiraw preset::tests::preset_hsl`
Expected: FAIL (HSL not in PresetRaw yet)

**Step 3: Write implementation**

Add `hsl` field to `PresetRaw`. Import `HslChannels` from engine:

```rust
use crate::engine::{HslChannels, Parameters};
```

Add to `PresetRaw`:
```rust
struct PresetRaw {
    // ... existing fields ...
    #[serde(default)]
    hsl: HslChannels,
}
```

Update `Preset::from_toml()` to include `hsl` in the Parameters construction:
```rust
params: Parameters {
    // ... existing fields ...
    hsl: raw.hsl,
},
```

Update `Preset::to_toml()` to include `hsl` in PresetRaw construction:
```rust
let raw = PresetRaw {
    // ... existing fields ...
    hsl: self.params.hsl.clone(),
};
```

Update `Preset::load_from_file()` similarly — set `hsl: raw.hsl` in the Parameters construction.

**Step 4: Run tests**

Run: `cargo test -p oxiraw preset::tests`
Expected: PASS

**Step 5: Stage**

```bash
git add crates/oxiraw/src/preset/mod.rs
git commit -m "feat: add HSL section to preset TOML format"
```

---

## Phase 5: CLI Integration

### Task 5.1: Add HSL flags to CLI

**Files:**
- Modify: `crates/oxiraw-cli/src/main.rs`

**Step 1: Write failing test**

Add to `crates/oxiraw-cli/tests/integration.rs`:

```rust
#[test]
fn cli_edit_with_hsl_flags() {
    let temp_dir = std::env::temp_dir();
    let input = temp_dir.join("oxiraw_cli_hsl_in.png");
    let output = temp_dir.join("oxiraw_cli_hsl_out.png");

    let img: image::ImageBuffer<image::Rgb<u8>, Vec<u8>> =
        image::ImageBuffer::from_pixel(4, 4, image::Rgb([255u8, 0, 0]));
    img.save(&input).unwrap();

    let bin = env!("CARGO_BIN_EXE_oxiraw-cli");
    let status = std::process::Command::new(bin)
        .args([
            "edit",
            "-i", input.to_str().unwrap(),
            "-o", output.to_str().unwrap(),
            "--hsl-red-s", "-100",
        ])
        .status()
        .unwrap();
    assert!(status.success(), "CLI should succeed with HSL flags");
    assert!(output.exists(), "Output file should exist");

    let _ = std::fs::remove_file(&input);
    let _ = std::fs::remove_file(&output);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p oxiraw-cli cli_edit_with_hsl_flags`
Expected: FAIL (unknown flag)

**Step 3: Write implementation**

Add 24 HSL fields to the `Edit` variant in `Commands`. Each uses `visible_alias` for the short form:

```rust
// In Commands::Edit variant, after the `format` field:

/// Red hue shift (-180 to +180 degrees)
#[arg(long = "hsl-red-hue", visible_alias = "hsl-red-h", default_value_t = 0.0, allow_hyphen_values = true)]
hsl_red_hue: f32,
/// Red saturation (-100 to +100)
#[arg(long = "hsl-red-saturation", visible_alias = "hsl-red-s", default_value_t = 0.0, allow_hyphen_values = true)]
hsl_red_saturation: f32,
/// Red luminance (-100 to +100)
#[arg(long = "hsl-red-luminance", visible_alias = "hsl-red-l", default_value_t = 0.0, allow_hyphen_values = true)]
hsl_red_luminance: f32,

// Repeat for: orange, yellow, green, aqua, blue, purple, magenta
// (same pattern, replace "red"/"Red" with the channel name)
```

Build `HslChannels` from the parsed args in `main()` and pass to `run_edit`:

```rust
use oxiraw::engine::{HslChannel, HslChannels};

// In the Commands::Edit match arm:
let hsl = HslChannels {
    red: HslChannel { hue: hsl_red_hue, saturation: hsl_red_saturation, luminance: hsl_red_luminance },
    orange: HslChannel { hue: hsl_orange_hue, saturation: hsl_orange_saturation, luminance: hsl_orange_luminance },
    // ... all 8 channels ...
};
```

Update `run_edit` to accept `hsl: &HslChannels` and set it on the engine:

```rust
params.hsl = hsl.clone();
```

**Step 4: Run tests**

Run: `cargo test -p oxiraw-cli`
Expected: PASS

**Step 5: Stage**

```bash
git add crates/oxiraw-cli/src/main.rs crates/oxiraw-cli/tests/integration.rs
git commit -m "feat: add HSL CLI flags with short aliases"
```

---

## Phase 6: Documentation and Exports

### Task 6.1: Update docs, exports, and ARCHITECTURE.md

**Files:**
- Modify: `crates/oxiraw/src/lib.rs` — add `HslChannel`, `HslChannels` re-exports
- Modify: `crates/oxiraw/src/adjust/README.md` — document new HSL functions
- Modify: `ARCHITECTURE.md` — add implementation plan to design docs table

**Step 1: Update lib.rs**

```rust
pub use engine::{Engine, HslChannel, HslChannels, Parameters};
```

**Step 2: Update adjust/README.md**

Add to the Public API section:

```markdown
- `hue_distance(a, b)` -- shortest angular distance between two hue angles in degrees
- `cosine_weight(hue_dist, half_width)` -- cosine falloff weight function for HSL channel targeting
- `apply_hsl(r, g, b, hue_shifts, saturation_shifts, luminance_shifts, weight_fn)` -- per-channel HSL adjustment in sRGB gamma space
- `WeightFn` -- type alias for pluggable weight functions: `fn(f32, f32) -> f32`
```

**Step 3: Update ARCHITECTURE.md**

Add to the Plans table:

```markdown
| 2026-03-05 | [HSL Adjustments Design](docs/plans/2026-03-05-hsl-adjustments-design.md)                        |
| 2026-03-05 | [HSL Adjustments Implementation](docs/plans/2026-03-05-hsl-adjustments-implementation.md)          |
```

**Step 4: Stage**

```bash
git add crates/oxiraw/src/lib.rs crates/oxiraw/src/adjust/README.md ARCHITECTURE.md
git commit -m "docs: update exports, adjust README, and architecture docs for HSL"
```

---

## Verification

Run the full verification suite:

```bash
./scripts/verify.sh
```

All 5 checks must pass:
1. Format (`cargo fmt`)
2. Clippy (`cargo clippy`)
3. Library tests (`cargo test -p oxiraw`) — existing 95 + ~15 new HSL tests
4. CLI tests (`cargo test -p oxiraw-cli`) — existing 7 + 1 new HSL test
5. Documentation links

If all pass:

```bash
git add -A
git commit -m "chore: final verification pass for HSL adjustments"
```

(Only if there are formatting or clippy fixes needed. Skip if clean.)
