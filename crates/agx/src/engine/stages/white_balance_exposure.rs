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

    fn is_active(&self, _params: &Parameters) -> bool {
        true
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

    fn make_ctx(pixels: Vec<[f32; 3]>, w: u32, h: u32) -> RenderContext<'static> {
        let params = Box::leak(Box::new(Parameters::default()));
        RenderContext {
            buf: pixels,
            width: w,
            height: h,
            params,
            lut: None,
        }
    }

    #[test]
    fn stage_neutral_params_is_identity() {
        let pixels = vec![[0.5, 0.3, 0.1]];
        let mut ctx = make_ctx(pixels.clone(), 1, 1);
        let mut stage = WhiteBalanceExposureStage::new();
        stage.prepare(ctx.params);
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
    fn stage_color_space_is_linear() {
        let stage = WhiteBalanceExposureStage::new();
        assert_eq!(stage.input_color_space(), ColorSpace::LinearSrgb);
        assert_eq!(stage.output_color_space(), ColorSpace::LinearSrgb);
    }
}
