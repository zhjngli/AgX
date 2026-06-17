use crate::adjust;
use crate::engine::{ColorSpace, RenderContext, Stage, StageInputs};
use crate::error::AgxError;

/// Removes atmospheric haze using dark channel prior. Operates in linear space.
pub struct DehazeStage;

impl Default for DehazeStage {
    fn default() -> Self {
        Self
    }
}

impl DehazeStage {
    /// Create a new dehaze stage.
    pub fn new() -> Self {
        Self
    }
}

impl Stage for DehazeStage {
    fn name(&self) -> &'static str {
        "dehaze"
    }

    fn input_color_space(&self, _inp: &StageInputs) -> ColorSpace {
        ColorSpace::LinearRec2020
    }

    fn output_color_space(&self, _inp: &StageInputs) -> ColorSpace {
        ColorSpace::LinearRec2020
    }

    fn is_active(&self, inp: &StageInputs) -> bool {
        !inp.params.dehaze.is_neutral()
    }

    fn prepare(&mut self, _inp: &StageInputs) {}

    fn process(&self, ctx: &mut RenderContext) -> Result<(), AgxError> {
        let result = adjust::dehaze::apply_dehaze(
            &ctx.buf,
            ctx.width as usize,
            ctx.height as usize,
            &ctx.params.dehaze,
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
    fn dehaze_inactive_when_neutral() {
        let params = Parameters::default();
        let inp = crate::engine::StageInputs {
            params: &params,
            lut: None,
        };
        let stage = DehazeStage::new();
        assert!(!stage.is_active(&inp));
    }

    #[test]
    fn dehaze_active_when_nonzero() {
        let mut params = Parameters::default();
        params.dehaze.amount = 50.0;
        let inp = crate::engine::StageInputs {
            params: &params,
            lut: None,
        };
        let stage = DehazeStage::new();
        assert!(stage.is_active(&inp));
    }

    #[test]
    fn dehaze_color_space_is_linear() {
        let params = Parameters::default();
        let inp = crate::engine::StageInputs {
            params: &params,
            lut: None,
        };
        let stage = DehazeStage::new();
        assert_eq!(stage.input_color_space(&inp), ColorSpace::LinearRec2020);
        assert_eq!(stage.output_color_space(&inp), ColorSpace::LinearRec2020);
    }
}
