use crate::adjust;
use crate::engine::{ColorSpace, RenderContext, Stage, StageInputs};
use crate::error::AgxError;

/// Applies all per-pixel gamma-encoded adjustments to the gamma Rec.2020
/// working buffer: contrast, highlights, shadows, whites, blacks, tone
/// curves, HSL, and color grading. LUT sampling is handled by `LutStage`.
pub struct PerPixelAdjustmentsStage {
    tone_curve_pre: Option<adjust::ToneCurvePrecomputed>,
    color_grading_pre: Option<adjust::ColorGradingPrecomputed>,
}

impl Default for PerPixelAdjustmentsStage {
    fn default() -> Self {
        Self::new()
    }
}

impl PerPixelAdjustmentsStage {
    /// Create a new per-pixel adjustments stage with no precomputed state.
    pub fn new() -> Self {
        Self {
            tone_curve_pre: None,
            color_grading_pre: None,
        }
    }
}

impl Stage for PerPixelAdjustmentsStage {
    fn name(&self) -> &'static str {
        "per_pixel_adjustments"
    }

    fn input_color_space(&self, _inp: &StageInputs) -> ColorSpace {
        ColorSpace::GammaRec2020
    }

    fn output_color_space(&self, _inp: &StageInputs) -> ColorSpace {
        ColorSpace::GammaRec2020
    }

    fn is_active(&self, _inp: &StageInputs) -> bool {
        true // always active — even neutral params need to be checked per-pixel
    }

    fn prepare(&mut self, inp: &StageInputs) {
        self.tone_curve_pre = (!inp.params.tone_curve.is_default())
            .then(|| adjust::ToneCurvePrecomputed::new(&inp.params.tone_curve));
        self.color_grading_pre = (!inp.params.color_grading.is_default())
            .then(|| adjust::ColorGradingPrecomputed::new(&inp.params.color_grading));
    }

    fn process(&self, ctx: &mut RenderContext) -> Result<(), AgxError> {
        let pp = adjust::PerPixelParams {
            contrast: ctx.params.contrast,
            highlights: ctx.params.highlights,
            shadows: ctx.params.shadows,
            whites: ctx.params.whites,
            blacks: ctx.params.blacks,
            tone_curve_pre: self.tone_curve_pre.as_ref(),
            hsl_active: !ctx.params.hsl.is_default(),
            hue_shifts: ctx.params.hsl.hue_shifts(),
            sat_shifts: ctx.params.hsl.saturation_shifts(),
            lum_shifts: ctx.params.hsl.luminance_shifts(),
            color_grading_pre: self.color_grading_pre,
        };
        adjust::apply_per_pixel_adjustments(&mut ctx.buf, &pp);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::Parameters;

    #[test]
    fn per_pixel_stage_neutral_is_identity() {
        let params = Parameters::default();
        let pixels = vec![[0.7, 0.5, 0.3]];
        let mut ctx = RenderContext {
            buf: pixels.clone(),
            width: 1,
            height: 1,
            params: &params,
            lut: None,
        };
        let mut stage = PerPixelAdjustmentsStage::new();
        let inp = crate::engine::StageInputs {
            params: &params,
            lut: None,
        };
        stage.prepare(&inp);
        stage.process(&mut ctx).unwrap();
        for (c, &v) in ctx.buf[0].iter().enumerate() {
            assert!(
                (v - pixels[0][c]).abs() < 1e-6,
                "channel {c} changed with neutral params"
            );
        }
    }

    #[test]
    fn per_pixel_stage_color_space_is_srgb() {
        let params = Parameters::default();
        let inp = crate::engine::StageInputs {
            params: &params,
            lut: None,
        };
        let stage = PerPixelAdjustmentsStage::new();
        assert_eq!(stage.input_color_space(&inp), ColorSpace::GammaRec2020);
        assert_eq!(stage.output_color_space(&inp), ColorSpace::GammaRec2020);
    }

    #[test]
    fn per_pixel_stage_always_active() {
        let params = Parameters::default();
        let inp = crate::engine::StageInputs {
            params: &params,
            lut: None,
        };
        let stage = PerPixelAdjustmentsStage::new();
        assert!(stage.is_active(&inp));
    }
}
