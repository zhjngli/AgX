//! Dehaze (dark channel prior + guided filter) GPU dispatcher.
//!
//! Implements the full dehaze pipeline:
//! 1. Dark channel computation (separable min filter)
//! 2. Airlight estimation (CPU readback for percentile selection)
//! 3. Normalized dark channel + raw transmission map
//! 4. Guided filter refinement (8 box filter passes + coefficient computation)
//! 5. Scene recovery (positive) or fog addition (negative)

use crate::adjust::dehaze::DehazeParams;
use crate::engine::gpu::params::GpuParameters;
use crate::engine::gpu::runtime::GpuRuntime;
use crate::engine::gpu::shaders::ShaderCache;

const PATCH_HALF: f32 = 7.0; // PATCH_SIZE=15, half=7
const GUIDED_RADIUS: f32 = 40.0;
const AIRLIGHT_PERCENTILE: f64 = 0.001;

/// Dispatch the full dehaze stage.
///
/// For positive amounts: dark channel → airlight → normalize → transmission →
/// guided filter → scene recovery. For negative: dark channel → airlight → fog blend.
pub fn dispatch_dehaze(
    runtime: &GpuRuntime,
    shaders: &ShaderCache,
    gpu_params: &mut GpuParameters,
    dehaze_params: &DehazeParams,
) {
    if dehaze_params.is_neutral() {
        return;
    }

    let amount = dehaze_params.amount;
    let wg_count = runtime.pixel_count().div_ceil(256);

    let pixel_min_pipeline = shaders.get("dehaze_pixel_min").expect("dehaze_pixel_min");
    let min_filter_pipeline = shaders.get("dehaze_min_filter").expect("dehaze_min_filter");

    // Step 1: Dark channel of original image
    // pixel_min → lum_buffer, min_h → temp_buffer, min_v → blur_buffer
    gpu_params.dehaze_mode = 0.0; // raw
    runtime.upload_params(gpu_params);
    dispatch_3buf(
        runtime,
        pixel_min_pipeline,
        &runtime.pixel_buffer,
        &runtime.lum_buffer,
        &runtime.params_buffer,
        "dehaze_pixel_min",
        wg_count,
    );

    gpu_params.dehaze_filter_radius = PATCH_HALF;
    gpu_params.dehaze_mode = 0.0; // horizontal
    runtime.upload_params(gpu_params);
    dispatch_3buf(
        runtime,
        min_filter_pipeline,
        &runtime.lum_buffer,
        &runtime.temp_buffer,
        &runtime.params_buffer,
        "dehaze_min_h",
        wg_count,
    );

    gpu_params.dehaze_mode = 1.0; // vertical
    runtime.upload_params(gpu_params);
    dispatch_3buf(
        runtime,
        min_filter_pipeline,
        &runtime.temp_buffer,
        &runtime.blur_buffer,
        &runtime.params_buffer,
        "dehaze_min_v",
        wg_count,
    );

    // Step 2: Airlight estimation (CPU readback)
    let dark_channel = runtime.download_single_channel(&runtime.blur_buffer);
    let pixels_cpu = runtime.download_pixels();
    let airlight = estimate_airlight_cpu(&pixels_cpu, &dark_channel);

    gpu_params.dehaze_airlight_r = airlight[0];
    gpu_params.dehaze_airlight_g = airlight[1];
    gpu_params.dehaze_airlight_b = airlight[2];

    if amount < 0.0 {
        // Negative dehaze: just blend toward airlight
        let strength = (-amount / 100.0).min(1.0);
        gpu_params.dehaze_omega = strength;
        gpu_params.dehaze_mode = 1.0; // fog mode
        runtime.upload_params(gpu_params);

        // Use blur_buffer as dummy transmission (unused in fog mode)
        dispatch_3buf(
            runtime,
            shaders.get("dehaze_recover").expect("dehaze_recover"),
            &runtime.pixel_buffer,
            &runtime.blur_buffer,
            &runtime.params_buffer,
            "dehaze_fog",
            wg_count,
        );
        return;
    }

    // Positive dehaze
    let omega = (amount / 100.0).min(1.0);
    gpu_params.dehaze_omega = omega;

    // Step 3: Normalized dark channel
    gpu_params.dehaze_mode = 1.0; // normalized
    runtime.upload_params(gpu_params);
    dispatch_3buf(
        runtime,
        pixel_min_pipeline,
        &runtime.pixel_buffer,
        &runtime.lum_buffer,
        &runtime.params_buffer,
        "dehaze_norm_pmin",
        wg_count,
    );

    gpu_params.dehaze_filter_radius = PATCH_HALF;
    gpu_params.dehaze_mode = 0.0; // horizontal
    runtime.upload_params(gpu_params);
    dispatch_3buf(
        runtime,
        min_filter_pipeline,
        &runtime.lum_buffer,
        &runtime.temp_buffer,
        &runtime.params_buffer,
        "dehaze_norm_min_h",
        wg_count,
    );

    gpu_params.dehaze_mode = 1.0; // vertical
    runtime.upload_params(gpu_params);
    dispatch_3buf(
        runtime,
        min_filter_pipeline,
        &runtime.temp_buffer,
        &runtime.denoise_accum_buffer,
        &runtime.params_buffer,
        "dehaze_norm_min_v",
        wg_count,
    );
    // dc_norm is now in denoise_accum_buffer

    // Step 4: Raw transmission map → lum_buffer
    let transmission_pipeline = shaders
        .get("dehaze_transmission")
        .expect("dehaze_transmission");
    runtime.upload_params(gpu_params);
    dispatch_3buf(
        runtime,
        transmission_pipeline,
        &runtime.denoise_accum_buffer,
        &runtime.lum_buffer,
        &runtime.params_buffer,
        "dehaze_transmission",
        wg_count,
    );
    // t_raw is now in lum_buffer

    // Step 5: Luminance guide → temp_buffer
    let extract_lum_pipeline = shaders
        .get("detail_extract_lum")
        .expect("detail_extract_lum");
    dispatch_2buf(
        runtime,
        extract_lum_pipeline,
        &runtime.pixel_buffer,
        &runtime.temp_buffer,
        "dehaze_lum_guide",
        wg_count,
    );
    // guide is now in temp_buffer

    // Step 6: Guided filter
    // guide = temp_buffer, input = lum_buffer (t_raw)
    let box_filter_pipeline = shaders.get("dehaze_box_filter").expect("dehaze_box_filter");
    let mul_pipeline = shaders.get("dehaze_mul").expect("dehaze_mul");
    let coeffs_pipeline = shaders
        .get("dehaze_guided_coeffs")
        .expect("dehaze_guided_coeffs");
    let fma_pipeline = shaders.get("dehaze_fma").expect("dehaze_fma");

    // 6a: mean_g = box_filter(guide) → scratch_a
    box_filter_2d(
        runtime,
        box_filter_pipeline,
        gpu_params,
        &runtime.temp_buffer,
        &runtime.scratch_a,
        wg_count,
    );

    // 6b: mean_p = box_filter(t_raw) → scratch_b
    box_filter_2d(
        runtime,
        box_filter_pipeline,
        gpu_params,
        &runtime.lum_buffer,
        &runtime.scratch_b,
        wg_count,
    );

    // 6c: gp = guide * t_raw → blur_buffer
    dispatch_3buf(
        runtime,
        mul_pipeline,
        &runtime.temp_buffer,
        &runtime.lum_buffer,
        &runtime.blur_buffer,
        "dehaze_mul_gp",
        wg_count,
    );

    // 6d: mean_gp = box_filter(gp) → scratch_c
    box_filter_2d(
        runtime,
        box_filter_pipeline,
        gpu_params,
        &runtime.blur_buffer,
        &runtime.scratch_c,
        wg_count,
    );

    // 6e: gg = guide * guide → blur_buffer (reuse)
    dispatch_3buf(
        runtime,
        mul_pipeline,
        &runtime.temp_buffer,
        &runtime.temp_buffer,
        &runtime.blur_buffer,
        "dehaze_mul_gg",
        wg_count,
    );

    // 6f: mean_gg = box_filter(gg) → denoise_accum_buffer
    // (can't use scratch_d as output since box_filter_2d uses it as H-intermediate)
    box_filter_2d(
        runtime,
        box_filter_pipeline,
        gpu_params,
        &runtime.blur_buffer,
        &runtime.denoise_accum_buffer,
        wg_count,
    );

    // 6g: Compute coefficients a, b
    // mean_g=scratch_a, mean_p=scratch_b, mean_gp=scratch_c, mean_gg=denoise_accum_buffer
    // a→lum_buffer (t_raw no longer needed), b→blur_buffer
    dispatch_guided_coeffs(
        runtime,
        coeffs_pipeline,
        &runtime.scratch_a,
        &runtime.scratch_b,
        &runtime.scratch_c,
        &runtime.denoise_accum_buffer,
        &runtime.lum_buffer,
        &runtime.blur_buffer,
        wg_count,
    );

    // 6h: mean_a = box_filter(a) → scratch_a
    box_filter_2d(
        runtime,
        box_filter_pipeline,
        gpu_params,
        &runtime.lum_buffer,
        &runtime.scratch_a,
        wg_count,
    );

    // 6i: mean_b = box_filter(b) → scratch_b
    box_filter_2d(
        runtime,
        box_filter_pipeline,
        gpu_params,
        &runtime.blur_buffer,
        &runtime.scratch_b,
        wg_count,
    );

    // 6j: t_refined = mean_a * guide + mean_b → lum_buffer
    dispatch_fma(
        runtime,
        fma_pipeline,
        &runtime.scratch_a,
        &runtime.temp_buffer,
        &runtime.scratch_b,
        &runtime.lum_buffer,
        wg_count,
    );
    // t_refined is now in lum_buffer

    // Step 7: Scene recovery
    gpu_params.dehaze_mode = 0.0; // positive mode
    runtime.upload_params(gpu_params);
    let recover_pipeline = shaders.get("dehaze_recover").expect("dehaze_recover");
    dispatch_3buf(
        runtime,
        recover_pipeline,
        &runtime.pixel_buffer,
        &runtime.lum_buffer,
        &runtime.params_buffer,
        "dehaze_recover",
        wg_count,
    );
}

