//! Detail (sharpening, clarity, texture) GPU dispatcher.
//!
//! Runs up to 3 sequential unsharp-mask passes, each consisting of:
//! 1. Extract luminance from pixel buffer
//! 2. Horizontal Gaussian blur on luminance
//! 3. Vertical Gaussian blur on intermediate
//! 4. Apply unsharp mask delta to pixel buffer

use crate::adjust::detail::{build_gaussian_kernel, DetailParams};
use crate::engine::gpu::params::GpuParameters;
use crate::engine::gpu::runtime::GpuRuntime;
use crate::engine::gpu::shaders::ShaderCache;

/// Dispatch the full detail stage (texture, clarity, sharpening).
///
/// Each active sub-pass (texture, clarity, sharpening) is dispatched
/// sequentially: extract luminance, 2-pass Gaussian blur, apply unsharp mask.
pub fn dispatch_detail(
    runtime: &GpuRuntime,
    shaders: &ShaderCache,
    gpu_params: &mut GpuParameters,
    params: &DetailParams,
) {
    if params.is_neutral() {
        return;
    }

    // Build list of active passes: (sigma, strength, threshold)
    let mut passes: Vec<(f32, f32, f32)> = Vec::new();

    if params.texture != 0.0 {
        passes.push((3.0, params.texture / 100.0, 0.0));
    }
    if params.clarity != 0.0 {
        passes.push((20.0, params.clarity / 100.0, 0.0));
    }
    if params.sharpening.amount != 0.0 {
        let sigma = params.sharpening.radius.max(0.1);
        let strength = params.sharpening.amount / 100.0;
        let threshold = params.sharpening.threshold / 255.0;
        passes.push((sigma, strength, threshold));
    }

    let extract_lum_pipeline = shaders
        .get("detail_extract_lum")
        .expect("detail_extract_lum");
    let blur_h_pipeline = shaders.get("blur_horizontal").expect("blur_horizontal");
    let blur_v_pipeline = shaders.get("blur_vertical").expect("blur_vertical");
    let apply_pipeline = shaders.get("detail_apply").expect("detail_apply");

    for (sigma, strength, threshold) in passes {
        // Build and upload kernel
        let kernel = build_gaussian_kernel(sigma);
        runtime.upload_kernel(&kernel);

        // Update detail params
        gpu_params.detail_strength = strength;
        gpu_params.detail_threshold = threshold;
        gpu_params.detail_masking = 0.0; // masking not yet implemented on GPU
        gpu_params.kernel_size = kernel.len() as f32;
        runtime.upload_params(gpu_params);

        // 1. Extract luminance → lum_buffer
        dispatch_extract_lum(runtime, extract_lum_pipeline);

        // 2. Horizontal blur: lum_buffer → temp_buffer
        dispatch_blur_h(runtime, blur_h_pipeline);

        // 3. Vertical blur: temp_buffer → blur_buffer
        dispatch_blur_v(runtime, blur_v_pipeline);

        // 4. Apply unsharp mask: read lum_buffer (original) and blur_buffer (blurred),
        //    write delta to pixel_buffer
        dispatch_apply(runtime, apply_pipeline);
    }
}

/// Extract luminance from pixel buffer into lum_buffer.
fn dispatch_extract_lum(runtime: &GpuRuntime, pipeline: &wgpu::ComputePipeline) {
    let bind_group_layout = pipeline.get_bind_group_layout(0);
    let bind_group = runtime
        .device
        .create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("detail_extract_lum"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: runtime.pixel_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: runtime.lum_buffer.as_entire_binding(),
                },
            ],
        });

    let workgroup_count = runtime.pixel_count().div_ceil(256);
    let mut encoder = runtime
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("detail_extract_lum"),
        });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("detail_extract_lum"),
            timestamp_writes: None,
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(workgroup_count, 1, 1);
    }
    runtime.queue.submit(std::iter::once(encoder.finish()));
}

/// Horizontal blur: lum_buffer (input) → temp_buffer (output).
fn dispatch_blur_h(runtime: &GpuRuntime, pipeline: &wgpu::ComputePipeline) {
    let bind_group_layout = pipeline.get_bind_group_layout(0);
    let bind_group = runtime
        .device
        .create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("blur_horizontal"),
            layout: &bind_group_layout,
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
                    resource: runtime.kernel_buffer.as_entire_binding(),
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
            label: Some("blur_horizontal"),
        });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("blur_horizontal"),
            timestamp_writes: None,
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(workgroup_count, 1, 1);
    }
    runtime.queue.submit(std::iter::once(encoder.finish()));
}

/// Vertical blur: temp_buffer (input) → blur_buffer (output).
fn dispatch_blur_v(runtime: &GpuRuntime, pipeline: &wgpu::ComputePipeline) {
    let bind_group_layout = pipeline.get_bind_group_layout(0);
    let bind_group = runtime
        .device
        .create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("blur_vertical"),
            layout: &bind_group_layout,
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
                    resource: runtime.kernel_buffer.as_entire_binding(),
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
            label: Some("blur_vertical"),
        });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("blur_vertical"),
            timestamp_writes: None,
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(workgroup_count, 1, 1);
    }
    runtime.queue.submit(std::iter::once(encoder.finish()));
}

