//! Denoise (à trous wavelet decomposition) GPU dispatcher.
//!
//! Processes Y, Cb, Cr channels sequentially. Each channel goes through
//! 5 wavelet decomposition levels with separable B3-spline convolution,
//! soft thresholding per level, and wavelet reconstruction.
//!
//! Sigma estimation uses a hybrid approach: level-0 detail is read back
//! to CPU for MAD-based sigma computation, matching the CPU path exactly.

use crate::adjust::denoise::{estimate_sigma, NoiseReductionParams, LEVEL_SCALE, NUM_LEVELS};
use crate::engine::gpu::params::GpuParameters;
use crate::engine::gpu::runtime::GpuRuntime;
use crate::engine::gpu::shaders::ShaderCache;

/// Dispatch the full denoise stage (à trous wavelet, 3 channels).
///
/// When noise reduction params are all zero, returns immediately.
/// Otherwise processes Y, Cb, Cr channels sequentially through
/// the wavelet decomposition pipeline.
pub fn dispatch_denoise(
    runtime: &GpuRuntime,
    shaders: &ShaderCache,
    gpu_params: &mut GpuParameters,
    nr_params: &NoiseReductionParams,
) {
    if nr_params.is_neutral() {
        return;
    }

    let luma_strength = nr_params.luminance / 100.0 * 3.0;
    let chroma_strength = nr_params.color / 100.0 * 3.0;
    let detail_factor = 1.0 - nr_params.detail / 100.0;

    let rgb_to_channel = shaders
        .get("denoise_rgb_to_channel")
        .expect("denoise_rgb_to_channel");
    let atrous_h = shaders.get("denoise_atrous_h").expect("denoise_atrous_h");
    let atrous_v = shaders.get("denoise_atrous_v").expect("denoise_atrous_v");
    let threshold_accum = shaders
        .get("denoise_threshold_accum")
        .expect("denoise_threshold_accum");
    let add_residual = shaders
        .get("denoise_add_residual")
        .expect("denoise_add_residual");
    let channel_to_rgb = shaders
        .get("denoise_channel_to_rgb")
        .expect("denoise_channel_to_rgb");

    // Process channels: 0=Y, 1=Cb, 2=Cr
    for channel in 0u32..3 {
        let strength = if channel == 0 {
            luma_strength
        } else {
            chroma_strength
        };
        if strength == 0.0 {
            continue;
        }

        let is_luma = channel == 0;

        // 1. Extract channel from pixels → lum_buffer
        gpu_params.nr_channel = channel as f32;
        gpu_params.nr_is_luma = if is_luma { 1.0 } else { 0.0 };
        runtime.upload_params(gpu_params);
        dispatch_rgb_to_channel(runtime, rgb_to_channel);

        // 2. Zero out denoise_accum_buffer
        let zero_data = vec![0.0f32; runtime.pixel_count() as usize];
        runtime.queue.write_buffer(
            &runtime.denoise_accum_buffer,
            0,
            bytemuck::cast_slice(&zero_data),
        );

        // 3. Level 0: compute smoothed, estimate sigma from detail
        gpu_params.nr_gap = 1.0;
        runtime.upload_params(gpu_params);
        dispatch_atrous_h(runtime, atrous_h); // lum_buffer → temp_buffer
        dispatch_atrous_v(runtime, atrous_v); // temp_buffer → blur_buffer

        // Read back lum_buffer (approx) and blur_buffer (smoothed) for sigma estimation
        let approx_data = runtime.download_single_channel(&runtime.lum_buffer);
        let smoothed_data = runtime.download_single_channel(&runtime.blur_buffer);

        let detail_0: Vec<f32> = approx_data
            .iter()
            .zip(smoothed_data.iter())
            .map(|(a, s)| a - s)
            .collect();
        let sigma = estimate_sigma(&detail_0);

        // 4. Threshold + accumulate level 0
        let mut threshold = sigma * LEVEL_SCALE[0] * strength;
        if is_luma {
            threshold *= detail_factor;
        }
        gpu_params.nr_threshold = threshold;
        runtime.upload_params(gpu_params);
        dispatch_threshold_accum(runtime, threshold_accum);
        // After: lum_buffer = smoothed (next level's approx), accum has detail[0]

        // 5. Levels 1–4
        for (level, &scale) in LEVEL_SCALE.iter().enumerate().take(NUM_LEVELS).skip(1) {
            gpu_params.nr_gap = (1u32 << level) as f32;
            gpu_params.nr_threshold = sigma * scale * strength;
            runtime.upload_params(gpu_params);

            dispatch_atrous_h(runtime, atrous_h);
            dispatch_atrous_v(runtime, atrous_v);
            dispatch_threshold_accum(runtime, threshold_accum);
        }

        // 6. Add residual (final approximation in lum_buffer)
        dispatch_add_residual(runtime, add_residual);

        // 7. Write denoised channel back to pixels
        gpu_params.nr_channel = channel as f32;
        runtime.upload_params(gpu_params);
        dispatch_channel_to_rgb(runtime, channel_to_rgb);
    }
}

