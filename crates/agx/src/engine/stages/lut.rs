use crate::engine::{ColorSpace, RenderContext, Stage, StageInputs};
use crate::error::AgxError;
use crate::lut::LutEncoding;
use rayon::prelude::*;

/// Samples the active 3D LUT. The LUT declares the color space it was authored
/// in; the executor converts the buffer into that space before this stage and
/// back afterward, so `process` samples directly with no internal bracket.
pub struct LutStage;

impl Default for LutStage {
    fn default() -> Self {
        Self
    }
}

impl LutStage {
    /// Create a new LUT stage.
    pub fn new() -> Self {
        Self
    }
}

/// Map a LUT's authoring encoding to the pipeline color space it samples in.
fn encoding_space(inp: &StageInputs) -> ColorSpace {
    match inp.lut.map(|l| l.encoding) {
        Some(LutEncoding::Linear) => ColorSpace::LinearSrgb,
        // Srgb encoding, or no LUT. The executor never calls this stage when inactive
        // (is_active returns false), so the no-LUT arm is only a sane fallback default.
        _ => ColorSpace::SrgbGamma,
    }
}

impl Stage for LutStage {
    fn name(&self) -> &'static str {
        "lut"
    }

    fn input_color_space(&self, inp: &StageInputs) -> ColorSpace {
        encoding_space(inp)
    }

    fn output_color_space(&self, inp: &StageInputs) -> ColorSpace {
        encoding_space(inp)
    }

    fn is_active(&self, inp: &StageInputs) -> bool {
        inp.lut.is_some()
    }

    fn prepare(&mut self, _inp: &StageInputs) {}

    fn process(&self, ctx: &mut RenderContext) -> Result<(), AgxError> {
        let Some(lut) = ctx.lut else {
            return Ok(());
        };
        ctx.buf.par_iter_mut().for_each(|px| {
            let (r, g, b) = lut.lookup(px[0], px[1], px[2]);
            *px = [r, g, b];
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::Parameters;
    use crate::lut::Lut3D;

    fn identity_lut(encoding: LutEncoding) -> Lut3D {
        let size = 2;
        let mut table = Vec::with_capacity(size * size * size);
        for b in 0..size {
            for g in 0..size {
                for r in 0..size {
                    let d = (size - 1) as f32;
                    table.push([r as f32 / d, g as f32 / d, b as f32 / d]);
                }
            }
        }
        Lut3D {
            title: None,
            size,
            domain_min: [0.0; 3],
            domain_max: [1.0; 3],
            table,
            encoding,
        }
    }

    #[test]
    fn inactive_without_lut() {
        let params = Parameters::default();
        let inp = StageInputs {
            params: &params,
            lut: None,
        };
        assert!(!LutStage::new().is_active(&inp));
    }

    #[test]
    fn space_follows_encoding() {
        let params = Parameters::default();
        let srgb = identity_lut(LutEncoding::Srgb);
        let lin = identity_lut(LutEncoding::Linear);
        let stage = LutStage::new();
        assert_eq!(
            stage.input_color_space(&StageInputs {
                params: &params,
                lut: Some(&srgb)
            }),
            ColorSpace::SrgbGamma
        );
        assert_eq!(
            stage.input_color_space(&StageInputs {
                params: &params,
                lut: Some(&lin)
            }),
            ColorSpace::LinearSrgb
        );
    }
}
