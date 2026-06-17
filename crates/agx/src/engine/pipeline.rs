use crate::color_space::convert_buffer;
use image::{Rgb, Rgb32FImage};

use super::stages;
use super::ColorSpace;
use super::{Parameters, RenderContext, RenderResult, Stage, StageInputs};

#[cfg(feature = "profiling")]
use super::RenderProfile;

/// CPU render pipeline using rayon for parallelism.
/// Executes stages sequentially on a CPU pixel buffer.
pub struct CpuPipeline {
    stages: Vec<Box<dyn Stage>>,
}

impl Default for CpuPipeline {
    fn default() -> Self {
        Self::new()
    }
}

impl CpuPipeline {
    /// Construct the fixed pipeline with all stages in render order.
    pub fn new() -> Self {
        CpuPipeline {
            stages: vec![
                Box::new(stages::WhiteBalanceExposureStage::new()),
                Box::new(stages::DehazeStage::new()),
                Box::new(stages::DenoiseStage::new()),
                Box::new(stages::PerPixelAdjustmentsStage::new()),
                Box::new(stages::LutStage::new()),
                Box::new(stages::DetailStage::new()),
                Box::new(stages::GrainStage::new()),
                Box::new(stages::VignetteStage::new()),
            ],
        }
    }

    /// Execute the pipeline, returning the rendered image and optional profiling data.
    pub fn execute(
        &mut self,
        original: &Rgb32FImage,
        params: &Parameters,
        lut: Option<&crate::lut::Lut3D>,
    ) -> RenderResult {
        let (w, h) = original.dimensions();

        #[cfg(feature = "profiling")]
        let render_start = std::time::Instant::now();
        #[cfg(feature = "profiling")]
        let mut profile_stages: Vec<(String, f64)> = Vec::new();

        let buf: Vec<[f32; 3]> = original
            .pixels()
            .map(|p| [p.0[0], p.0[1], p.0[2]])
            .collect();

        let mut ctx = RenderContext {
            buf,
            width: w,
            height: h,
            params,
            lut,
        };

        let inputs = StageInputs { params, lut };

        // Prepare all active stages
        for stage in &mut self.stages {
            if stage.is_active(&inputs) {
                stage.prepare(&inputs);
            }
        }

        // Execute stages in order, auto-inserting color-space conversions when
        // the next active stage declares a different input space than the current
        // buffer state. The buffer enters as LinearRec2020 (engine input contract).
        let mut current = ColorSpace::LinearRec2020;

        for stage in &self.stages {
            if !stage.is_active(&inputs) {
                continue;
            }

            let needed = stage.input_color_space(&inputs);
            if current != needed {
                #[cfg(feature = "profiling")]
                let conv_start = std::time::Instant::now();

                convert_buffer(&mut ctx.buf, current, needed);

                #[cfg(feature = "profiling")]
                profile_stages.push((
                    format!("convert: {needed:?}"),
                    conv_start.elapsed().as_secs_f64() * 1000.0,
                ));
            }

            #[cfg(feature = "profiling")]
            let stage_start = std::time::Instant::now();

            stage
                .process(&mut ctx)
                .expect("stage processing should not fail");

            current = stage.output_color_space(&inputs);

            #[cfg(feature = "profiling")]
            profile_stages.push((
                stage.name().to_string(),
                stage_start.elapsed().as_secs_f64() * 1000.0,
            ));
        }

        // Engine output contract: linear Rec.2020.
        if current != ColorSpace::LinearRec2020 {
            #[cfg(feature = "profiling")]
            let conv_start = std::time::Instant::now();

            convert_buffer(&mut ctx.buf, current, ColorSpace::LinearRec2020);

            #[cfg(feature = "profiling")]
            profile_stages.push((
                "convert: LinearRec2020".to_string(),
                conv_start.elapsed().as_secs_f64() * 1000.0,
            ));
        }

        let image = Rgb32FImage::from_fn(w, h, |x, y| {
            let idx = (y * w + x) as usize;
            Rgb(ctx.buf[idx])
        });

        RenderResult {
            image,
            #[cfg(feature = "profiling")]
            profile: Some(RenderProfile {
                stages: profile_stages,
                total_ms: render_start.elapsed().as_secs_f64() * 1000.0,
            }),
        }
    }
}

#[cfg(test)]
mod auto_insert_tests {
    use super::*;
    use crate::engine::Parameters;
    use image::{ImageBuffer, Rgb};

    #[test]
    fn neutral_render_is_identity_through_auto_insert() {
        let img = ImageBuffer::from_pixel(2, 2, Rgb([0.4_f32, 0.6, 0.2]));
        let original: Vec<[f32; 3]> = img.pixels().map(|p| p.0).collect();
        let mut pipeline = CpuPipeline::new();
        let params = Parameters::default();
        let out = pipeline.execute(&img, &params, None).image;
        for (i, p) in out.pixels().enumerate() {
            for (c, (&actual, &orig)) in p.0.iter().zip(original[i].iter()).enumerate() {
                assert!(
                    (actual - orig).abs() < 1e-5,
                    "neutral render changed pixel [{i}][{c}]"
                );
            }
        }
    }
}
