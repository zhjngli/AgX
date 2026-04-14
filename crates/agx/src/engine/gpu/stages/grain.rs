//! Grain (film noise) GPU dispatcher.
//!
//! Runs up to 3 sequential dispatches:
//! 1. Generate Gaussian noise into `lum_buffer`
//! 2. Optional 2-pass Gaussian blur: `lum_buffer` -> `temp_buffer` -> `blur_buffer`
//! 3. Apply grain to pixel buffer using blurred (or unblurred) noise

use crate::adjust::detail::build_gaussian_kernel;
use crate::adjust::grain::GrainParams;
use crate::engine::gpu::params::GpuParameters;
use crate::engine::gpu::runtime::GpuRuntime;
use crate::engine::gpu::shaders::ShaderCache;

/// Minimum sigma for blur to be applied (below this, noise is used unblurred).
const MIN_BLUR_SIGMA: f32 = 0.3;

/// Dispatch the full grain stage (noise generation, optional blur, apply).
///
/// When `grain_params.amount` is zero the function returns immediately.
/// For non-zero amounts, noise is generated into the luminance buffer,
/// optionally blurred based on grain size, then applied to the pixel buffer
/// with luminance-weighted blending.
pub fn dispatch_grain(
    runtime: &GpuRuntime,
    shaders: &ShaderCache,
    gpu_params: &mut GpuParameters,
    grain_params: &GrainParams,
) {
    if grain_params.is_neutral() {
        return;
    }

    let sigma = grain_sigma(grain_params.size, runtime.width, runtime.height);
    let use_blur = sigma >= MIN_BLUR_SIGMA;

    let noise_gen_pipeline = shaders.get("grain_noise_gen").expect("grain_noise_gen");

    // 1. Generate noise → lum_buffer
    dispatch_noise_gen(runtime, noise_gen_pipeline);

    if use_blur {
        // Build and upload kernel for grain blur
        let kernel = build_gaussian_kernel(sigma);
        runtime.upload_kernel(&kernel);

        gpu_params.kernel_size = kernel.len() as f32;
        runtime.upload_params(gpu_params);

        let blur_h_pipeline = shaders.get("blur_horizontal").expect("blur_horizontal");
        let blur_v_pipeline = shaders.get("blur_vertical").expect("blur_vertical");

        // 2a. Horizontal blur: lum_buffer → temp_buffer
        dispatch_blur_h(runtime, blur_h_pipeline);

        // 2b. Vertical blur: temp_buffer → blur_buffer
        dispatch_blur_v(runtime, blur_v_pipeline);
    }

    let apply_pipeline = shaders.get("grain_apply").expect("grain_apply");

    // 3. Apply grain from blur_buffer (blurred) or lum_buffer (unblurred) to pixel_buffer
    if use_blur {
        dispatch_apply(runtime, apply_pipeline, &runtime.blur_buffer);
    } else {
        dispatch_apply(runtime, apply_pipeline, &runtime.lum_buffer);
    }
}

/// Compute grain blur sigma from size parameter and image dimensions.
fn grain_sigma(size: f32, width: u32, height: u32) -> f32 {
    let t = (size / 100.0).clamp(0.0, 1.0);
    let base_sigma = t.powf(1.5) * 1.0; // GRAIN_MAX_SIGMA = 1.0
    let long_edge = width.max(height) as f32;
    base_sigma * (long_edge / 2000.0) // GRAIN_REF_RESOLUTION = 2000
}

/// Generate noise into lum_buffer.
fn dispatch_noise_gen(runtime: &GpuRuntime, pipeline: &wgpu::ComputePipeline) {
    let bind_group_layout = pipeline.get_bind_group_layout(0);
    let bind_group = runtime
        .device
        .create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("grain_noise_gen"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: runtime.lum_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: runtime.params_buffer.as_entire_binding(),
                },
            ],
        });

    let workgroup_count = runtime.pixel_count().div_ceil(256);
    let mut encoder = runtime
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("grain_noise_gen"),
        });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("grain_noise_gen"),
            timestamp_writes: None,
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(workgroup_count, 1, 1);
    }
    runtime.queue.submit(std::iter::once(encoder.finish()));
}

/// Horizontal blur: lum_buffer (input) -> temp_buffer (output).
fn dispatch_blur_h(runtime: &GpuRuntime, pipeline: &wgpu::ComputePipeline) {
    let bind_group_layout = pipeline.get_bind_group_layout(0);
    let bind_group = runtime
        .device
        .create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("grain_blur_h"),
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
            label: Some("grain_blur_h"),
        });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("grain_blur_h"),
            timestamp_writes: None,
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(workgroup_count, 1, 1);
    }
    runtime.queue.submit(std::iter::once(encoder.finish()));
}

/// Vertical blur: temp_buffer (input) -> blur_buffer (output).
fn dispatch_blur_v(runtime: &GpuRuntime, pipeline: &wgpu::ComputePipeline) {
    let bind_group_layout = pipeline.get_bind_group_layout(0);
    let bind_group = runtime
        .device
        .create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("grain_blur_v"),
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
            label: Some("grain_blur_v"),
        });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("grain_blur_v"),
            timestamp_writes: None,
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(workgroup_count, 1, 1);
    }
    runtime.queue.submit(std::iter::once(encoder.finish()));
}

