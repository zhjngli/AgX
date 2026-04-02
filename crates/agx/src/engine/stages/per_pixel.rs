use crate::adjust;
use crate::engine::{ColorSpace, Parameters, RenderContext, Stage};
use crate::error::AgxError;

/// Applies all per-pixel sRGB gamma-space adjustments: contrast, highlights,
/// shadows, whites, blacks, tone curves, HSL, color grading, and LUT.
pub struct PerPixelAdjustmentsStage {
    tone_curve_pre: Option<adjust::ToneCurvePrecomputed>,
    color_grading_pre: Option<adjust::ColorGradingPrecomputed>,
    hue_shifts: [f32; 8],
    sat_shifts: [f32; 8],
    lum_shifts: [f32; 8],
    hsl_active: bool,
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
            hue_shifts: [0.0; 8],
            sat_shifts: [0.0; 8],
            lum_shifts: [0.0; 8],
            hsl_active: false,
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
        self.hsl_active = !params.hsl.is_default();
        self.hue_shifts = params.hsl.hue_shifts();
        self.sat_shifts = params.hsl.saturation_shifts();
        self.lum_shifts = params.hsl.luminance_shifts();
    }

    fn process(&self, ctx: &mut RenderContext) -> Result<(), AgxError> {
        let pp = adjust::PerPixelParams {
            contrast: ctx.params.contrast,
            highlights: ctx.params.highlights,
            shadows: ctx.params.shadows,
            whites: ctx.params.whites,
            blacks: ctx.params.blacks,
            tone_curve_pre: self.tone_curve_pre.clone(),
            hsl_active: self.hsl_active,
            hue_shifts: self.hue_shifts,
            sat_shifts: self.sat_shifts,
            lum_shifts: self.lum_shifts,
            color_grading_pre: self.color_grading_pre,
            lut: ctx.lut,
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
