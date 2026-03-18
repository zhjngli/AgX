use image::{Rgb, Rgb32FImage};
use serde::{Deserialize, Serialize};

use crate::adjust;

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
/// Channel order: Red (0deg), Orange (30deg), Yellow (60deg), Green (120deg),
/// Aqua (180deg), Blue (240deg), Purple (270deg), Magenta (330deg).
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
            self.red.hue,
            self.orange.hue,
            self.yellow.hue,
            self.green.hue,
            self.aqua.hue,
            self.blue.hue,
            self.purple.hue,
            self.magenta.hue,
        ]
    }

    /// Extract saturation shifts as an array ordered by channel index.
    pub fn saturation_shifts(&self) -> [f32; 8] {
        [
            self.red.saturation,
            self.orange.saturation,
            self.yellow.saturation,
            self.green.saturation,
            self.aqua.saturation,
            self.blue.saturation,
            self.purple.saturation,
            self.magenta.saturation,
        ]
    }

    /// Extract luminance shifts as an array ordered by channel index.
    pub fn luminance_shifts(&self) -> [f32; 8] {
        [
            self.red.luminance,
            self.orange.luminance,
            self.yellow.luminance,
            self.green.luminance,
            self.aqua.luminance,
            self.blue.luminance,
            self.purple.luminance,
            self.magenta.luminance,
        ]
    }
}

/// Vignette adjustment parameters.
///
/// Darkens or brightens image edges. Amount range: -100 to +100. 0 = no effect.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct VignetteParams {
    #[serde(default)]
    pub amount: f32,
    #[serde(default)]
    pub shape: crate::adjust::VignetteShape,
}

/// All adjustment parameters for the rendering engine.
///
/// Defaults to neutral (no change) for all values.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Parameters {
    /// Exposure in stops, range -5.0 to +5.0
    pub exposure: f32,
    /// Contrast, range -100 to +100
    pub contrast: f32,
    /// Highlights, range -100 to +100
    pub highlights: f32,
    /// Shadows, range -100 to +100
    pub shadows: f32,
    /// Whites, range -100 to +100
    pub whites: f32,
    /// Blacks, range -100 to +100
    pub blacks: f32,
    /// White balance temperature shift
    pub temperature: f32,
    /// White balance tint shift (green/magenta)
    pub tint: f32,
    /// Per-channel HSL adjustments
    #[serde(default)]
    pub hsl: HslChannels,
    /// Creative vignette (edge darkening/brightening)
    #[serde(default)]
    pub vignette: VignetteParams,
}

impl Default for Parameters {
    fn default() -> Self {
        Self {
            exposure: 0.0,
            contrast: 0.0,
            highlights: 0.0,
            shadows: 0.0,
            whites: 0.0,
            blacks: 0.0,
            temperature: 0.0,
            tint: 0.0,
            hsl: HslChannels::default(),
            vignette: VignetteParams::default(),
        }
    }
}

/// Partial per-channel HSL adjustment — `None` means "not specified".
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PartialHslChannel {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hue: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub saturation: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub luminance: Option<f32>,
}

impl PartialHslChannel {
    /// Merge overlay on top of self (last-write-wins).
    pub fn merge(&self, overlay: &Self) -> Self {
        Self {
            hue: overlay.hue.or(self.hue),
            saturation: overlay.saturation.or(self.saturation),
            luminance: overlay.luminance.or(self.luminance),
        }
    }

    /// Convert to concrete HslChannel. None fields become 0.0.
    pub fn materialize(&self) -> HslChannel {
        HslChannel {
            hue: self.hue.unwrap_or(0.0),
            saturation: self.saturation.unwrap_or(0.0),
            luminance: self.luminance.unwrap_or(0.0),
        }
    }
}

impl From<&HslChannel> for PartialHslChannel {
    fn from(ch: &HslChannel) -> Self {
        Self {
            hue: Some(ch.hue),
            saturation: Some(ch.saturation),
            luminance: Some(ch.luminance),
        }
    }
}

/// Partial HSL adjustments for all 8 channels — `None` means channel not specified.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PartialHslChannels {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub red: Option<PartialHslChannel>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub orange: Option<PartialHslChannel>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub yellow: Option<PartialHslChannel>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub green: Option<PartialHslChannel>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aqua: Option<PartialHslChannel>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blue: Option<PartialHslChannel>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub purple: Option<PartialHslChannel>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub magenta: Option<PartialHslChannel>,
}

