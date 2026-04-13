//! Color space conversion GPU dispatchers.

use crate::engine::gpu::runtime::GpuRuntime;

/// Dispatch the linear-to-sRGB compute shader.
pub fn dispatch_linear_to_srgb(runtime: &GpuRuntime, pipeline: &wgpu::ComputePipeline) {
    dispatch_pixel_only(runtime, pipeline, "linear_to_srgb");
}

/// Dispatch the sRGB-to-linear compute shader.
pub fn dispatch_srgb_to_linear(runtime: &GpuRuntime, pipeline: &wgpu::ComputePipeline) {
    dispatch_pixel_only(runtime, pipeline, "srgb_to_linear");
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

    let workgroup_count = (runtime.pixel_count() + 255) / 256;
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
        pass.dispatch_workgroups(workgroup_count, 1, 1);
    }
    runtime.queue.submit(std::iter::once(encoder.finish()));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::gpu::runtime::GpuRuntime;
    use crate::engine::gpu::shaders::ShaderCache;

    fn gpu_available() -> bool {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .is_some()
    }

    #[test]
    fn gpu_linear_srgb_roundtrip() {
        if !gpu_available() {
            eprintln!("skipping: no GPU adapter");
            return;
        }
        // Use a larger image to catch alignment/stride bugs
        let runtime = GpuRuntime::new(4, 2).unwrap();
        let shaders = ShaderCache::new(&runtime.device).unwrap();

        let pixels: Vec<[f32; 3]> = (0..8)
            .map(|i| {
                let t = i as f32 / 7.0;
                [t, 1.0 - t, 0.5 * t]
            })
            .collect();
        runtime.upload_pixels(&pixels);

        let to_srgb = shaders.get("linear_to_srgb").unwrap();
        dispatch_linear_to_srgb(&runtime, to_srgb);

        let to_linear = shaders.get("srgb_to_linear").unwrap();
        dispatch_srgb_to_linear(&runtime, to_linear);

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