/// Apply grain noise to pixel buffer.
///
/// `noise_buffer` is either `lum_buffer` (no blur) or `blur_buffer` (after blur).
fn dispatch_apply(
    runtime: &GpuRuntime,
    pipeline: &wgpu::ComputePipeline,
    noise_buffer: &wgpu::Buffer,
) {
    let bind_group_layout = pipeline.get_bind_group_layout(0);
    let bind_group = runtime
        .device
        .create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("grain_apply"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: runtime.pixel_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: noise_buffer.as_entire_binding(),
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
            label: Some("grain_apply"),
        });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("grain_apply"),
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
    use crate::adjust::grain::{GrainParams, GrainType};
    use crate::engine::gpu::params::GpuParameters;
    use crate::engine::gpu::runtime::GpuRuntime;
    use crate::engine::gpu::shaders::ShaderCache;
    use crate::engine::Parameters;

    fn gpu_available() -> bool {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .is_some()
    }

    fn setup_runtime(
        width: u32,
        height: u32,
        pixels: &[[f32; 3]],
        grain: &GrainParams,
    ) -> (GpuRuntime, ShaderCache, GpuParameters) {
        let runtime = GpuRuntime::new(width, height).unwrap();
        let shaders = ShaderCache::new(&runtime.device).unwrap();
        runtime.upload_pixels(pixels);

        let params = Parameters::default();
        let mut gpu_params: GpuParameters = (&params).into();
        gpu_params.width = width as f32;
        gpu_params.height = height as f32;
        gpu_params.grain_amount = grain.amount;
        gpu_params.grain_size = grain.size;
        gpu_params.grain_type = match grain.grain_type {
            GrainType::Fine => 0.0,
            GrainType::Silver => 1.0,
            GrainType::Harsh => 2.0,
        };
        gpu_params.grain_seed = grain.seed.unwrap_or(42) as f32;
        runtime.upload_params(&gpu_params);

        (runtime, shaders, gpu_params)
    }

    #[test]
    fn gpu_grain_modifies_image() {
        if !gpu_available() {
            eprintln!("skipping: no GPU adapter");
            return;
        }

        let width = 16u32;
        let height = 16u32;
        let pixels: Vec<[f32; 3]> = (0..(width * height) as usize)
            .map(|i| {
                let t = i as f32 / (width * height - 1) as f32;
                [t, t, t]
            })
            .collect();

        let grain = GrainParams {
            amount: 50.0,
            size: 25.0,
            grain_type: GrainType::Silver,
            seed: Some(42),
        };

        let (runtime, shaders, mut gpu_params) = setup_runtime(width, height, &pixels, &grain);
        dispatch_grain(&runtime, &shaders, &mut gpu_params, &grain);
        let result = runtime.download_pixels();

        // At least some pixels should differ from the input
        let changed = pixels
            .iter()
            .zip(result.iter())
            .any(|(a, b)| (a[0] - b[0]).abs() > 1e-4);
        assert!(changed, "expected grain to modify some pixels");
    }

    #[test]
    fn gpu_grain_zero_amount_is_identity() {
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

        let grain = GrainParams {
            amount: 0.0,
            size: 50.0,
            grain_type: GrainType::Silver,
            seed: Some(42),
        };

        let (runtime, shaders, mut gpu_params) = setup_runtime(width, height, &pixels, &grain);
        dispatch_grain(&runtime, &shaders, &mut gpu_params, &grain);
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
    fn gpu_grain_respects_luminance() {
        if !gpu_available() {
            eprintln!("skipping: no GPU adapter");
            return;
        }

        let width = 32u32;
        let height = 32u32;
        let total = (width * height) as usize;

        // Create image: top half dark (0.1), bottom half bright (0.9)
        let pixels: Vec<[f32; 3]> = (0..total)
            .map(|i| {
                let y = i / width as usize;
                if y < height as usize / 2 {
                    [0.1, 0.1, 0.1] // dark
                } else {
                    [0.9, 0.9, 0.9] // bright
                }
            })
            .collect();

        let grain = GrainParams {
            amount: 80.0,
            size: 25.0,
            grain_type: GrainType::Silver,
            seed: Some(123),
        };

        let (runtime, shaders, mut gpu_params) = setup_runtime(width, height, &pixels, &grain);
        dispatch_grain(&runtime, &shaders, &mut gpu_params, &grain);
        let result = runtime.download_pixels();

        // Measure variance of changes in dark vs bright regions
        let half = total / 2;
        let dark_variance: f32 = (0..half)
            .map(|i| {
                let diff = result[i][0] - pixels[i][0];
                diff * diff
            })
            .sum::<f32>()
            / half as f32;

        let bright_variance: f32 = (half..total)
            .map(|i| {
                let diff = result[i][0] - pixels[i][0];
                diff * diff
            })
            .sum::<f32>()
            / (total - half) as f32;

        // Dark pixels should get more grain effect (higher variance) than bright pixels
        // due to luminance-weighted falloff
        assert!(
            dark_variance > bright_variance,
            "dark_variance ({dark_variance:.6}) should be greater than bright_variance ({bright_variance:.6})"
        );
    }
}
