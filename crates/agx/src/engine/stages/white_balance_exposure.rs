use crate::adjust;
use crate::engine::{ColorSpace, Parameters, RenderContext, Stage};
use crate::error::AgxError;

/// Applies white balance and exposure in linear space.
pub struct WhiteBalanceExposureStage;

impl Default for WhiteBalanceExposureStage {
    fn default() -> Self {
        Self
    }
}

impl WhiteBalanceExposureStage {
    /// Create a new white balance and exposure stage.
    pub fn new() -> Self {
        Self
    }
}

impl Stage for WhiteBalanceExposureStage {
    fn name(&self) -> &'static str {
        "white_balance_exposure"
    }

    fn input_color_space(&self) -> ColorSpace {
        ColorSpace::LinearSrgb
    }

    fn output_color_space(&self) -> ColorSpace {
        ColorSpace::LinearSrgb
    }

    fn is_active(&self, params: &Parameters) -> bool {
        params.temperature != 0.0 || params.tint != 0.0 || params.exposure != 0.0
    }

    fn prepare(&mut self, _params: &Parameters) {}

    fn process(&self, ctx: &mut RenderContext) -> Result<(), AgxError> {
        adjust::apply_white_balance_exposure_buffer(
            &mut ctx.buf,
            ctx.params.temperature,
            ctx.params.tint,
            ctx.params.exposure,
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::Parameters;

    #[test]
    fn stage_neutral_params_is_identity() {
        let params = Parameters::default();
        let pixels = vec![[0.5, 0.3, 0.1]];
        let mut ctx = RenderContext {
            buf: pixels.clone(),
            width: 1,
            height: 1,
            params: &params,
            lut: None,
        };
        let mut stage = WhiteBalanceExposureStage::new();
        stage.prepare(&params);
        stage.process(&mut ctx).unwrap();
        for (c, &v) in ctx.buf[0].iter().enumerate() {
            assert!(
                (v - pixels[0][c]).abs() < 1e-6,
                "channel {c} changed with neutral params"
            );
        }
    }

    #[test]
    fn stage_color_space_is_linear() {
        let stage = WhiteBalanceExposureStage::new();
        assert_eq!(stage.input_color_space(), ColorSpace::LinearSrgb);
        assert_eq!(stage.output_color_space(), ColorSpace::LinearSrgb);
    }
}
