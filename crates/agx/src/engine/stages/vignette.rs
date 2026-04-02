use crate::adjust;
use crate::engine::{ColorSpace, Parameters, RenderContext, Stage};
use crate::error::AgxError;

/// Position-dependent edge darkening/brightening.
/// Operates in sRGB gamma space.
pub struct VignetteStage;

impl Default for VignetteStage {
    fn default() -> Self {
        Self
    }
}

impl VignetteStage {
    pub fn new() -> Self {
        Self
    }
}

impl Stage for VignetteStage {
    fn name(&self) -> &'static str {
        "vignette"
    }

    fn input_color_space(&self) -> ColorSpace {
        ColorSpace::SrgbGamma
    }

    fn output_color_space(&self) -> ColorSpace {
        ColorSpace::SrgbGamma
    }

    fn is_active(&self, params: &Parameters) -> bool {
        !params.vignette.is_default()
    }

    fn prepare(&mut self, _params: &Parameters) {}

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
        let stage = VignetteStage::new();
        assert!(!stage.is_active(&params));
    }

    #[test]
    fn vignette_active_when_nonzero() {
        let mut params = Parameters::default();
        params.vignette.amount = -50.0;
        let stage = VignetteStage::new();
        assert!(stage.is_active(&params));
    }

    #[test]
    fn vignette_color_space_is_srgb() {
        let stage = VignetteStage::new();
        assert_eq!(stage.input_color_space(), ColorSpace::SrgbGamma);
        assert_eq!(stage.output_color_space(), ColorSpace::SrgbGamma);
    }
}
