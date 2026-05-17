//! Color space conversion GPU dispatchers.

use crate::engine::gpu::runtime::GpuRuntime;

/// Dispatch the linear-to-gamma compute shader.
///
/// Applies the sign-preserving sRGB transfer curve per channel to working-space
/// (linear Rec.2020) pixels, producing gamma Rec.2020 values. The sign-preserving
/// variant lets negative components from out-of-gamut math survive the transfer.
pub fn dispatch_linear_to_gamma(runtime: &GpuRuntime, pipeline: &wgpu::ComputePipeline) {
    dispatch_pixel_only(runtime, pipeline, "linear_to_gamma");
}

/// Dispatch the gamma-to-linear compute shader.
///
/// Inverse of [`dispatch_linear_to_gamma`]: applies the sign-preserving sRGB
/// inverse transfer per channel to bring gamma Rec.2020 pixels back to linear
/// Rec.2020 for further scene-referred processing.
pub fn dispatch_gamma_to_linear(runtime: &GpuRuntime, pipeline: &wgpu::ComputePipeline) {
    dispatch_pixel_only(runtime, pipeline, "gamma_to_linear");
}

/// Shared dispatch for shaders that only bind the pixel buffer.
fn dispatch_pixel_only(runtime: &GpuRuntime, pipeline: &wgpu::ComputePipeline, label: &str) {
    let bind_group_layout = pipeline.get_bind_group_layout(0);
    let bind_group = runtime
        .device
        .create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(label),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: runtime.pixel_buffer.as_entire_binding(),
            }],
        });

    let wg = runtime.workgroup_counts();
    let mut encoder = runtime
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some(label) });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some(label),
            timestamp_writes: None,
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(wg.0, wg.1, 1);
    }
    runtime.queue.submit(std::iter::once(encoder.finish()));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::gpu::runtime::{gpu_available, GpuRuntime};
    use crate::engine::gpu::shaders::ShaderCache;

    #[test]
    fn gpu_linear_gamma_roundtrip() {
        if !gpu_available() {
            eprintln!("skipping: no GPU adapter");
            return;
        }
        // Use a larger image to catch alignment/stride bugs
        let runtime = GpuRuntime::new(4, 2).unwrap();
        let shaders = ShaderCache::new(&runtime.device).unwrap();

        // Range spans negative values to exercise the sign-preserving curve.
        let pixels: Vec<[f32; 3]> = (0..8)
            .map(|i| {
                let t = (i as f32 / 7.0) * 2.0 - 0.3; // range roughly -0.3 .. 1.7
                [t, 1.0 - t, 0.5 * t]
            })
            .collect();
        runtime.upload_pixels(&pixels);

        let to_gamma = shaders.get("linear_to_gamma").unwrap();
        dispatch_linear_to_gamma(&runtime, to_gamma);

        let to_linear = shaders.get("gamma_to_linear").unwrap();
        dispatch_gamma_to_linear(&runtime, to_linear);

        let result = runtime.download_pixels();
        assert_eq!(result.len(), pixels.len());
        for (i, (a, b)) in pixels.iter().zip(result.iter()).enumerate() {
            for c in 0..3 {
                assert!(
                    (a[c] - b[c]).abs() < 1e-4,
                    "pixel[{i}][{c}]: expected {}, got {} (diff {})",
                    a[c],
                    b[c],
                    (a[c] - b[c]).abs()
                );
            }
        }
    }
}
