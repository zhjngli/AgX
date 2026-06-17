use crate::color_space::{srgb_curve_signed, srgb_curve_signed_inverse};
use crate::engine::{ColorSpace, RenderContext, Stage, StageInputs};
use crate::error::AgxError;
use rayon::prelude::*;

/// Converts the buffer from linear Rec.2020 to gamma-encoded Rec.2020 by
/// applying the sRGB transfer curve to each Rec.2020 linear channel.
pub struct LinearToGammaStage;

impl Default for LinearToGammaStage {
    fn default() -> Self {
        Self
    }
}

impl LinearToGammaStage {
    /// Create a new linear-to-gamma conversion stage.
    pub fn new() -> Self {
        Self
    }
}

impl Stage for LinearToGammaStage {
    fn name(&self) -> &'static str {
        "linear_to_gamma"
    }

    fn input_color_space(&self, _inp: &StageInputs) -> ColorSpace {
        ColorSpace::LinearRec2020
    }

    fn output_color_space(&self, _inp: &StageInputs) -> ColorSpace {
        ColorSpace::GammaRec2020
    }

    fn is_active(&self, _inp: &StageInputs) -> bool {
        true
    }

    fn prepare(&mut self, _inp: &StageInputs) {}

    fn process(&self, ctx: &mut RenderContext) -> Result<(), AgxError> {
        ctx.buf.par_iter_mut().for_each(|pixel| {
            pixel[0] = srgb_curve_signed(pixel[0]);
            pixel[1] = srgb_curve_signed(pixel[1]);
            pixel[2] = srgb_curve_signed(pixel[2]);
        });
        Ok(())
    }
}

/// Converts the buffer from gamma-encoded Rec.2020 to linear Rec.2020 by
/// applying the inverse sRGB transfer curve to each channel.
pub struct GammaToLinearStage;

impl Default for GammaToLinearStage {
    fn default() -> Self {
        Self
    }
}

impl GammaToLinearStage {
    /// Create a new gamma-to-linear conversion stage.
    pub fn new() -> Self {
        Self
    }
}

impl Stage for GammaToLinearStage {
    fn name(&self) -> &'static str {
        "gamma_to_linear"
    }

    fn input_color_space(&self, _inp: &StageInputs) -> ColorSpace {
        ColorSpace::GammaRec2020
    }

    fn output_color_space(&self, _inp: &StageInputs) -> ColorSpace {
        ColorSpace::LinearRec2020
    }

    fn is_active(&self, _inp: &StageInputs) -> bool {
        true
    }

    fn prepare(&mut self, _inp: &StageInputs) {}

    fn process(&self, ctx: &mut RenderContext) -> Result<(), AgxError> {
        ctx.buf.par_iter_mut().for_each(|pixel| {
            pixel[0] = srgb_curve_signed_inverse(pixel[0]);
            pixel[1] = srgb_curve_signed_inverse(pixel[1]);
            pixel[2] = srgb_curve_signed_inverse(pixel[2]);
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::Parameters;

    #[test]
    fn linear_to_gamma_roundtrip() {
        let params = Parameters::default();
        let pixels = vec![[0.5, 0.3, 0.1], [0.0, 1.0, 0.25]];
        let mut ctx = RenderContext {
            buf: pixels.clone(),
            width: 2,
            height: 1,
            params: &params,
            lut: None,
        };

        let inp = crate::engine::StageInputs { params: &params, lut: None };
        let mut to_gamma = LinearToGammaStage::new();
        to_gamma.prepare(&inp);
        to_gamma.process(&mut ctx).unwrap();

        assert!(
            (ctx.buf[0][0] - 0.5).abs() > 0.01,
            "gamma encoding should change 0.5"
        );

        let mut to_linear = GammaToLinearStage::new();
        to_linear.prepare(&inp);
        to_linear.process(&mut ctx).unwrap();

        for (i, pixel) in ctx.buf.iter().enumerate() {
            for (c, &v) in pixel.iter().enumerate() {
                assert!(
                    (v - pixels[i][c]).abs() < 1e-5,
                    "pixel[{i}][{c}]: expected {}, got {v}",
                    pixels[i][c]
                );
            }
        }
    }

    #[test]
    fn linear_to_gamma_always_active() {
        let params = Parameters::default();
        let inp = crate::engine::StageInputs { params: &params, lut: None };
        let stage = LinearToGammaStage::new();
        assert!(stage.is_active(&inp));
    }

    #[test]
    fn gamma_to_linear_always_active() {
        let params = Parameters::default();
        let inp = crate::engine::StageInputs { params: &params, lut: None };
        let stage = GammaToLinearStage::new();
        assert!(stage.is_active(&inp));
    }

    #[test]
    fn color_space_declarations_correct() {
        let params = Parameters::default();
        let inp = crate::engine::StageInputs { params: &params, lut: None };
        let to_gamma = LinearToGammaStage::new();
        assert_eq!(to_gamma.input_color_space(&inp), ColorSpace::LinearRec2020);
        assert_eq!(to_gamma.output_color_space(&inp), ColorSpace::GammaRec2020);

        let to_linear = GammaToLinearStage::new();
        assert_eq!(to_linear.input_color_space(&inp), ColorSpace::GammaRec2020);
        assert_eq!(to_linear.output_color_space(&inp), ColorSpace::LinearRec2020);
    }
}