/// Airlight estimation on CPU: pick brightest pixel among top 0.1% of dark channel.
fn estimate_airlight_cpu(pixels: &[[f32; 3]], dark_channel: &[f32]) -> [f32; 3] {
    let n = pixels.len();
    if n == 0 {
        return [1.0, 1.0, 1.0];
    }
    let top_count = ((n as f64 * AIRLIGHT_PERCENTILE).ceil() as usize).max(1);
    let mut indices: Vec<usize> = (0..n).collect();
    let pivot = top_count.min(n) - 1;
    indices.select_nth_unstable_by(pivot, |&a, &b| {
        dark_channel[b].partial_cmp(&dark_channel[a]).unwrap()
    });
    let mut best_idx = indices[0];
    let mut best_intensity = 0.0_f32;
    for &idx in indices.iter().take(top_count) {
        let [r, g, b] = pixels[idx];
        let intensity = r + g + b;
        if intensity > best_intensity {
            best_intensity = intensity;
            best_idx = idx;
        }
    }
    pixels[best_idx]
}

/// 2D box filter (separable H+V) using a temporary buffer.
/// Uses denoise_accum_buffer as intermediate for H-pass, then writes to output.
fn box_filter_2d(
    runtime: &GpuRuntime,
    pipeline: &wgpu::ComputePipeline,
    gpu_params: &mut GpuParameters,
    input: &wgpu::Buffer,
    output: &wgpu::Buffer,
    wg_count: u32,
) {
    // We need a temp buffer for the H-pass result. Use denoise_accum_buffer if it's
    // not the input or output, otherwise use scratch_c or scratch_d.
    // Since callers may be using various buffers, we use a dedicated intermediate.
    // H-pass: input → lum_buffer (temporary)... but lum_buffer may be in use.
    //
    // To avoid conflicts, the H-pass writes to a known-safe intermediate.
    // We'll use the output buffer itself for H-pass, then V-pass reads from output
    // and writes back to output. Wait, that's wrong — we'd need separate read/write.
    //
    // Safest approach: two dispatches using two distinct buffers.
    // H-pass: input → scratch_c (if not in use) → but scratch_c may be used by caller.
    //
    // Actually, let's just use the approach: H writes to output, V reads from output
    // and writes to a temp, then copy temp→output. But that's 3 dispatches.
    //
    // Simpler: use a scratch that we know is safe. The caller tells us input/output,
    // we need a third buffer for H-intermediate. Let's always use scratch_d as the
    // H-pass temp for box_filter_2d. The caller must not pass scratch_d as input/output.
    //
    // Actually, scratch_d IS used as mean_gg and then as input to guided_coeffs.
    // Let me think about this more carefully...
    //
    // The key insight: box_filter_2d is called 6 times. For each call, we need a
    // temp buffer that's neither the input nor the output. Since we have 8 buffers
    // (lum, temp, blur, denoise_accum, scratch_a/b/c/d), there's always a free one.
    //
    // For simplicity, let's pass the H-intermediate through the same output buffer:
    // H-pass: input → output, then V-pass needs to read from output and write somewhere.
    // But V-pass can't read and write the same buffer.
    //
    // Use a two-step approach with a known scratch:
    // H-pass: input → output (horizontal filtered rows stored in output)
    // V-pass: output → scratch_c (vertical filtered)
    // Copy: scratch_c → output
    //
    // But we can't copy if scratch_c is being used for something else.
    //
    // Let me just make the V-pass write directly to output by reading from a temp.
    // H-pass: input → (temp area), V-pass: (temp area) → output.
    //
    // We know lum_buffer and temp_buffer might be in use (guide and t_raw).
    // Let me use blur_buffer as the H-pass intermediate for box_filter_2d.
    // ... but blur_buffer is also used by the caller.
    //
    // OK, pragmatic solution: always use scratch_d as the H-pass intermediate.
    // The caller must arrange buffer assignments so scratch_d is not the input or output
    // of any active box_filter_2d call.

    // H-pass: input → scratch_d (horizontal intermediate)
    gpu_params.dehaze_filter_radius = GUIDED_RADIUS;
    gpu_params.dehaze_mode = 0.0; // horizontal
    runtime.upload_params(gpu_params);
    dispatch_3buf(
        runtime,
        pipeline,
        input,
        &runtime.scratch_d,
        &runtime.params_buffer,
        "box_h",
        wg_count,
    );

    // V-pass: scratch_d → output
    gpu_params.dehaze_mode = 1.0; // vertical
    runtime.upload_params(gpu_params);
    dispatch_3buf(
        runtime,
        pipeline,
        &runtime.scratch_d,
        output,
        &runtime.params_buffer,
        "box_v",
        wg_count,
    );
}

