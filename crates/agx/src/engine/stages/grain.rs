use crate::adjust;
use crate::engine::{ColorSpace, RenderContext, Stage, StageInputs};
use crate::error::AgxError;

/// Film grain simulation via noise generation + Gaussian blur.
/// Operates in the gamma Rec.2020 working space as a buffer-level pass.
pub struct GrainStage {
    seed: u64,
}

impl Default for GrainStage {
    fn default() -> Self {
        Self::new()
    }
}

impl GrainStage {
    /// Create a new grain stage with a zero seed (overwritten during prepare).
    pub fn new() -> Self {
        Self { seed: 0 }
    }
}

impl Stage for GrainStage {
    fn name(&self) -> &'static str {
        "grain"
    }

    fn input_color_space(&self, _inp: &StageInputs) -> ColorSpace {
        ColorSpace::GammaRec2020
    }

    fn output_color_space(&self, _inp: &StageInputs) -> ColorSpace {
        ColorSpace::GammaRec2020
    }

    fn is_active(&self, inp: &StageInputs) -> bool {
        !inp.params.grain.is_neutral()
    }

    fn prepare(&mut self, inp: &StageInputs) {
        self.seed = inp.params.grain.seed.unwrap_or_else(rand::random::<u64>);
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
        let inp = crate::engine::StageInputs { params: &params, lut: None };
        let stage = GrainStage::new();
        assert!(!stage.is_active(&inp));
    }

    #[test]
    fn grain_active_when_nonzero() {
        let mut params = Parameters::default();
        params.grain.amount = 50.0;
        let inp = crate::engine::StageInputs { params: &params, lut: None };
        let stage = GrainStage::new();
        assert!(stage.is_active(&inp));
    }

    #[test]
    fn grain_color_space_is_srgb() {
        let params = Parameters::default();
        let inp = crate::engine::StageInputs { params: &params, lut: None };
        let stage = GrainStage::new();
        assert_eq!(stage.input_color_space(&inp), ColorSpace::GammaRec2020);
        assert_eq!(stage.output_color_space(&inp), ColorSpace::GammaRec2020);
    }
}
