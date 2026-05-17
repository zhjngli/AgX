use crate::adjust;
use crate::engine::{ColorSpace, Parameters, RenderContext, Stage};
use crate::error::AgxError;

/// Wavelet-based noise reduction. Operates in linear space.
pub struct DenoiseStage;

impl Default for DenoiseStage {
    fn default() -> Self {
        Self
    }
}

impl DenoiseStage {
    /// Create a new denoise stage.
    pub fn new() -> Self {
        Self
    }
}

impl Stage for DenoiseStage {
    fn name(&self) -> &'static str {
        "denoise"
    }

    fn input_color_space(&self) -> ColorSpace {
        ColorSpace::LinearRec2020
    }

    fn output_color_space(&self) -> ColorSpace {
        ColorSpace::LinearRec2020
    }

    fn is_active(&self, params: &Parameters) -> bool {
        !params.noise_reduction.is_neutral()
    }

    fn prepare(&mut self, _params: &Parameters) {}

    fn process(&self, ctx: &mut RenderContext) -> Result<(), AgxError> {
        let result = adjust::denoise::apply_noise_reduction(
            &ctx.buf,
            ctx.width as usize,
            ctx.height as usize,
            &ctx.params.noise_reduction,
        );
        ctx.buf = result;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::Parameters;

    #[test]
    fn denoise_inactive_when_neutral() {
        let params = Parameters::default();
        let stage = DenoiseStage::new();
        assert!(!stage.is_active(&params));
    }

    #[test]
    fn denoise_active_when_nonzero() {
        let mut params = Parameters::default();
        params.noise_reduction.luminance = 50.0;
        let stage = DenoiseStage::new();
        assert!(stage.is_active(&params));
    }

    #[test]
    fn denoise_color_space_is_linear() {
        let stage = DenoiseStage::new();
        assert_eq!(stage.input_color_space(), ColorSpace::LinearRec2020);
        assert_eq!(stage.output_color_space(), ColorSpace::LinearRec2020);
    }
}