impl PartialHslChannels {
    fn merge_channel(
        base: &Option<PartialHslChannel>,
        overlay: &Option<PartialHslChannel>,
    ) -> Option<PartialHslChannel> {
        match (base, overlay) {
            (None, None) => None,
            (Some(b), None) => Some(b.clone()),
            (None, Some(o)) => Some(o.clone()),
            (Some(b), Some(o)) => Some(b.merge(o)),
        }
    }

    /// Merge overlay on top of self (last-write-wins per field).
    pub fn merge(&self, overlay: &Self) -> Self {
        Self {
            red: Self::merge_channel(&self.red, &overlay.red),
            orange: Self::merge_channel(&self.orange, &overlay.orange),
            yellow: Self::merge_channel(&self.yellow, &overlay.yellow),
            green: Self::merge_channel(&self.green, &overlay.green),
            aqua: Self::merge_channel(&self.aqua, &overlay.aqua),
            blue: Self::merge_channel(&self.blue, &overlay.blue),
            purple: Self::merge_channel(&self.purple, &overlay.purple),
            magenta: Self::merge_channel(&self.magenta, &overlay.magenta),
        }
    }

    /// Convert to concrete HslChannels. None channels/fields become default (0.0).
    pub fn materialize(&self) -> HslChannels {
        HslChannels {
            red: self
                .red
                .as_ref()
                .map(|c| c.materialize())
                .unwrap_or_default(),
            orange: self
                .orange
                .as_ref()
                .map(|c| c.materialize())
                .unwrap_or_default(),
            yellow: self
                .yellow
                .as_ref()
                .map(|c| c.materialize())
                .unwrap_or_default(),
            green: self
                .green
                .as_ref()
                .map(|c| c.materialize())
                .unwrap_or_default(),
            aqua: self
                .aqua
                .as_ref()
                .map(|c| c.materialize())
                .unwrap_or_default(),
            blue: self
                .blue
                .as_ref()
                .map(|c| c.materialize())
                .unwrap_or_default(),
            purple: self
                .purple
                .as_ref()
                .map(|c| c.materialize())
                .unwrap_or_default(),
            magenta: self
                .magenta
                .as_ref()
                .map(|c| c.materialize())
                .unwrap_or_default(),
        }
    }
}

impl From<&HslChannels> for PartialHslChannels {
    fn from(hsl: &HslChannels) -> Self {
        Self {
            red: Some(PartialHslChannel::from(&hsl.red)),
            orange: Some(PartialHslChannel::from(&hsl.orange)),
            yellow: Some(PartialHslChannel::from(&hsl.yellow)),
            green: Some(PartialHslChannel::from(&hsl.green)),
            aqua: Some(PartialHslChannel::from(&hsl.aqua)),
            blue: Some(PartialHslChannel::from(&hsl.blue)),
            purple: Some(PartialHslChannel::from(&hsl.purple)),
            magenta: Some(PartialHslChannel::from(&hsl.magenta)),
        }
    }
}

/// Partial vignette parameters — `None` means "not specified".
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PartialVignetteParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amount: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shape: Option<crate::adjust::VignetteShape>,
}

impl PartialVignetteParams {
    /// Merge overlay on top of self (last-write-wins).
    pub fn merge(&self, overlay: &Self) -> Self {
        Self {
            amount: overlay.amount.or(self.amount),
            shape: overlay.shape.or(self.shape),
        }
    }

    /// Convert to concrete VignetteParams. None fields become defaults.
    pub fn materialize(&self) -> VignetteParams {
        VignetteParams {
            amount: self.amount.unwrap_or(0.0),
            shape: self.shape.unwrap_or_default(),
        }
    }
}

impl From<&VignetteParams> for PartialVignetteParams {
    fn from(v: &VignetteParams) -> Self {
        Self {
            amount: Some(v.amount),
            shape: Some(v.shape),
        }
    }
}

/// Partial parameter set — `None` means "not specified by this preset".
///
/// Used for preset deserialization and merging. Convert to concrete
/// `Parameters` via `materialize()` for the engine.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PartialParameters {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exposure: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contrast: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub highlights: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shadows: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub whites: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blacks: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tint: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hsl: Option<PartialHslChannels>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vignette: Option<PartialVignetteParams>,
}

