//! GPU render pipeline using wgpu + WGSL compute shaders.

pub mod params;
pub mod runtime;
pub mod shaders;
pub mod stages;

use image::Rgb32FImage;

use crate::error::AgxError;

use super::{Parameters, RenderResult};

/// GPU render pipeline. Owns the wgpu runtime and compiled shader cache.
///
/// Created once per [`Engine`](super::Engine) and reused across renders.
/// Each `execute()` call uploads the pixel buffer, dispatches compute stages
/// in the same fixed order as the CPU pipeline, then downloads the result.
pub struct GpuPipeline {
    runtime: runtime::GpuRuntime,
    shaders: shaders::ShaderCache,
}

impl GpuPipeline {
    /// Create a new GPU pipeline for images of the given dimensions.
    pub fn new(width: u32, height: u32) -> Result<Self, AgxError> {
        let runtime = runtime::GpuRuntime::new(width, height)?;
        let shaders = shaders::ShaderCache::new(&runtime.device)?;
        Ok(Self { runtime, shaders })
    }

    /// Execute the full GPU render pipeline.
    ///
    /// Stage order mirrors [`CpuPipeline`](super::pipeline::CpuPipeline):
    /// 1. Linear adjustments (white balance + exposure)
    /// 2. Dehaze
    /// 3. Denoise
    /// 4. Linear → sRGB conversion
    /// 5. Gamma adjustments (contrast, HSL, tone curves, color grading, LUT)
    /// 6. Detail (texture, clarity, sharpening)
    /// 7. Grain
    /// 8. Vignette
    /// 9. sRGB → Linear conversion
    pub fn execute(
        &mut self,
        original: &Rgb32FImage,
        params: &Parameters,
        lut: Option<&crate::lut::Lut3D>,
    ) -> RenderResult {
        let (w, h) = original.dimensions();
        let buf: Vec<[f32; 3]> = original.pixels().map(|p| [p.0[0], p.0[1], p.0[2]]).collect();

        // Upload pixels and params
        self.runtime.upload_pixels(&buf);
        let mut gpu_params = params::GpuParameters::from(params);
        gpu_params.width = w as f32;
        gpu_params.height = h as f32;
        self.runtime.upload_params(&gpu_params);

        // Upload tone curves
        let tc_data = params::build_tone_curve_data(params);
        self.runtime.upload_tone_curves(&tc_data);

        // Upload LUT if present
        if let Some(lut) = lut {
            self.runtime.upload_lut(lut);
        }

        // 1. Linear adjustments (white balance + exposure)
        if params.temperature != 0.0 || params.tint != 0.0 || params.exposure != 0.0 {
            stages::linear_adjustments::dispatch_linear_adjustments(
                &self.runtime,
                self.shaders.get("linear_adjustments").expect("linear_adjustments"),
            );
        }

        // 2. Dehaze
        if !params.dehaze.is_neutral() {
            stages::dehaze::dispatch_dehaze(
                &self.runtime,
                &self.shaders,
                &mut gpu_params,
                &params.dehaze,
            );
        }

        // 3. Denoise
        if !params.noise_reduction.is_neutral() {
            stages::denoise::dispatch_denoise(
                &self.runtime,
                &self.shaders,
                &mut gpu_params,
                &params.noise_reduction,
            );
        }

        // 4. Linear → sRGB
        stages::color_space::dispatch_linear_to_srgb(
            &self.runtime,
            self.shaders.get("linear_to_srgb").expect("linear_to_srgb"),
        );

        // 5. Gamma adjustments (always active)
        stages::gamma_adjustments::dispatch_gamma_adjustments(
            &self.runtime,
            self.shaders.get("gamma_adjustments").expect("gamma_adjustments"),
        );

        // 6. Detail
        if !params.detail.is_neutral() {
            stages::detail::dispatch_detail(
                &self.runtime,
                &self.shaders,
                &mut gpu_params,
                &params.detail,
            );
        }

        // 7. Grain
        if !params.grain.is_neutral() {
            stages::grain::dispatch_grain(
                &self.runtime,
                &self.shaders,
                &mut gpu_params,
                &params.grain,
            );
        }

        // 8. Vignette
        if !params.vignette.is_default() {
            // Re-upload params in case detail/grain/denoise/dehaze mutated them
            gpu_params = params::GpuParameters::from(params);
            gpu_params.width = w as f32;
            gpu_params.height = h as f32;
            self.runtime.upload_params(&gpu_params);
            stages::vignette::dispatch_vignette(
                &self.runtime,
                self.shaders.get("vignette").expect("vignette"),
            );
        }

        // 9. sRGB → Linear
        stages::color_space::dispatch_srgb_to_linear(
            &self.runtime,
            self.shaders.get("srgb_to_linear").expect("srgb_to_linear"),
        );

        // Download result
        let result_buf = self.runtime.download_pixels();
        let image = Rgb32FImage::from_fn(w, h, |x, y| {
            let idx = (y * w + x) as usize;
            image::Rgb(result_buf[idx])
        });

        RenderResult {
            image,
            #[cfg(feature = "profiling")]
            profile: None, // GPU profiling is a follow-up
        }
    }
}
