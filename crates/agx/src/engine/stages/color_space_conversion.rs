use crate::adjust;
use crate::engine::{ColorSpace, RenderContext, Stage};
use crate::error::AgxError;

/// Converts the buffer from linear sRGB to sRGB gamma space.
pub struct LinearToSrgbStage;

impl Default for LinearToSrgbStage {
    fn default() -> Self {
        Self
    }
}

impl LinearToSrgbStage {
    /// Create a new linear-to-sRGB conversion stage.
    pub fn new() -> Self {
        Self
    }
}

impl Stage for LinearToSrgbStage {
    fn name(&self) -> &'static str {
        "linear_to_srgb"
    }

    fn input_color_space(&self) -> ColorSpace {
        ColorSpace::LinearSrgb
    }

    fn output_color_space(&self) -> ColorSpace {
        ColorSpace::SrgbGamma
    }

    fn is_active(&self, _params: &crate::engine::Parameters) -> bool {
        true
    }

    fn prepare(&mut self, _params: &crate::engine::Parameters) {}

    fn process(&self, ctx: &mut RenderContext) -> Result<(), AgxError> {
        for pixel in ctx.buf.iter_mut() {
            let (r, g, b) = adjust::linear_to_srgb(pixel[0], pixel[1], pixel[2]);
            *pixel = [r, g, b];
        }
        Ok(())
    }
}

/// Converts the buffer from sRGB gamma space to linear sRGB.
pub struct SrgbToLinearStage;

impl Default for SrgbToLinearStage {
    fn default() -> Self {
        Self
    }
}

impl SrgbToLinearStage {
    /// Create a new sRGB-to-linear conversion stage.
    pub fn new() -> Self {
        Self
    }
}

impl Stage for SrgbToLinearStage {
    fn name(&self) -> &'static str {
        "srgb_to_linear"
    }

    fn input_color_space(&self) -> ColorSpace {
        ColorSpace::SrgbGamma
    }

    fn output_color_space(&self) -> ColorSpace {
        ColorSpace::LinearSrgb
    }

    fn is_active(&self, _params: &crate::engine::Parameters) -> bool {
        true
    }

    fn prepare(&mut self, _params: &crate::engine::Parameters) {}

    fn process(&self, ctx: &mut RenderContext) -> Result<(), AgxError> {
        for pixel in ctx.buf.iter_mut() {
            let (r, g, b) = adjust::srgb_to_linear(pixel[0], pixel[1], pixel[2]);
            *pixel = [r, g, b];
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::Parameters;

    #[test]
    fn linear_to_srgb_roundtrip() {
        let params = Parameters::default();
        let pixels = vec![[0.5, 0.3, 0.1], [0.0, 1.0, 0.25]];
        let mut ctx = RenderContext {
            buf: pixels.clone(),
            width: 2,
            height: 1,
            params: &params,
            lut: None,
        };

        let mut to_srgb = LinearToSrgbStage::new();
        to_srgb.prepare(&params);
        to_srgb.process(&mut ctx).unwrap();

        assert!(
            (ctx.buf[0][0] - 0.5).abs() > 0.01,
            "gamma encoding should change 0.5"
        );

        let mut to_linear = SrgbToLinearStage::new();
        to_linear.prepare(&params);
        to_linear.process(&mut ctx).unwrap();

        for (i, pixel) in ctx.buf.iter().enumerate() {
            for (c, &v) in pixel.iter().enumerate() {
                assert!(
                    (v - pixels[i][c]).abs() < 1e-5,
                    "pixel[{i}][{c}]: expected {}, got {v}",
                    pixels[i][c]
                );
            }
        }
    }

    #[test]
    fn linear_to_srgb_always_active() {
        let params = Parameters::default();
        let stage = LinearToSrgbStage::new();
        assert!(stage.is_active(&params));
    }

    #[test]
    fn srgb_to_linear_always_active() {
        let params = Parameters::default();
        let stage = SrgbToLinearStage::new();
        assert!(stage.is_active(&params));
    }

    #[test]
    fn color_space_declarations_correct() {
        let to_srgb = LinearToSrgbStage::new();
        assert_eq!(to_srgb.input_color_space(), ColorSpace::LinearSrgb);
        assert_eq!(to_srgb.output_color_space(), ColorSpace::SrgbGamma);

        let to_linear = SrgbToLinearStage::new();
        assert_eq!(to_linear.input_color_space(), ColorSpace::SrgbGamma);
        assert_eq!(to_linear.output_color_space(), ColorSpace::LinearSrgb);
    }
}