/// Generic dispatch for 3-binding shaders (input, output, params or similar).
fn dispatch_3buf(
    runtime: &GpuRuntime,
    pipeline: &wgpu::ComputePipeline,
    buf0: &wgpu::Buffer,
    buf1: &wgpu::Buffer,
    buf2: &wgpu::Buffer,
    label: &str,
    wg_count: u32,
) {
    let bind_group = runtime
        .device
        .create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(label),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buf0.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: buf1.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: buf2.as_entire_binding(),
                },
            ],
        });
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
        pass.dispatch_workgroups(wg_count, 1, 1);
    }
    runtime.queue.submit(std::iter::once(encoder.finish()));
}

/// Generic dispatch for 2-binding shaders (input, output).
fn dispatch_2buf(
    runtime: &GpuRuntime,
    pipeline: &wgpu::ComputePipeline,
    buf0: &wgpu::Buffer,
    buf1: &wgpu::Buffer,
    label: &str,
    wg_count: u32,
) {
    let bind_group = runtime
        .device
        .create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(label),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buf0.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: buf1.as_entire_binding(),
                },
            ],
        });
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
        pass.dispatch_workgroups(wg_count, 1, 1);
    }
    runtime.queue.submit(std::iter::once(encoder.finish()));
}

/// Dispatch guided filter coefficient computation (6-binding shader).
#[allow(clippy::too_many_arguments)]
fn dispatch_guided_coeffs(
    runtime: &GpuRuntime,
    pipeline: &wgpu::ComputePipeline,
    mean_g: &wgpu::Buffer,
    mean_p: &wgpu::Buffer,
    mean_gp: &wgpu::Buffer,
    mean_gg: &wgpu::Buffer,
    a_out: &wgpu::Buffer,
    b_out: &wgpu::Buffer,
    wg_count: u32,
) {
    let bind_group = runtime
        .device
        .create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("dehaze_guided_coeffs"),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: mean_g.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: mean_p.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: mean_gp.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: mean_gg.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: a_out.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: b_out.as_entire_binding(),
                },
            ],
        });
    let mut encoder = runtime
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("dehaze_guided_coeffs"),
        });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("dehaze_guided_coeffs"),
            timestamp_writes: None,
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(wg_count, 1, 1);
    }
    runtime.queue.submit(std::iter::once(encoder.finish()));
}

