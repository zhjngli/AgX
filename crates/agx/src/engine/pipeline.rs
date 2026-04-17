use image::{Rgb, Rgb32FImage};

use super::stages;
use super::{Parameters, RenderContext, RenderResult, Stage};
#[cfg(debug_assertions)]
use super::ColorSpace;

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
                Box::new(stages::LinearToSrgbStage::new()),
                Box::new(stages::PerPixelAdjustmentsStage::new()),
                Box::new(stages::DetailStage::new()),
                Box::new(stages::GrainStage::new()),
                Box::new(stages::VignetteStage::new()),
                Box::new(stages::SrgbToLinearStage::new()),
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

        // Prepare all active stages
        for stage in &mut self.stages {
            if stage.is_active(params) {
                stage.prepare(params);
            }
        }

        // Execute stages in order, tracking color space in debug builds
        #[cfg(debug_assertions)]
        let mut current_color_space = ColorSpace::LinearSrgb;

        for stage in &self.stages {
            if !stage.is_active(params) {
                continue;
            }

            #[cfg(debug_assertions)]
            debug_assert_eq!(
                stage.input_color_space(),
                current_color_space,
                "stage '{}' expects {:?} but current space is {:?}",
                stage.name(),
                stage.input_color_space(),
                current_color_space,
            );

            #[cfg(feature = "profiling")]
            let stage_start = std::time::Instant::now();

            stage
                .process(&mut ctx)
                .expect("stage processing should not fail");

            #[cfg(debug_assertions)]
            {
                current_color_space = stage.output_color_space();
            }

            #[cfg(feature = "profiling")]
            profile_stages.push((
                stage.name().to_string(),
                stage_start.elapsed().as_secs_f64() * 1000.0,
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
