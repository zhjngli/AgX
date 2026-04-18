//! Cross-path consistency tests: run identical params through CpuPipeline and
//! GpuPipeline, assert near-identical output.
//!
//! These tests only compile and run when the `gpu` feature is enabled.
//! Each test renders through both pipelines and compares pixel values
//! with a per-test tolerance.

#![cfg(feature = "gpu")]

use agx::engine::gpu::GpuPipeline;
use agx::engine::pipeline::CpuPipeline;
use agx::engine::Parameters;
use image::{ImageBuffer, Rgb, Rgb32FImage};

fn gpu_available() -> bool {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).is_some()
}

fn make_gradient(w: u32, h: u32) -> Rgb32FImage {
    ImageBuffer::from_fn(w, h, |x, y| {
        let r = (x as f32) / (w as f32);
        let g = (y as f32) / (h as f32);
        let b = 0.5;
        Rgb([r, g, b])
    })
}

/// Render through both CPU and GPU pipelines, return (cpu_image, gpu_image).
fn render_both(
    img: &Rgb32FImage,
    params: &Parameters,
    lut: Option<&agx::lut::Lut3D>,
) -> (Rgb32FImage, Rgb32FImage) {
    let (w, h) = img.dimensions();
    let mut cpu = CpuPipeline::new();
    let cpu_result = cpu.execute(img, params, lut);
    let mut gpu = GpuPipeline::new(w, h).unwrap();
    let gpu_result = gpu.execute(img, params, lut);
    (cpu_result.image, gpu_result.image)
}

fn assert_near_identical(cpu: &Rgb32FImage, gpu: &Rgb32FImage, tolerance: f32) {
    assert_eq!(cpu.dimensions(), gpu.dimensions());
    let (w, h) = cpu.dimensions();
    let mut max_diff: f32 = 0.0;
    for y in 0..h {
        for x in 0..w {
            let cp = cpu.get_pixel(x, y);
            let gp = gpu.get_pixel(x, y);
            for c in 0..3 {
                let diff = (cp.0[c] - gp.0[c]).abs();
                max_diff = max_diff.max(diff);
                assert!(
                    diff <= tolerance,
                    "pixel[{x},{y}][{c}]: cpu={}, gpu={}, diff={diff} (tol={tolerance})",
                    cp.0[c],
                    gp.0[c]
                );
            }
        }
    }
    eprintln!("max cross-path diff: {max_diff:.6}");
}

#[test]
fn neutral_params() {
    if !gpu_available() {
        return;
    }
    let img = make_gradient(16, 16);
    let params = Parameters::default();
    let (cpu, gpu) = render_both(&img, &params, None);
    assert_near_identical(&cpu, &gpu, 0.00001);
}

#[test]
fn exposure_and_contrast() {
    if !gpu_available() {
        return;
    }
    let img = make_gradient(16, 16);
    let params = Parameters {
        exposure: 1.5,
        contrast: 40.0,
        highlights: -30.0,
        shadows: 20.0,
        ..Default::default()
    };
    let (cpu, gpu) = render_both(&img, &params, None);
    assert_near_identical(&cpu, &gpu, 0.00001);
}

#[test]
fn white_balance() {
    if !gpu_available() {
        return;
    }
    let img = make_gradient(16, 16);
    let params = Parameters {
        temperature: 30.0,
        tint: -15.0,
        ..Default::default()
    };
    let (cpu, gpu) = render_both(&img, &params, None);
    assert_near_identical(&cpu, &gpu, 0.00001);
}

#[test]
fn vignette() {
    if !gpu_available() {
        return;
    }
    let img = make_gradient(16, 16);
    let mut params = Parameters::default();
    params.vignette.amount = -50.0;
    let (cpu, gpu) = render_both(&img, &params, None);
    assert_near_identical(&cpu, &gpu, 0.00001);
}

#[test]
fn detail_sharpening() {
    if !gpu_available() {
        return;
    }
    let img = make_gradient(32, 32);
    let mut params = Parameters::default();
    params.detail.sharpening.amount = 60.0;
    let (cpu, gpu) = render_both(&img, &params, None);
    assert_near_identical(&cpu, &gpu, 0.00001);
}

#[test]
fn detail_clarity() {
    if !gpu_available() {
        return;
    }
    let img = make_gradient(32, 32);
    let mut params = Parameters::default();
    params.detail.clarity = 40.0;
    let (cpu, gpu) = render_both(&img, &params, None);
    assert_near_identical(&cpu, &gpu, 0.00001);
}

#[test]
fn denoise_luminance() {
    if !gpu_available() {
        return;
    }
    let img = make_gradient(32, 32);
    let mut params = Parameters::default();
    params.noise_reduction.luminance = 50.0;
    let (cpu, gpu) = render_both(&img, &params, None);
    assert_near_identical(&cpu, &gpu, 0.00001);
}

#[test]
fn denoise_color() {
    if !gpu_available() {
        return;
    }
    let img = make_gradient(32, 32);
    let mut params = Parameters::default();
    params.noise_reduction.color = 50.0;
    let (cpu, gpu) = render_both(&img, &params, None);
    assert_near_identical(&cpu, &gpu, 0.00001);
}

#[test]
fn dehaze_positive() {
    if !gpu_available() {
        return;
    }
    let img = make_gradient(32, 32);
    let mut params = Parameters::default();
    params.dehaze.amount = 30.0;
    let (cpu, gpu) = render_both(&img, &params, None);
    assert_near_identical(&cpu, &gpu, 0.00001);
}

#[test]
fn dehaze_negative() {
    if !gpu_available() {
        return;
    }
    let img = make_gradient(32, 32);
    let mut params = Parameters::default();
    params.dehaze.amount = -30.0;
    let (cpu, gpu) = render_both(&img, &params, None);
    assert_near_identical(&cpu, &gpu, 0.00001);
}

#[test]
fn combined_adjustments() {
    if !gpu_available() {
        return;
    }
    let img = make_gradient(32, 32);
    let mut params = Parameters {
        exposure: 0.8,
        contrast: 20.0,
        highlights: -20.0,
        shadows: 15.0,
        whites: 10.0,
        blacks: -5.0,
        ..Default::default()
    };
    params.vignette.amount = -30.0;
    params.detail.sharpening.amount = 40.0;
    let (cpu, gpu) = render_both(&img, &params, None);
    assert_near_identical(&cpu, &gpu, 0.00001);
}