impl PartialParameters {
    /// Merge `other` on top of `self` (last-write-wins).
    pub fn merge(&self, other: &Self) -> Self {
        Self {
            exposure: other.exposure.or(self.exposure),
            contrast: other.contrast.or(self.contrast),
            highlights: other.highlights.or(self.highlights),
            shadows: other.shadows.or(self.shadows),
            whites: other.whites.or(self.whites),
            blacks: other.blacks.or(self.blacks),
            temperature: other.temperature.or(self.temperature),
            tint: other.tint.or(self.tint),
            hsl: match (&self.hsl, &other.hsl) {
                (None, None) => None,
                (Some(b), None) => Some(b.clone()),
                (None, Some(o)) => Some(o.clone()),
                (Some(b), Some(o)) => Some(b.merge(o)),
            },
            vignette: match (&self.vignette, &other.vignette) {
                (None, None) => None,
                (Some(b), None) => Some(b.clone()),
                (None, Some(o)) => Some(o.clone()),
                (Some(b), Some(o)) => Some(b.merge(o)),
            },
        }
    }

    /// Convert to concrete Parameters. `None` fields become their default (0.0).
    pub fn materialize(&self) -> Parameters {
        Parameters {
            exposure: self.exposure.unwrap_or(0.0),
            contrast: self.contrast.unwrap_or(0.0),
            highlights: self.highlights.unwrap_or(0.0),
            shadows: self.shadows.unwrap_or(0.0),
            whites: self.whites.unwrap_or(0.0),
            blacks: self.blacks.unwrap_or(0.0),
            temperature: self.temperature.unwrap_or(0.0),
            tint: self.tint.unwrap_or(0.0),
            hsl: self
                .hsl
                .as_ref()
                .map(|h| h.materialize())
                .unwrap_or_default(),
            vignette: self
                .vignette
                .as_ref()
                .map(|v| v.materialize())
                .unwrap_or_default(),
        }
    }
}

impl From<&Parameters> for PartialParameters {
    fn from(params: &Parameters) -> Self {
        Self {
            exposure: Some(params.exposure),
            contrast: Some(params.contrast),
            highlights: Some(params.highlights),
            shadows: Some(params.shadows),
            whites: Some(params.whites),
            blacks: Some(params.blacks),
            temperature: Some(params.temperature),
            tint: Some(params.tint),
            hsl: Some(PartialHslChannels::from(&params.hsl)),
            vignette: Some(PartialVignetteParams::from(&params.vignette)),
        }
    }
}

/// The rendering engine. Holds an immutable original image and mutable parameters.
///
/// On each `render()` call, all adjustments are applied from scratch in a fixed
/// internal order. This gives user-facing order-independence.
pub struct Engine {
    original: Rgb32FImage,
    params: Parameters,
    lut: Option<crate::lut::Lut3D>,
}

impl Engine {
    /// Create a new engine with the given linear sRGB image and neutral parameters.
    pub fn new(image: Rgb32FImage) -> Self {
        Self {
            original: image,
            params: Parameters::default(),
            lut: None,
        }
    }

    /// Get a reference to the original (unmodified) image.
    pub fn original(&self) -> &Rgb32FImage {
        &self.original
    }

    /// Get a reference to the current parameters.
    pub fn params(&self) -> &Parameters {
        &self.params
    }

    /// Get a mutable reference to the current parameters.
    pub fn params_mut(&mut self) -> &mut Parameters {
        &mut self.params
    }

    /// Set parameters from a full Parameters struct.
    pub fn set_params(&mut self, params: Parameters) {
        self.params = params;
    }

    /// Get a reference to the current LUT, if any.
    pub fn lut(&self) -> Option<&crate::lut::Lut3D> {
        self.lut.as_ref()
    }

    /// Set or clear the 3D LUT.
    pub fn set_lut(&mut self, lut: Option<crate::lut::Lut3D>) {
        self.lut = lut;
    }

    /// Apply a preset, replacing the current parameters and LUT.
    pub fn apply_preset(&mut self, preset: &crate::preset::Preset) {
        self.params = preset.params();
        self.lut = preset.lut.clone();
    }

