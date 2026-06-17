use crate::adjust;
use crate::engine::{ColorSpace, RenderContext, Stage, StageInputs};
use crate::error::AgxError;

/// Position-dependent edge darkening/brightening.
/// Operates in the gamma Rec.2020 working space.
pub struct VignetteStage;

impl Default for VignetteStage {
    fn default() -> Self {
        Self
    }
}

impl VignetteStage {
    /// Create a new vignette stage.
    pub fn new() -> Self {
        Self
    }
}

impl Stage for VignetteStage {
    fn name(&self) -> &'static str {
        "vignette"
    }

    fn input_color_space(&self, _inp: &StageInputs) -> ColorSpace {
        ColorSpace::GammaRec2020
    }

    fn output_color_space(&self, _inp: &StageInputs) -> ColorSpace {
        ColorSpace::GammaRec2020
    }

    fn is_active(&self, inp: &StageInputs) -> bool {
        !inp.params.vignette.is_default()
    }

    fn prepare(&mut self, _inp: &StageInputs) {}

    fn process(&self, ctx: &mut RenderContext) -> Result<(), AgxError> {
        let pre = adjust::VignettePrecomputed::new(
            ctx.params.vignette.amount,
            ctx.params.vignette.shape,
            ctx.width,
            ctx.height,
        );
        adjust::apply_vignette_buffer(&mut ctx.buf, ctx.width, ctx.height, &pre);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::Parameters;

    #[test]
    fn vignette_inactive_when_neutral() {
        let params = Parameters::default();
        let inp = crate::engine::StageInputs { params: &params, lut: None };
        let stage = VignetteStage::new();
        assert!(!stage.is_active(&inp));
    }

    #[test]
    fn vignette_active_when_nonzero() {
        let mut params = Parameters::default();
        params.vignette.amount = -50.0;
        let inp = crate::engine::StageInputs { params: &params, lut: None };
        let stage = VignetteStage::new();
        assert!(stage.is_active(&inp));
    }

    #[test]
    fn vignette_color_space_is_srgb() {
        let params = Parameters::default();
        let inp = crate::engine::StageInputs { params: &params, lut: None };
        let stage = VignetteStage::new();
        assert_eq!(stage.input_color_space(&inp), ColorSpace::GammaRec2020);
        assert_eq!(stage.output_color_space(&inp), ColorSpace::GammaRec2020);
    }
}
