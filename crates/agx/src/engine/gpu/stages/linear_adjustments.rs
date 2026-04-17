//! Linear-space adjustment GPU dispatcher (white balance + exposure).

use crate::engine::gpu::runtime::GpuRuntime;

/// Dispatch the linear-adjustments compute shader.
pub fn dispatch_linear_adjustments(runtime: &GpuRuntime, pipeline: &wgpu::ComputePipeline) {
    dispatch_with_params(runtime, pipeline, "linear_adjustments");
}

/// Shared dispatch for shaders that bind the pixel buffer and params buffer.
pub(crate) fn dispatch_with_params(
    runtime: &GpuRuntime,
    pipeline: &wgpu::ComputePipeline,
    label: &str,
) {
    let bind_group_layout = pipeline.get_bind_group_layout(0);
    let bind_group = runtime
        .device
        .create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(label),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: runtime.pixel_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: runtime.params_buffer.as_entire_binding(),
                },
            ],
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
    use crate::engine::gpu::params::GpuParameters;
    use crate::engine::gpu::runtime::gpu_available;
    use crate::engine::gpu::shaders::ShaderCache;
    use crate::engine::Parameters;

    #[test]
    fn gpu_exposure_brightens_pixels() {
        if !gpu_available() {
            eprintln!("skipping: no GPU adapter");
            return;
        }
        let runtime = GpuRuntime::new(2, 2).unwrap();
        let shaders = ShaderCache::new(&runtime.device).unwrap();

        let pixels = vec![[0.2, 0.4, 0.6]; 4];
        runtime.upload_pixels(&pixels);

        let mut params = Parameters::default();
        params.exposure = 1.0; // +1 stop = 2x brightness
        let mut gpu_params: GpuParameters = (&params).into();
        gpu_params.width = 2.0;
        gpu_params.height = 2.0;
        runtime.upload_params(&gpu_params);

        let pipeline = shaders.get("linear_adjustments").unwrap();
        dispatch_linear_adjustments(&runtime, pipeline);

        let result = runtime.download_pixels();
        // +1 stop should roughly double values
        for (orig, out) in pixels.iter().zip(result.iter()) {
            for c in 0..3 {
                assert!(
                    out[c] > orig[c] * 1.5,
                    "expected brighter: orig={}, got={}",
                    orig[c],
                    out[c]
                );
            }
        }
    }

    #[test]
    fn gpu_neutral_params_is_identity() {
        if !gpu_available() {
            eprintln!("skipping: no GPU adapter");
            return;
        }
        let runtime = GpuRuntime::new(2, 2).unwrap();
        let shaders = ShaderCache::new(&runtime.device).unwrap();

        let pixels = vec![
            [0.3, 0.5, 0.7],
            [0.1, 0.9, 0.4],
            [0.6, 0.2, 0.8],
            [0.0, 1.0, 0.5],
        ];
        runtime.upload_pixels(&pixels);

        let params = Parameters::default();
        let mut gpu_params: GpuParameters = (&params).into();
        gpu_params.width = 2.0;
        gpu_params.height = 2.0;
        runtime.upload_params(&gpu_params);

        let pipeline = shaders.get("linear_adjustments").unwrap();
        dispatch_linear_adjustments(&runtime, pipeline);

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
