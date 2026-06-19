use crate::adjust;
use crate::engine::{ColorSpace, RenderContext, Stage, StageInputs};
use crate::error::AgxError;

/// Sharpening, clarity, and texture via Gaussian blur + unsharp mask.
/// Operates in the gamma Rec.2020 working space as a buffer-level pass.
pub struct DetailStage;

impl Default for DetailStage {
    fn default() -> Self {
        Self
    }
}

impl DetailStage {
    /// Create a new detail stage.
    pub fn new() -> Self {
        Self
    }
}

impl Stage for DetailStage {
    fn name(&self) -> &'static str {
        "detail"
    }

    fn input_color_space(&self, _inp: &StageInputs) -> ColorSpace {
        ColorSpace::GammaRec2020
    }

    fn output_color_space(&self, _inp: &StageInputs) -> ColorSpace {
        ColorSpace::GammaRec2020
    }

    fn is_active(&self, inp: &StageInputs) -> bool {
        !inp.params.detail.is_neutral()
    }

    fn prepare(&mut self, _inp: &StageInputs) {}

    fn process(&self, ctx: &mut RenderContext) -> Result<(), AgxError> {
        let result = adjust::detail::apply_detail_pass(
            &ctx.buf,
            ctx.width as usize,
            ctx.height as usize,
            &ctx.params.detail,
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
    fn detail_inactive_when_neutral() {
        let params = Parameters::default();
        let inp = crate::engine::StageInputs {
            params: &params,
            lut: None,
        };
        let stage = DetailStage::new();
        assert!(!stage.is_active(&inp));
    }

    #[test]
    fn detail_active_when_sharpening() {
        let mut params = Parameters::default();
        params.detail.sharpening.amount = 50.0;
        let inp = crate::engine::StageInputs {
            params: &params,
            lut: None,
        };
        let stage = DetailStage::new();
        assert!(stage.is_active(&inp));
    }

    #[test]
    fn detail_color_space_is_srgb() {
        let params = Parameters::default();
        let inp = crate::engine::StageInputs {
            params: &params,
            lut: None,
        };
        let stage = DetailStage::new();
        assert_eq!(stage.input_color_space(&inp), ColorSpace::GammaRec2020);
        assert_eq!(stage.output_color_space(&inp), ColorSpace::GammaRec2020);
    }
}