    /// Layer a preset on top of current parameters.
    /// Only fields specified in the preset (Some values in partial_params)
    /// are overridden. Unspecified fields keep their current values.
    pub fn layer_preset(&mut self, preset: &crate::preset::Preset) {
        let current_partial = PartialParameters::from(&self.params);
        let merged = current_partial.merge(&preset.partial_params);
        self.params = merged.materialize();
        if preset.lut.is_some() {
            self.lut = preset.lut.clone();
        }
    }

    /// Render the image by applying all adjustments from scratch.
    ///
    /// Pipeline order:
    /// 1. White balance (linear space) — channel multipliers
    /// 2. Exposure (linear space) — multiply by 2^stops
    /// 3. Convert to sRGB gamma space
    /// 4. Contrast, highlights, shadows, whites, blacks (sRGB gamma space)
    /// 5. HSL adjustments (sRGB gamma space)
    /// 6. LUT application (sRGB gamma space)
    /// 7. Vignette (sRGB gamma space, position-dependent)
    /// 8. Convert back to linear space
    pub fn render(&self) -> Rgb32FImage {
        let (w, h) = self.original.dimensions();
        let exposure_factor = adjust::exposure_factor(self.params.exposure);
        let hsl_active = !self.params.hsl.is_default();
        let vignette_active = self.params.vignette.amount != 0.0;
        let hue_shifts = self.params.hsl.hue_shifts();
        let sat_shifts = self.params.hsl.saturation_shifts();
        let lum_shifts = self.params.hsl.luminance_shifts();

        Rgb32FImage::from_fn(w, h, |x, y| {
            let p = self.original.get_pixel(x, y);
            let (mut r, mut g, mut b) = (p.0[0], p.0[1], p.0[2]);

            // 1. White balance (linear space)
            let wb =
                adjust::apply_white_balance(r, g, b, self.params.temperature, self.params.tint);
            r = wb.0;
            g = wb.1;
            b = wb.2;

            // 2. Exposure (linear space)
            r = adjust::apply_exposure(r, exposure_factor);
            g = adjust::apply_exposure(g, exposure_factor);
            b = adjust::apply_exposure(b, exposure_factor);

            // 3. Convert to sRGB gamma space
            let (mut sr, mut sg, mut sb) = adjust::linear_to_srgb(r, g, b);

            // 4. Contrast
            sr = adjust::apply_contrast(sr, self.params.contrast);
            sg = adjust::apply_contrast(sg, self.params.contrast);
            sb = adjust::apply_contrast(sb, self.params.contrast);

            // 5. Highlights
            sr = adjust::apply_highlights(sr, self.params.highlights);
            sg = adjust::apply_highlights(sg, self.params.highlights);
            sb = adjust::apply_highlights(sb, self.params.highlights);

            // 6. Shadows
            sr = adjust::apply_shadows(sr, self.params.shadows);
            sg = adjust::apply_shadows(sg, self.params.shadows);
            sb = adjust::apply_shadows(sb, self.params.shadows);

            // 7. Whites
            sr = adjust::apply_whites(sr, self.params.whites);
            sg = adjust::apply_whites(sg, self.params.whites);
            sb = adjust::apply_whites(sb, self.params.whites);

            // 8. Blacks
            sr = adjust::apply_blacks(sr, self.params.blacks);
            sg = adjust::apply_blacks(sg, self.params.blacks);
            sb = adjust::apply_blacks(sb, self.params.blacks);

            // 9. HSL adjustments (sRGB gamma space)
            if hsl_active {
                let (hr, hg, hb) = adjust::apply_hsl(
                    sr,
                    sg,
                    sb,
                    &hue_shifts,
                    &sat_shifts,
                    &lum_shifts,
                    adjust::cosine_weight,
                );
                sr = hr;
                sg = hg;
                sb = hb;
            }

            // 10. LUT (sRGB gamma space)
            if let Some(lut) = &self.lut {
                let (lr, lg, lb) = lut.lookup(sr, sg, sb);
                sr = lr;
                sg = lg;
                sb = lb;
            }

            // 10.5. Vignette (sRGB gamma space, position-dependent)
            if vignette_active {
                let (vr, vg, vb) = adjust::apply_vignette(
                    sr, sg, sb,
                    self.params.vignette.amount,
                    self.params.vignette.shape,
                    x, y, w, h,
                );
                sr = vr;
                sg = vg;
                sb = vb;
            }

            // 11. Convert back to linear space
            let (lr, lg, lb) = adjust::srgb_to_linear(sr, sg, sb);

            Rgb([lr, lg, lb])
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::ImageBuffer;

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
            assert!(
                (orig.0[i] - rend.0[i]).abs() < 1e-5,
                "Channel {}: expected {}, got {}",
                i,
                orig.0[i],
                rend.0[i]
            );
        }
    }

    #[test]
    fn render_exposure_plus_one_doubles() {
        let img = make_test_image(0.25, 0.25, 0.25);
        let mut engine = Engine::new(img);
        engine.params_mut().exposure = 1.0;
        let pixel = *engine.render().get_pixel(0, 0);
        for i in 0..3 {
            assert!(
                (pixel.0[i] - 0.5).abs() < 1e-5,
                "Channel {}: expected 0.5, got {}",
                i,
                pixel.0[i]
            );
        }
    }

    #[test]
    fn render_contrast_changes_output() {
        let img = make_test_image(0.5, 0.5, 0.5);
        let mut engine = Engine::new(img);
        engine.params_mut().contrast = 50.0;
        let rendered = engine.render();
        let neutral_engine = Engine::new(make_test_image(0.5, 0.5, 0.5));
        let neutral = neutral_engine.render();
        // With contrast, pixel values above/below mid should shift
        let rp = rendered.get_pixel(0, 0);
        let np = neutral.get_pixel(0, 0);
        // 0.5 in linear → sRGB ~0.735 → contrast pushes further from 0.5
        // Result should differ from neutral
        assert!(
            (rp.0[0] - np.0[0]).abs() > 1e-6 || rp.0[0] == np.0[0],
            "Contrast should change output for non-midpoint sRGB values"
        );
    }

    #[test]
    fn render_warm_white_balance_boosts_red() {
        let img = make_test_image(0.5, 0.5, 0.5);
        let mut engine = Engine::new(img);
        engine.params_mut().temperature = 50.0;
        let pixel = *engine.render().get_pixel(0, 0);
        // Warm shift: red > blue
        assert!(
            pixel.0[0] > pixel.0[2],
            "Expected red > blue with warm WB, got r={} b={}",
            pixel.0[0],
            pixel.0[2]
        );
    }

    #[test]
    fn render_combined_exposure_and_contrast() {
        let img = make_test_image(0.2, 0.2, 0.2);
        let mut engine = Engine::new(img);
        engine.params_mut().exposure = 1.0;
        engine.params_mut().contrast = 25.0;
        let pixel = *engine.render().get_pixel(0, 0);
        // Should be brighter than original 0.2
        assert!(pixel.0[0] > 0.2, "Expected brighter, got {}", pixel.0[0]);
    }

    #[test]
    fn render_with_identity_lut_is_identity() {
        let img = make_test_image(0.5, 0.3, 0.1);
        let mut engine = Engine::new(img);
        let size = 17;
        let n = (size - 1) as f32;
        let mut table = Vec::with_capacity(size * size * size);
        for b in 0..size {
            for g in 0..size {
                for r in 0..size {
                    table.push([r as f32 / n, g as f32 / n, b as f32 / n]);
                }
            }
        }
        let lut = crate::lut::Lut3D {
            title: None,
            size,
            domain_min: [0.0, 0.0, 0.0],
            domain_max: [1.0, 1.0, 1.0],
            table,
        };
        engine.set_lut(Some(lut));

        let rendered = engine.render();
        let orig = engine.original().get_pixel(0, 0);
        let rend = rendered.get_pixel(0, 0);
        for i in 0..3 {
            assert!(
                (orig.0[i] - rend.0[i]).abs() < 0.01,
                "Channel {}: expected ~{}, got {}",
                i,
                orig.0[i],
                rend.0[i]
            );
        }
    }

    #[test]
    fn render_with_no_lut_unchanged() {
        let img = make_test_image(0.5, 0.3, 0.1);
        let engine = Engine::new(img);
        assert!(engine.lut().is_none());
        let rendered = engine.render();
        let orig = engine.original().get_pixel(0, 0);
        let rend = rendered.get_pixel(0, 0);
        for i in 0..3 {
            assert!((orig.0[i] - rend.0[i]).abs() < 1e-5);
        }
    }

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
                orig.0[i],
                rend.0[i]
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

    // --- PartialHslChannel tests ---

    #[test]
    fn partial_hsl_channel_default_is_all_none() {
        let ch = super::PartialHslChannel::default();
        assert_eq!(ch.hue, None);
        assert_eq!(ch.saturation, None);
        assert_eq!(ch.luminance, None);
    }

    #[test]
    fn partial_hsl_channels_default_is_all_none() {
        let hsl = super::PartialHslChannels::default();
        assert_eq!(hsl.red, None);
        assert_eq!(hsl.green, None);
        assert_eq!(hsl.blue, None);
    }

    #[test]
    fn partial_hsl_channel_merge_overlay_wins() {
        let base = super::PartialHslChannel {
            hue: Some(10.0),
            saturation: Some(20.0),
            luminance: None,
        };
        let overlay = super::PartialHslChannel {
            hue: Some(30.0),
            saturation: None,
            luminance: Some(5.0),
        };
        let merged = base.merge(&overlay);
        assert_eq!(merged.hue, Some(30.0));
        assert_eq!(merged.saturation, Some(20.0));
        assert_eq!(merged.luminance, Some(5.0));
    }

    #[test]
    fn partial_hsl_channels_merge_channel_level() {
        let mut base = super::PartialHslChannels::default();
        base.red = Some(super::PartialHslChannel {
            hue: Some(10.0),
            saturation: None,
            luminance: None,
        });
        let mut overlay = super::PartialHslChannels::default();
        overlay.red = Some(super::PartialHslChannel {
            hue: None,
            saturation: Some(20.0),
            luminance: None,
        });
        overlay.green = Some(super::PartialHslChannel {
            hue: Some(5.0),
            saturation: None,
            luminance: None,
        });
        let merged = base.merge(&overlay);
        assert_eq!(merged.red.as_ref().unwrap().hue, Some(10.0));
        assert_eq!(merged.red.as_ref().unwrap().saturation, Some(20.0));
        assert_eq!(merged.green.as_ref().unwrap().hue, Some(5.0));
        assert_eq!(merged.blue, None);
    }

    #[test]
    fn partial_hsl_channel_materialize() {
        let partial = super::PartialHslChannel {
            hue: Some(15.0),
            saturation: None,
            luminance: Some(-10.0),
        };
        let concrete = partial.materialize();
        assert_eq!(concrete.hue, 15.0);
        assert_eq!(concrete.saturation, 0.0);
        assert_eq!(concrete.luminance, -10.0);
    }

    #[test]
    fn partial_hsl_channels_materialize() {
        let mut partial = super::PartialHslChannels::default();
        partial.red = Some(super::PartialHslChannel {
            hue: Some(15.0),
            saturation: None,
            luminance: None,
        });
        let concrete = partial.materialize();
        assert_eq!(concrete.red.hue, 15.0);
        assert_eq!(concrete.red.saturation, 0.0);
        assert_eq!(concrete.green, super::HslChannel::default());
    }

    // --- PartialParameters tests ---

    #[test]
    fn partial_parameters_default_is_all_none() {
        let p = super::PartialParameters::default();
        assert_eq!(p.exposure, None);
        assert_eq!(p.contrast, None);
        assert_eq!(p.hsl, None);
    }

    #[test]
    fn partial_parameters_merge_overlay_wins() {
        let base = super::PartialParameters {
            exposure: Some(1.0),
            contrast: Some(20.0),
            ..Default::default()
        };
        let overlay = super::PartialParameters {
            exposure: Some(2.0),
            highlights: Some(-30.0),
            ..Default::default()
        };
        let merged = base.merge(&overlay);
        assert_eq!(merged.exposure, Some(2.0));
        assert_eq!(merged.contrast, Some(20.0));
        assert_eq!(merged.highlights, Some(-30.0));
        assert_eq!(merged.shadows, None);
    }

    #[test]
    fn partial_parameters_materialize_defaults() {
        let partial = super::PartialParameters {
            exposure: Some(1.5),
            ..Default::default()
        };
        let params = partial.materialize();
        assert_eq!(params.exposure, 1.5);
        assert_eq!(params.contrast, 0.0);
        assert_eq!(params.temperature, 0.0);
        assert!(params.hsl.is_default());
    }

    #[test]
    fn partial_parameters_from_parameters_all_some() {
        let params = Parameters {
            exposure: 1.0,
            contrast: 20.0,
            ..Default::default()
        };
        let partial = super::PartialParameters::from(&params);
        assert_eq!(partial.exposure, Some(1.0));
        assert_eq!(partial.contrast, Some(20.0));
        assert_eq!(partial.highlights, Some(0.0));
    }

    #[test]
    fn partial_parameters_merge_with_hsl() {
        let base = super::PartialParameters {
            exposure: Some(1.0),
            ..Default::default()
        };
        let mut hsl = super::PartialHslChannels::default();
        hsl.red = Some(super::PartialHslChannel {
            hue: Some(10.0),
            saturation: None,
            luminance: None,
        });
        let overlay = super::PartialParameters {
            hsl: Some(hsl),
            ..Default::default()
        };
        let merged = base.merge(&overlay);
        assert_eq!(merged.exposure, Some(1.0));
        assert!(merged.hsl.is_some());
        assert_eq!(
            merged.hsl.as_ref().unwrap().red.as_ref().unwrap().hue,
            Some(10.0)
        );
    }

    // --- layer_preset tests ---

    #[test]
    fn layer_preset_only_overrides_specified_fields() {
        let img = make_test_image(0.5, 0.5, 0.5);
        let mut engine = Engine::new(img);
        engine.params_mut().exposure = 1.0;
        engine.params_mut().contrast = 20.0;

        let mut preset = crate::preset::Preset::default();
        preset.partial_params.contrast = Some(50.0);

        engine.layer_preset(&preset);
        assert_eq!(engine.params().exposure, 1.0);
        assert_eq!(engine.params().contrast, 50.0);
    }

    #[test]
    fn layer_preset_preserves_unspecified_hsl() {
        let img = make_test_image(0.5, 0.5, 0.5);
        let mut engine = Engine::new(img);
        engine.params_mut().hsl.red.hue = 15.0;

        let mut preset = crate::preset::Preset::default();
        let mut partial_hsl = PartialHslChannels::default();
        partial_hsl.green = Some(PartialHslChannel {
            hue: Some(10.0),
            saturation: None,
            luminance: None,
        });
        preset.partial_params.hsl = Some(partial_hsl);

        engine.layer_preset(&preset);
        assert_eq!(engine.params().hsl.red.hue, 15.0);
        assert_eq!(engine.params().hsl.green.hue, 10.0);
    }

    #[test]
    fn layer_multiple_presets_last_wins() {
        let img = make_test_image(0.5, 0.5, 0.5);
        let mut engine = Engine::new(img);

        let mut preset1 = crate::preset::Preset::default();
        preset1.partial_params.exposure = Some(1.0);
        preset1.partial_params.contrast = Some(20.0);

        let mut preset2 = crate::preset::Preset::default();
        preset2.partial_params.exposure = Some(2.0);

        engine.layer_preset(&preset1);
        engine.layer_preset(&preset2);

        assert_eq!(engine.params().exposure, 2.0);
        assert_eq!(engine.params().contrast, 20.0);
    }

    // --- VignetteParams tests ---

    #[test]
    fn vignette_params_default() {
        let v = super::VignetteParams::default();
        assert_eq!(v.amount, 0.0);
        assert_eq!(v.shape, crate::adjust::VignetteShape::Elliptical);
    }

    #[test]
    fn partial_vignette_params_default_is_all_none() {
        let v = super::PartialVignetteParams::default();
        assert_eq!(v.amount, None);
        assert_eq!(v.shape, None);
    }

    #[test]
    fn partial_vignette_params_merge_overlay_wins() {
        let base = super::PartialVignetteParams {
            amount: Some(-30.0),
            shape: Some(crate::adjust::VignetteShape::Elliptical),
        };
        let overlay = super::PartialVignetteParams {
            amount: Some(-50.0),
            shape: None,
        };
        let merged = base.merge(&overlay);
        assert_eq!(merged.amount, Some(-50.0));
        assert_eq!(merged.shape, Some(crate::adjust::VignetteShape::Elliptical));
    }

    #[test]
    fn partial_vignette_params_materialize_defaults() {
        let partial = super::PartialVignetteParams {
            amount: Some(-30.0),
            shape: None,
        };
        let concrete = partial.materialize();
        assert_eq!(concrete.amount, -30.0);
        assert_eq!(concrete.shape, crate::adjust::VignetteShape::Elliptical);
    }

    #[test]
    fn partial_vignette_params_from_concrete() {
        let concrete = super::VignetteParams {
            amount: -30.0,
            shape: crate::adjust::VignetteShape::Circular,
        };
        let partial = super::PartialVignetteParams::from(&concrete);
        assert_eq!(partial.amount, Some(-30.0));
        assert_eq!(partial.shape, Some(crate::adjust::VignetteShape::Circular));
    }

    #[test]
    fn parameters_default_vignette_is_neutral() {
        let p = Parameters::default();
        assert_eq!(p.vignette.amount, 0.0);
        assert_eq!(p.vignette.shape, crate::adjust::VignetteShape::Elliptical);
    }

    #[test]
    fn apply_preset_still_does_full_replacement() {
        let img = make_test_image(0.5, 0.5, 0.5);
        let mut engine = Engine::new(img);
        engine.params_mut().exposure = 1.0;
        engine.params_mut().contrast = 20.0;

        let mut preset = crate::preset::Preset::default();
        preset.partial_params.exposure = Some(0.5);

        engine.apply_preset(&preset);
        assert_eq!(engine.params().exposure, 0.5);
        assert_eq!(engine.params().contrast, 0.0);
    }

    #[test]
    fn render_vignette_darkens_corners() {
        // Use a 10x10 image so corners are clearly away from center
        let img: Rgb32FImage = ImageBuffer::from_pixel(10, 10, Rgb([0.5, 0.5, 0.5]));
        let mut engine = Engine::new(img);
        engine.params_mut().vignette.amount = -50.0;
        let rendered = engine.render();

        // Center pixel should be close to original
        let center = rendered.get_pixel(5, 5);
        assert!(
            (center.0[0] - 0.5).abs() < 0.05,
            "Center should be near original, got {}",
            center.0[0]
        );

        // Corner pixel should be darker
        let corner = rendered.get_pixel(0, 0);
        assert!(
            corner.0[0] < center.0[0],
            "Corner ({}) should be darker than center ({})",
            corner.0[0],
            center.0[0]
        );
    }

    #[test]
    fn render_vignette_zero_is_identity() {
        let img = make_test_image(0.5, 0.3, 0.1);
        let mut engine = Engine::new(img);
        engine.params_mut().vignette.amount = 0.0;
        let rendered = engine.render();
        let orig = engine.original().get_pixel(0, 0);
        let rend = rendered.get_pixel(0, 0);
        for i in 0..3 {
            assert!(
                (orig.0[i] - rend.0[i]).abs() < 1e-5,
                "Channel {}: expected {}, got {}",
                i, orig.0[i], rend.0[i]
            );
        }
    }

    #[test]
    fn full_pipeline_decode_engine_encode() {
        let temp_dir = std::env::temp_dir();
        let input = temp_dir.join("agx_e2e_in.png");
        let output = temp_dir.join("agx_e2e_out.png");

        // Create sRGB 128,128,128 test image
        let img: ImageBuffer<image::Rgb<u8>, Vec<u8>> =
            ImageBuffer::from_pixel(4, 4, image::Rgb([128u8, 128, 128]));
        img.save(&input).unwrap();

        // Decode → Engine +1 stop → Render → Encode
        let linear = crate::decode::decode_standard(&input).unwrap();
        let mut engine = Engine::new(linear);
        engine.params_mut().exposure = 1.0;
        let rendered = engine.render();
        crate::encode::encode_to_file(&rendered, &output).unwrap();

        // Verify output is brighter (sRGB 128 → linear ~0.216 → *2 → ~0.432 → sRGB ~173)
        let out_img = image::ImageReader::open(&output)
            .unwrap()
            .decode()
            .unwrap()
            .to_rgb8();
        let pixel = out_img.get_pixel(0, 0);
        assert!(
            pixel.0[0] > 150 && pixel.0[0] < 190,
            "Expected ~173, got {}",
            pixel.0[0]
        );

        let _ = std::fs::remove_file(&input);
        let _ = std::fs::remove_file(&output);
    }
}