/// Apply unsharp mask: read lum_buffer (original) + blur_buffer (blurred), modify pixel_buffer.
fn dispatch_apply(runtime: &GpuRuntime, pipeline: &wgpu::ComputePipeline) {
    let bind_group_layout = pipeline.get_bind_group_layout(0);
    let bind_group = runtime
        .device
        .create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("detail_apply"),
            layout: &bind_group_layout,
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
                    resource: runtime.blur_buffer.as_entire_binding(),
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
            label: Some("detail_apply"),
        });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("detail_apply"),
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
    use crate::adjust::detail::{DetailParams, SharpeningParams};
    use crate::engine::gpu::runtime::GpuRuntime;
    use crate::engine::gpu::shaders::ShaderCache;
    use crate::engine::Parameters;

    fn gpu_available() -> bool {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .is_some()
    }

    #[test]
    fn gpu_detail_neutral_is_identity() {
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

        let detail_params = DetailParams::default();
        dispatch_detail(&runtime, &shaders, &mut gpu_params, &detail_params);

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
    fn gpu_detail_sharpening_matches_cpu() {
        if !gpu_available() {
            eprintln!("skipping: no GPU adapter");
            return;
        }

        let width = 16usize;
        let height = 16usize;
        // Create a gradient pattern that will exercise sharpening
        let pixels: Vec<[f32; 3]> = (0..width * height)
            .map(|i| {
                let x = (i % width) as f32 / (width - 1) as f32;
                let y = (i / width) as f32 / (height - 1) as f32;
                [x, y * 0.8, 0.5 * x + 0.3 * y]
            })
            .collect();

        let detail_params = DetailParams {
            sharpening: SharpeningParams {
                amount: 50.0,
                radius: 1.0,
                threshold: 0.0,
                masking: 0.0,
            },
            clarity: 0.0,
            texture: 0.0,
        };

        // CPU path
        let cpu_result =
            crate::adjust::detail::apply_detail_pass(&pixels, width, height, &detail_params);

        // GPU path
        let runtime = GpuRuntime::new(width as u32, height as u32).unwrap();
        let shaders = ShaderCache::new(&runtime.device).unwrap();
        runtime.upload_pixels(&pixels);

        let params = Parameters::default();
        let mut gpu_params: GpuParameters = (&params).into();
        gpu_params.width = width as f32;
        gpu_params.height = height as f32;
        runtime.upload_params(&gpu_params);

        dispatch_detail(&runtime, &shaders, &mut gpu_params, &detail_params);
        let gpu_result = runtime.download_pixels();

        // Generous tolerance: GPU float precision and workgroup ordering
        let tolerance = 0.02;
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
        eprintln!("gpu_detail_sharpening_matches_cpu: max_diff={max_diff:.6}");
    }

    #[test]
    fn gpu_detail_texture_changes_output() {
        if !gpu_available() {
            eprintln!("skipping: no GPU adapter");
            return;
        }

        let width = 16u32;
        let height = 16u32;
        // Checkerboard pattern — high frequency content for texture to act on
        let pixels: Vec<[f32; 3]> = (0..(width * height) as usize)
            .map(|i| {
                let x = i % width as usize;
                let y = i / width as usize;
                let v = if (x + y) % 2 == 0 { 0.8 } else { 0.2 };
                [v, v, v]
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

        let detail_params = DetailParams {
            texture: 50.0,
            ..Default::default()
        };
        dispatch_detail(&runtime, &shaders, &mut gpu_params, &detail_params);
        let result = runtime.download_pixels();

        // At least some pixels should differ from the input
        let changed = pixels
            .iter()
            .zip(result.iter())
            .any(|(a, b)| (a[0] - b[0]).abs() > 1e-4);
        assert!(changed, "expected texture pass to modify some pixels");
    }

    #[test]
    fn gpu_detail_clarity_matches_cpu() {
        if !gpu_available() {
            eprintln!("skipping: no GPU adapter");
            return;
        }

        let width = 16usize;
        let height = 16usize;
        let pixels: Vec<[f32; 3]> = (0..width * height)
            .map(|i| {
                let x = (i % width) as f32 / (width - 1) as f32;
                [x, x, x]
            })
            .collect();

        let detail_params = DetailParams {
            clarity: 40.0,
            ..Default::default()
        };

        // CPU path
        let cpu_result =
            crate::adjust::detail::apply_detail_pass(&pixels, width, height, &detail_params);

        // GPU path
        let runtime = GpuRuntime::new(width as u32, height as u32).unwrap();
        let shaders = ShaderCache::new(&runtime.device).unwrap();
        runtime.upload_pixels(&pixels);

        let params = Parameters::default();
        let mut gpu_params: GpuParameters = (&params).into();
        gpu_params.width = width as f32;
        gpu_params.height = height as f32;
        runtime.upload_params(&gpu_params);

        dispatch_detail(&runtime, &shaders, &mut gpu_params, &detail_params);
        let gpu_result = runtime.download_pixels();

        let tolerance = 0.02;
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
        eprintln!("gpu_detail_clarity_matches_cpu: max_diff={max_diff:.6}");
    }
}
