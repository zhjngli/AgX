//! Vignette GPU dispatcher (darkens/brightens image edges by distance from center).

use crate::engine::gpu::runtime::GpuRuntime;
use crate::engine::gpu::stages::linear_adjustments::dispatch_with_params;

/// Dispatch the vignette compute shader.
pub fn dispatch_vignette(runtime: &GpuRuntime, pipeline: &wgpu::ComputePipeline) {
    dispatch_with_params(runtime, pipeline, "vignette");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adjust::{self, VignetteShape};
    use crate::engine::gpu::params::GpuParameters;
    use crate::engine::gpu::shaders::ShaderCache;
    use crate::engine::Parameters;

    fn gpu_available() -> bool {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .is_some()
    }

    #[test]
    fn gpu_vignette_matches_cpu() {
        if !gpu_available() {
            eprintln!("skipping: no GPU adapter");
            return;
        }

        // 4x4 test image in sRGB/gamma space (vignette runs in gamma space)
        let pixels: Vec<[f32; 3]> = (0..16)
            .map(|i| {
                let t = i as f32 / 15.0;
                [t, 1.0 - t, 0.5]
            })
            .collect();

        // CPU path
        let mut cpu_buf = pixels.clone();
        let pre = adjust::VignettePrecomputed::new(-50.0, VignetteShape::Elliptical, 4, 4);
        adjust::apply_vignette_buffer(&mut cpu_buf, 4, 4, &pre);

        // GPU path
        let runtime = GpuRuntime::new(4, 4).unwrap();
        let shaders = ShaderCache::new(&runtime.device).unwrap();
        runtime.upload_pixels(&pixels);

        let params = Parameters::default();
        let mut gpu_params: GpuParameters = (&params).into();
        gpu_params.vignette_amount = -50.0;
        gpu_params.vignette_shape = 0.0; // Elliptical
        gpu_params.width = 4.0;
        gpu_params.height = 4.0;
        runtime.upload_params(&gpu_params);

        let pipeline = shaders.get("vignette").unwrap();
        dispatch_vignette(&runtime, pipeline);
        let gpu_buf = runtime.download_pixels();

        for (i, (cpu, gpu)) in cpu_buf.iter().zip(gpu_buf.iter()).enumerate() {
            for c in 0..3 {
                assert!(
                    (cpu[c] - gpu[c]).abs() < 1e-4,
                    "pixel[{i}][{c}]: cpu={}, gpu={} (diff={})",
                    cpu[c],
                    gpu[c],
                    (cpu[c] - gpu[c]).abs()
                );
            }
        }
    }

    #[test]
    fn gpu_vignette_zero_amount_is_identity() {
        if !gpu_available() {
            eprintln!("skipping: no GPU adapter");
            return;
        }

        let pixels: Vec<[f32; 3]> = (0..16)
            .map(|i| {
                let t = i as f32 / 15.0;
                [t, 1.0 - t, 0.5]
            })
            .collect();

        let runtime = GpuRuntime::new(4, 4).unwrap();
        let shaders = ShaderCache::new(&runtime.device).unwrap();
        runtime.upload_pixels(&pixels);

        let params = Parameters::default();
        let mut gpu_params: GpuParameters = (&params).into();
        gpu_params.vignette_amount = 0.0;
        gpu_params.vignette_shape = 0.0;
        gpu_params.width = 4.0;
        gpu_params.height = 4.0;
        runtime.upload_params(&gpu_params);

        let pipeline = shaders.get("vignette").unwrap();
        dispatch_vignette(&runtime, pipeline);
        let result = runtime.download_pixels();

        for (i, (orig, out)) in pixels.iter().zip(result.iter()).enumerate() {
            for c in 0..3 {
                assert!(
                    (orig[c] - out[c]).abs() < 1e-6,
                    "pixel[{i}][{c}]: expected {}, got {} (diff={})",
                    orig[c],
                    out[c],
                    (orig[c] - out[c]).abs()
                );
            }
        }
    }
}
