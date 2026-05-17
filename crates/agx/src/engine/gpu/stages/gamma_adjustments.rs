//! Gamma-space adjustment GPU dispatcher (contrast, highlights, shadows, whites, blacks,
//! tone curves, HSL, color grading, LUT).

use crate::engine::gpu::runtime::GpuRuntime;

/// Dispatch the gamma-adjustments compute shader.
///
/// Binds pixel buffer, params buffer, tone curve buffer, LUT texture, and LUT sampler.
pub fn dispatch_gamma_adjustments(runtime: &GpuRuntime, pipeline: &wgpu::ComputePipeline) {
    let bind_group_layout = pipeline.get_bind_group_layout(0);

    let lut_view = runtime
        .lut_texture_view
        .as_ref()
        .unwrap_or(&runtime.fallback_lut_view);
    let lut_sampler = runtime
        .lut_sampler
        .as_ref()
        .unwrap_or(&runtime.fallback_lut_sampler);

    let bind_group = runtime
        .device
        .create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("gamma_adjustments"),
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
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: runtime.tone_curve_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(lut_view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Sampler(lut_sampler),
                },
            ],
        });

    let wg = runtime.workgroup_counts();
    let mut encoder = runtime
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("gamma_adjustments"),
        });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("gamma_adjustments"),
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
    use crate::engine::gpu::params::{build_tone_curve_data, GpuParameters};
    use crate::engine::gpu::runtime::gpu_available;
    use crate::engine::gpu::shaders::ShaderCache;
    use crate::engine::gpu::stages::color_space::dispatch_linear_to_gamma;
    use crate::engine::Parameters;

    /// Upload identity tone curves for tests that don't test tone curves.
    fn upload_identity_tone_curves(runtime: &GpuRuntime, params: &Parameters) {
        let tc_data = build_tone_curve_data(params);
        runtime.upload_tone_curves(&tc_data);
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

        // Convert to gamma first (gamma adjustments run in gamma space)
        let to_gamma = shaders.get("linear_to_gamma").unwrap();
        dispatch_linear_to_gamma(&runtime, to_gamma);
        let gamma_before = runtime.download_pixels();

        // Apply contrast
        let params = Parameters {
            contrast: 50.0,
            ..Default::default()
        };
        let mut gpu_params: GpuParameters = (&params).into();
        gpu_params.width = 2.0;
        gpu_params.height = 1.0;
        runtime.upload_params(&gpu_params);
        upload_identity_tone_curves(&runtime, &params);

        let pipeline = shaders.get("gamma_adjustments").unwrap();
        dispatch_gamma_adjustments(&runtime, pipeline);

        let result = runtime.download_pixels();
        // With positive contrast, values below 0.5 should decrease, above 0.5 should increase
        // Our test pixel (0.2 linear → ~0.48 sRGB) is near midpoint, so change should be modest
        // but nonzero
        for (i, (before, after)) in gamma_before.iter().zip(result.iter()).enumerate() {
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
        upload_identity_tone_curves(&runtime, &params);

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

    #[test]
    fn gpu_gamma_all_features_neutral_is_identity() {
        if !gpu_available() {
            eprintln!("skipping: no GPU adapter");
            return;
        }
        let runtime = GpuRuntime::new(2, 2).unwrap();
        let shaders = ShaderCache::new(&runtime.device).unwrap();

        // Diverse pixel values to exercise all paths
        let pixels = vec![
            [0.3, 0.5, 0.7],
            [0.1, 0.9, 0.4],
            [0.6, 0.2, 0.8],
            [0.0, 1.0, 0.5],
        ];
        runtime.upload_pixels(&pixels);

        // Default params: neutral tone curves, no HSL, no color grading, no LUT
        let params = Parameters::default();
        let mut gpu_params: GpuParameters = (&params).into();
        gpu_params.width = 2.0;
        gpu_params.height = 2.0;
        runtime.upload_params(&gpu_params);
        upload_identity_tone_curves(&runtime, &params);

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
