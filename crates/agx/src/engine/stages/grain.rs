use crate::adjust;
use crate::engine::{ColorSpace, Parameters, RenderContext, Stage};
use crate::error::AgxError;

/// Film grain simulation via noise generation + Gaussian blur.
/// Operates in sRGB gamma space as a buffer-level pass.
pub struct GrainStage {
    seed: u64,
}

impl GrainStage {
    pub fn new() -> Self {
        Self { seed: 0 }
    }
}

impl Stage for GrainStage {
    fn name(&self) -> &'static str {
        "grain"
    }

    fn input_color_space(&self) -> ColorSpace {
        ColorSpace::SrgbGamma
    }

    fn output_color_space(&self) -> ColorSpace {
        ColorSpace::SrgbGamma
    }

    fn is_active(&self, params: &Parameters) -> bool {
        !params.grain.is_neutral()
    }

    fn prepare(&mut self, params: &Parameters) {
        self.seed = params.grain.seed.unwrap_or_else(rand::random::<u64>);
    }

    fn process(&self, ctx: &mut RenderContext) -> Result<(), AgxError> {
        adjust::grain::apply_grain_buffer(
            &mut ctx.buf,
            ctx.width as usize,
            ctx.height as usize,
            &ctx.params.grain,
            self.seed,
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::Parameters;

    #[test]
    fn grain_inactive_when_neutral() {
        let params = Parameters::default();
        let stage = GrainStage::new();
        assert!(!stage.is_active(&params));
    }

    #[test]
    fn grain_active_when_nonzero() {
        let mut params = Parameters::default();
        params.grain.amount = 50.0;
        let stage = GrainStage::new();
        assert!(stage.is_active(&params));
    }

    #[test]
    fn grain_color_space_is_srgb() {
        let stage = GrainStage::new();
        assert_eq!(stage.input_color_space(), ColorSpace::SrgbGamma);
        assert_eq!(stage.output_color_space(), ColorSpace::SrgbGamma);
    }
}