/// Extract a YCbCr channel from pixels → lum_buffer.
fn dispatch_rgb_to_channel(runtime: &GpuRuntime, pipeline: &wgpu::ComputePipeline) {
    let bind_group = runtime
        .device
        .create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("denoise_rgb_to_channel"),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: runtime.pixel_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: runtime.lum_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: runtime.params_buffer.as_entire_binding(),
                },
            ],
        });

    let workgroup_count = runtime.pixel_count().div_ceil(256);
    let mut encoder = runtime
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("denoise_rgb_to_channel"),
        });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("denoise_rgb_to_channel"),
            timestamp_writes: None,
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(workgroup_count, 1, 1);
    }
    runtime.queue.submit(std::iter::once(encoder.finish()));
}

/// Horizontal à trous B3-spline: lum_buffer → temp_buffer.
fn dispatch_atrous_h(runtime: &GpuRuntime, pipeline: &wgpu::ComputePipeline) {
    let bind_group = runtime
        .device
        .create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("denoise_atrous_h"),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: runtime.lum_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: runtime.temp_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: runtime.params_buffer.as_entire_binding(),
                },
            ],
        });

    let workgroup_count = runtime.pixel_count().div_ceil(256);
    let mut encoder = runtime
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("denoise_atrous_h"),
        });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("denoise_atrous_h"),
            timestamp_writes: None,
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(workgroup_count, 1, 1);
    }
    runtime.queue.submit(std::iter::once(encoder.finish()));
}

/// Vertical à trous B3-spline: temp_buffer → blur_buffer.
fn dispatch_atrous_v(runtime: &GpuRuntime, pipeline: &wgpu::ComputePipeline) {
    let bind_group = runtime
        .device
        .create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("denoise_atrous_v"),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: runtime.temp_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: runtime.blur_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: runtime.params_buffer.as_entire_binding(),
                },
            ],
        });

    let workgroup_count = runtime.pixel_count().div_ceil(256);
    let mut encoder = runtime
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("denoise_atrous_v"),
        });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("denoise_atrous_v"),
            timestamp_writes: None,
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(workgroup_count, 1, 1);
    }
    runtime.queue.submit(std::iter::once(encoder.finish()));
}

/// Compute detail, threshold, accumulate; update approx for next level.
/// approx=lum_buffer, smoothed=blur_buffer, accum=denoise_accum_buffer.
fn dispatch_threshold_accum(runtime: &GpuRuntime, pipeline: &wgpu::ComputePipeline) {
    let bind_group = runtime
        .device
        .create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("denoise_threshold_accum"),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: runtime.lum_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: runtime.blur_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: runtime.denoise_accum_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: runtime.params_buffer.as_entire_binding(),
                },
            ],
        });

    let workgroup_count = runtime.pixel_count().div_ceil(256);
    let mut encoder = runtime
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("denoise_threshold_accum"),
        });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("denoise_threshold_accum"),
            timestamp_writes: None,
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(workgroup_count, 1, 1);
    }
    runtime.queue.submit(std::iter::once(encoder.finish()));
}

/// Add final residual (lum_buffer) to accumulator (denoise_accum_buffer).
fn dispatch_add_residual(runtime: &GpuRuntime, pipeline: &wgpu::ComputePipeline) {
    let bind_group = runtime
        .device
        .create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("denoise_add_residual"),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: runtime.lum_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: runtime.denoise_accum_buffer.as_entire_binding(),
                },
            ],
        });

    let workgroup_count = runtime.pixel_count().div_ceil(256);
    let mut encoder = runtime
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("denoise_add_residual"),
        });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("denoise_add_residual"),
            timestamp_writes: None,
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(workgroup_count, 1, 1);
    }
    runtime.queue.submit(std::iter::once(encoder.finish()));
}

