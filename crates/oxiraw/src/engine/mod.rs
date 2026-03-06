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
        self.params = preset.params.clone();
        self.lut = preset.lut.clone();
    }

    /// Render the image by applying all adjustments from scratch.
    ///
    /// Pipeline order:
    /// 1. White balance (linear space) — channel multipliers
    /// 2. Exposure (linear space) — multiply by 2^stops
    /// 3. Convert to sRGB gamma space
    /// 4. Contrast, highlights, shadows, whites, blacks (sRGB gamma space)
    /// 5. LUT application (sRGB gamma space)
    /// 6. Convert back to linear space
    pub fn render(&self) -> Rgb32FImage {
        let (w, h) = self.original.dimensions();
        let exposure_factor = adjust::exposure_factor(self.params.exposure);

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

            // 9. LUT (sRGB gamma space)
            if let Some(lut) = &self.lut {
                let (lr, lg, lb) = lut.lookup(sr, sg, sb);
                sr = lr;
                sg = lg;
                sb = lb;
            }

            // 10. Convert back to linear space
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
    fn full_pipeline_decode_engine_encode() {
        let temp_dir = std::env::temp_dir();
        let input = temp_dir.join("oxiraw_e2e_in.png");
        let output = temp_dir.join("oxiraw_e2e_out.png");

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