/// Dispatch fused multiply-add: output = a * b + c (4-binding shader).
fn dispatch_fma(
    runtime: &GpuRuntime,
    pipeline: &wgpu::ComputePipeline,
    a: &wgpu::Buffer,
    b: &wgpu::Buffer,
    c: &wgpu::Buffer,
    output: &wgpu::Buffer,
    wg_count: u32,
) {
    let bind_group = runtime
        .device
        .create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("dehaze_fma"),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: a.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: b.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: c.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: output.as_entire_binding(),
                },
            ],
        });
    let mut encoder = runtime
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("dehaze_fma"),
        });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("dehaze_fma"),
            timestamp_writes: None,
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(wg_count, 1, 1);
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
    fn gpu_dehaze_zero_is_identity() {
        if !gpu_available() {
            eprintln!("skipping: no GPU adapter");
            return;
        }

        let width = 16u32;
        let height = 16u32;
        let pixels: Vec<[f32; 3]> = (0..(width * height) as usize)
            .map(|i| {
                let t = i as f32 / (width * height - 1) as f32;
                [0.3 + 0.4 * t, 0.2 + 0.5 * t, 0.1 + 0.3 * t]
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

        let dehaze_params = DehazeParams::default();
        dispatch_dehaze(&runtime, &shaders, &mut gpu_params, &dehaze_params);

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
    fn gpu_dehaze_positive_changes_output() {
        if !gpu_available() {
            eprintln!("skipping: no GPU adapter");
            return;
        }

        let width = 16u32;
        let height = 16u32;
        // Simulated hazy image
        let pixels: Vec<[f32; 3]> = (0..(width * height) as usize)
            .map(|i| {
                let base = 0.5 + 0.3 * (i as f32 / (width * height) as f32);
                [base, base * 0.9, base * 0.85]
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

        let dehaze_params = DehazeParams { amount: 50.0 };
        dispatch_dehaze(&runtime, &shaders, &mut gpu_params, &dehaze_params);

        let result = runtime.download_pixels();
        let differs = result
            .iter()
            .zip(pixels.iter())
            .any(|(r, p)| (r[0] - p[0]).abs() > 1e-4);
        assert!(differs, "positive dehaze should modify image");
    }

    #[test]
    fn gpu_dehaze_negative_adds_fog() {
        if !gpu_available() {
            eprintln!("skipping: no GPU adapter");
            return;
        }

        let width = 16u32;
        let height = 16u32;
        let pixels: Vec<[f32; 3]> = (0..(width * height) as usize)
            .map(|i| {
                let t = i as f32 / (width * height) as f32;
                [0.2 + 0.5 * t, 0.3 + 0.3 * t, 0.1 + 0.4 * t]
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

        let dehaze_params = DehazeParams { amount: -30.0 };
        dispatch_dehaze(&runtime, &shaders, &mut gpu_params, &dehaze_params);

        let result = runtime.download_pixels();
        let differs = result
            .iter()
            .zip(pixels.iter())
            .any(|(r, p)| (r[0] - p[0]).abs() > 1e-4);
        assert!(differs, "negative dehaze should add fog");
    }
}