/// Write denoised channel from denoise_accum_buffer back to pixels.
fn dispatch_channel_to_rgb(runtime: &GpuRuntime, pipeline: &wgpu::ComputePipeline) {
    let bind_group = runtime
        .device
        .create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("denoise_channel_to_rgb"),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: runtime.pixel_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: runtime.denoise_accum_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: runtime.params_buffer.as_entire_binding(),
                },
            ],
        });

    let workgroup_count = runtime.pixel_count().div_ceil(256);
    let mut encoder = runtime
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("denoise_channel_to_rgb"),
        });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("denoise_channel_to_rgb"),
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
    use crate::engine::gpu::params::GpuParameters;
    use crate::engine::gpu::runtime::GpuRuntime;
    use crate::engine::gpu::shaders::ShaderCache;
    use crate::engine::Parameters;

    fn gpu_available() -> bool {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .is_some()
    }

    #[test]
    fn gpu_denoise_zero_is_identity() {
        if !gpu_available() {
            eprintln!("skipping: no GPU adapter");
            return;
        }

        let width = 16u32;
        let height = 16u32;
        let pixels: Vec<[f32; 3]> = (0..(width * height) as usize)
            .map(|i| {
                let t = i as f32 / (width * height - 1) as f32;
                [t, 1.0 - t, 0.5]
            })
            .collect();

        let runtime = GpuRuntime::new(width, height).unwrap();
        let shaders = ShaderCache::new(&runtime.device).unwrap();
        runtime.upload_pixels(&pixels);

        let params = Parameters::default();
        let mut gpu_params: GpuParameters = (&params).into();
        gpu_params.width = width as f32;
        gpu_params.height = height as f32;
        runtime.upload_params(&gpu_params);

        let nr_params = NoiseReductionParams::default();
        dispatch_denoise(&runtime, &shaders, &mut gpu_params, &nr_params);

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

    #[test]
    fn gpu_denoise_reduces_luma_variance() {
        if !gpu_available() {
            eprintln!("skipping: no GPU adapter");
            return;
        }

        let width = 32u32;
        let height = 32u32;
        let mut pixels: Vec<[f32; 3]> = Vec::with_capacity((width * height) as usize);
        let mut rng: u64 = 123;
        for i in 0..(width * height) as usize {
            let base = i as f32 / (width * height) as f32 * 0.5 + 0.25;
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
            let noise = ((rng >> 33) as f32 / (1u64 << 31) as f32 - 0.5) * 0.1;
            let v = (base + noise).clamp(0.0, 1.0);
            pixels.push([v, v, v]);
        }

        let runtime = GpuRuntime::new(width, height).unwrap();
        let shaders = ShaderCache::new(&runtime.device).unwrap();
        runtime.upload_pixels(&pixels);

        let params = Parameters::default();
        let mut gpu_params: GpuParameters = (&params).into();
        gpu_params.width = width as f32;
        gpu_params.height = height as f32;
        gpu_params.nr_luminance = 50.0;
        gpu_params.nr_color = 0.0;
        gpu_params.nr_detail = 0.0;
        runtime.upload_params(&gpu_params);

        let nr_params = NoiseReductionParams {
            luminance: 50.0,
            color: 0.0,
            detail: 0.0,
        };
        dispatch_denoise(&runtime, &shaders, &mut gpu_params, &nr_params);

        let result = runtime.download_pixels();

        let luma_before: Vec<f32> = pixels
            .iter()
            .map(|p| 0.2126 * p[0] + 0.7152 * p[1] + 0.0722 * p[2])
            .collect();
        let luma_after: Vec<f32> = result
            .iter()
            .map(|p| 0.2126 * p[0] + 0.7152 * p[1] + 0.0722 * p[2])
            .collect();
        let var = |v: &[f32]| {
            let mean = v.iter().sum::<f32>() / v.len() as f32;
            v.iter().map(|x| (x - mean).powi(2)).sum::<f32>() / v.len() as f32
        };
        let var_before = var(&luma_before);
        let var_after = var(&luma_after);
        assert!(
            var_after < var_before,
            "NR should reduce luma variance: before={var_before}, after={var_after}"
        );
    }

    #[test]
    fn gpu_denoise_luminance_matches_cpu() {
        if !gpu_available() {
            eprintln!("skipping: no GPU adapter");
            return;
        }

        let width = 32usize;
        let height = 32usize;
        let mut pixels: Vec<[f32; 3]> = Vec::with_capacity(width * height);
        let mut rng: u64 = 456;
        for i in 0..(width * height) {
            let base = i as f32 / (width * height) as f32 * 0.5 + 0.25;
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
            let noise = ((rng >> 33) as f32 / (1u64 << 31) as f32 - 0.5) * 0.1;
            let v = (base + noise).clamp(0.0, 1.0);
            pixels.push([v, v, v]);
        }

        let nr_params = NoiseReductionParams {
            luminance: 50.0,
            color: 0.0,
            detail: 0.0,
        };

        // CPU path
        let cpu_result =
            crate::adjust::denoise::apply_noise_reduction(&pixels, width, height, &nr_params);

        // GPU path
        let runtime = GpuRuntime::new(width as u32, height as u32).unwrap();
        let shaders = ShaderCache::new(&runtime.device).unwrap();
        runtime.upload_pixels(&pixels);

        let params = Parameters::default();
        let mut gpu_params: GpuParameters = (&params).into();
        gpu_params.width = width as f32;
        gpu_params.height = height as f32;
        gpu_params.nr_luminance = nr_params.luminance;
        gpu_params.nr_color = nr_params.color;
        gpu_params.nr_detail = nr_params.detail;
        runtime.upload_params(&gpu_params);

        dispatch_denoise(&runtime, &shaders, &mut gpu_params, &nr_params);
        let gpu_result = runtime.download_pixels();

        let tolerance = 0.05;
        let mut max_diff = 0.0f32;
        for (i, (cpu, gpu)) in cpu_result.iter().zip(gpu_result.iter()).enumerate() {
            for c in 0..3 {
                let diff = (cpu[c] - gpu[c]).abs();
                max_diff = max_diff.max(diff);
                assert!(
                    diff < tolerance,
                    "pixel[{i}][{c}]: cpu={}, gpu={} (diff={diff})",
                    cpu[c],
                    gpu[c]
                );
            }
        }
        eprintln!("gpu_denoise_luminance_matches_cpu: max_diff={max_diff:.6}");
    }
}
