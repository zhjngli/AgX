use crate::adjust;
use crate::engine::{ColorSpace, Parameters, RenderContext, Stage};
use crate::error::AgxError;

/// Applies all per-pixel sRGB gamma-space adjustments: contrast, highlights,
/// shadows, whites, blacks, tone curves, HSL, color grading, and LUT.
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

    fn input_color_space(&self) -> ColorSpace {
        ColorSpace::SrgbGamma
    }

    fn output_color_space(&self) -> ColorSpace {
        ColorSpace::SrgbGamma
    }

    fn is_active(&self, _params: &Parameters) -> bool {
        true // always active — even neutral params need to be checked per-pixel
    }

    fn prepare(&mut self, params: &Parameters) {
        self.tone_curve_pre = (!params.tone_curve.is_default())
            .then(|| adjust::ToneCurvePrecomputed::new(&params.tone_curve));
        self.color_grading_pre = (!params.color_grading.is_default())
            .then(|| adjust::ColorGradingPrecomputed::new(&params.color_grading));
    }

    fn process(&self, ctx: &mut RenderContext) -> Result<(), AgxError> {
        let lut_lookup = ctx
            .lut
            .map(|lut| move |r: f32, g: f32, b: f32| lut.lookup(r, g, b));
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
            lut_fn: lut_lookup
                .as_ref()
                .map(|f| f as &dyn Fn(f32, f32, f32) -> (f32, f32, f32)),
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
        stage.prepare(&params);
        stage.process(&mut ctx).unwrap();
        for c in 0..3 {
            assert!(
                (ctx.buf[0][c] - pixels[0][c]).abs() < 1e-6,
                "channel {} changed with neutral params",
                c
            );
        }
    }

    #[test]
    fn per_pixel_stage_color_space_is_srgb() {
        let stage = PerPixelAdjustmentsStage::new();
        assert_eq!(stage.input_color_space(), ColorSpace::SrgbGamma);
        assert_eq!(stage.output_color_space(), ColorSpace::SrgbGamma);
    }

    #[test]
    fn per_pixel_stage_always_active() {
        let params = Parameters::default();
        let stage = PerPixelAdjustmentsStage::new();
        assert!(stage.is_active(&params));
    }
}
