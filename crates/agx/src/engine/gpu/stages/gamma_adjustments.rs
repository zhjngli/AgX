//! Gamma-space tone adjustment GPU dispatcher (contrast, highlights, shadows, whites, blacks).

use super::linear_adjustments::dispatch_with_params;
use crate::engine::gpu::runtime::GpuRuntime;

/// Dispatch the gamma-adjustments compute shader.
pub fn dispatch_gamma_adjustments(runtime: &GpuRuntime, pipeline: &wgpu::ComputePipeline) {
    dispatch_with_params(runtime, pipeline, "gamma_adjustments");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::gpu::params::GpuParameters;
    use crate::engine::gpu::shaders::ShaderCache;
    use crate::engine::gpu::stages::color_space::dispatch_linear_to_srgb;
    use crate::engine::Parameters;

    fn gpu_available() -> bool {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .is_some()
    }

    #[test]
    fn gpu_contrast_shifts_midtones() {
        if !gpu_available() {
            eprintln!("skipping: no GPU adapter");
            return;
        }
        let runtime = GpuRuntime::new(2, 1).unwrap();
        let shaders = ShaderCache::new(&runtime.device).unwrap();

        // Linear-space mid-grey pixels
        let pixels = vec![[0.2, 0.2, 0.2]; 2];
        runtime.upload_pixels(&pixels);

        // Convert to sRGB first (gamma adjustments run in gamma space)
        let to_srgb = shaders.get("linear_to_srgb").unwrap();
        dispatch_linear_to_srgb(&runtime, to_srgb);
        let srgb_before = runtime.download_pixels();

        // Apply contrast
        let mut params = Parameters::default();
        params.contrast = 50.0;
        let mut gpu_params: GpuParameters = (&params).into();
        gpu_params.width = 2.0;
        gpu_params.height = 1.0;
        runtime.upload_params(&gpu_params);

        let pipeline = shaders.get("gamma_adjustments").unwrap();
        dispatch_gamma_adjustments(&runtime, pipeline);

        let result = runtime.download_pixels();
        // With positive contrast, values below 0.5 should decrease, above 0.5 should increase
        // Our test pixel (0.2 linear → ~0.48 sRGB) is near midpoint, so change should be modest
        // but nonzero
        for (i, (before, after)) in srgb_before.iter().zip(result.iter()).enumerate() {
            let changed = (0..3).any(|c| (before[c] - after[c]).abs() > 1e-6);
            assert!(
                changed,
                "pixel[{i}] unchanged: before={:?}, after={:?}",
                before, after
            );
        }
    }

    #[test]
    fn gpu_gamma_neutral_is_identity() {
        if !gpu_available() {
            eprintln!("skipping: no GPU adapter");
            return;
        }
        let runtime = GpuRuntime::new(2, 2).unwrap();
        let shaders = ShaderCache::new(&runtime.device).unwrap();

        // Already in sRGB/gamma space values
        let pixels = vec![[0.3, 0.5, 0.7], [0.1, 0.9, 0.4], [0.6, 0.2, 0.8], [0.0, 1.0, 0.5]];
        runtime.upload_pixels(&pixels);

        let params = Parameters::default();
        let mut gpu_params: GpuParameters = (&params).into();
        gpu_params.width = 2.0;
        gpu_params.height = 2.0;
        runtime.upload_params(&gpu_params);

        let pipeline = shaders.get("gamma_adjustments").unwrap();
        dispatch_gamma_adjustments(&runtime, pipeline);

        let result = runtime.download_pixels();
        for (i, (a, b)) in pixels.iter().zip(result.iter()).enumerate() {
            for c in 0..3 {
                assert!(
                    (a[c] - b[c]).abs() < 1e-6,
                    "pixel[{i}][{c}]: expected {}, got {} (diff {})",
                    a[c],
                    b[c],
                    (a[c] - b[c]).abs()
                );
            }
        }
    }
}
