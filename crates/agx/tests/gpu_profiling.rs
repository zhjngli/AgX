//! GPU vs CPU profiling comparison tests.
//!
//! Renders through both pipelines with profiling enabled and prints
//! per-stage timing comparison. Run with:
//!   cargo test --features gpu,profiling -p agx gpu_profiling -- --nocapture

#![cfg(all(feature = "gpu", feature = "profiling"))]

use agx::engine::{Engine, Parameters};
use image::{ImageBuffer, Rgb, Rgb32FImage};

fn gpu_available() -> bool {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).is_some()
}

fn make_test_image(w: u32, h: u32) -> Rgb32FImage {
    ImageBuffer::from_fn(w, h, |x, y| {
        let r = (x as f32) / (w as f32);
        let g = (y as f32) / (h as f32);
        let b = ((x + y) as f32 / (w + h) as f32).min(1.0);
        Rgb([r, g, b])
    })
}

fn print_profile(label: &str, profile: &agx::engine::RenderProfile) {
    eprintln!("  {label}:");
    for (name, ms) in &profile.stages {
        eprintln!("    {name:<30} {ms:>8.2} ms");
    }
    eprintln!("    {:<30} {:>8.2} ms", "TOTAL", profile.total_ms);
}

fn profile_both(label: &str, w: u32, h: u32, params: &Parameters) {
    let img = make_test_image(w, h);

    // CPU path
    let mut cpu_engine = Engine::new_cpu(img.clone());
    *cpu_engine.params_mut() = params.clone();
    let cpu_result = cpu_engine.render();
    let cpu_profile = cpu_result.profile.expect("CPU should have profile");

    // GPU path
    let mut gpu_engine = Engine::new_gpu(img).expect("GPU init failed");
    *gpu_engine.params_mut() = params.clone();
    let gpu_result = gpu_engine.render();
    let gpu_profile = gpu_result.profile.expect("GPU should have profile");

    eprintln!(
        "\n=== {label} ({w}x{h} = {} pixels) ===",
        w as u64 * h as u64
    );
    print_profile("CPU", &cpu_profile);
    print_profile("GPU", &gpu_profile);
    eprintln!(
        "  Speedup: {:.2}x (CPU {:.1}ms / GPU {:.1}ms)",
        cpu_profile.total_ms / gpu_profile.total_ms,
        cpu_profile.total_ms,
        gpu_profile.total_ms,
    );
}

fn profile_three(label: &str, w: u32, h: u32, params: &Parameters) {
    let img = make_test_image(w, h);

    // CPU path
    let mut cpu_engine = Engine::new_cpu(img.clone());
    *cpu_engine.params_mut() = params.clone();
    let cpu_result = cpu_engine.render();
    let cpu_profile = cpu_result.profile.expect("CPU should have profile");

    // GPU path
    let mut gpu_engine = Engine::new_gpu(img.clone()).expect("GPU init failed");
    *gpu_engine.params_mut() = params.clone();
    let gpu_result = gpu_engine.render();
    let gpu_profile = gpu_result.profile.expect("GPU should have profile");

    // Fallback path
    let fallback_profile = match Engine::new_gpu_fallback(img) {
        Ok(mut fb_engine) => {
            *fb_engine.params_mut() = params.clone();
            let fb_result = fb_engine.render();
            fb_result.profile
        }
        Err(e) => {
            eprintln!("  Fallback adapter not available: {e}");
            None
        }
    };

    eprintln!(
        "\n=== {label} ({w}x{h} = {} pixels) ===",
        w as u64 * h as u64
    );
    print_profile("CPU", &cpu_profile);
    print_profile("GPU", &gpu_profile);
    if let Some(ref fb) = fallback_profile {
        print_profile("Fallback", fb);
        eprintln!(
            "  GPU vs CPU: {:.2}x | Fallback vs CPU: {:.2}x",
            cpu_profile.total_ms / gpu_profile.total_ms,
            cpu_profile.total_ms / fb.total_ms,
        );
    } else {
        eprintln!(
            "  GPU vs CPU: {:.2}x | Fallback: N/A",
            cpu_profile.total_ms / gpu_profile.total_ms,
        );
    }
}

#[test]
fn profile_fallback_all_stages() {
    if !gpu_available() {
        eprintln!("skipping: no GPU adapter");
        return;
    }
    let mut params = Parameters::default();
    params.exposure = 0.5;
    params.contrast = 25.0;
    params.temperature = 10.0;
    params.detail.sharpening.amount = 30.0;
    params.noise_reduction.luminance = 40.0;
    params.noise_reduction.color = 20.0;
    params.dehaze.amount = 25.0;
    params.grain.amount = 20.0;
    params.grain.size = 50.0;
    params.vignette.amount = -30.0;
    profile_three("Fallback comparison (all stages)", 1024, 768, &params);
}

#[test]
fn probe_adapter_info() {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());

    eprintln!("\n=== GPU Adapter Probe ===");

    // Check for any adapter (hardware or software)
    match pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: None,
        force_fallback_adapter: false,
    })) {
        Some(adapter) => {
            let info = adapter.get_info();
            let limits = adapter.limits();
            eprintln!("  Primary adapter: {} ({:?})", info.name, info.backend);
            eprintln!("  Driver: {}", info.driver);
            eprintln!("  Type: {:?}", info.device_type);
            eprintln!("  Max buffer size: {} MB", limits.max_buffer_size / 1_048_576);
            eprintln!(
                "  Max storage buffer binding: {} MB",
                limits.max_storage_buffer_binding_size / 1_048_576
            );
        }
        None => eprintln!("  No primary adapter found"),
    }

    // Check for fallback adapter
    match pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        compatible_surface: None,
        force_fallback_adapter: true,
    })) {
        Some(adapter) => {
            let info = adapter.get_info();
            let limits = adapter.limits();
            eprintln!("  Fallback adapter: {} ({:?})", info.name, info.backend);
            eprintln!("  Driver: {}", info.driver);
            eprintln!("  Type: {:?}", info.device_type);
            eprintln!("  Max buffer size: {} MB", limits.max_buffer_size / 1_048_576);
            eprintln!(
                "  Max storage buffer binding: {} MB",
                limits.max_storage_buffer_binding_size / 1_048_576
            );
        }
        None => eprintln!("  No fallback adapter found"),
    }
}

#[test]
fn profile_neutral_params() {
    if !gpu_available() {
        eprintln!("skipping: no GPU adapter");
        return;
    }
    profile_both("Neutral params", 1024, 768, &Parameters::default());
}

#[test]
fn profile_basic_adjustments() {
    if !gpu_available() {
        eprintln!("skipping: no GPU adapter");
        return;
    }
    let mut params = Parameters::default();
    params.exposure = 0.5;
    params.contrast = 25.0;
    params.temperature = 10.0;
    params.highlights = -30.0;
    params.shadows = 20.0;
    profile_both("Basic adjustments", 1024, 768, &params);
}

#[test]
fn profile_all_stages() {
    if !gpu_available() {
        eprintln!("skipping: no GPU adapter");
        return;
    }
    let mut params = Parameters::default();
    params.exposure = 0.5;
    params.contrast = 25.0;
    params.temperature = 10.0;
    params.detail.sharpening.amount = 30.0;
    params.noise_reduction.luminance = 40.0;
    params.noise_reduction.color = 20.0;
    params.dehaze.amount = 25.0;
    params.grain.amount = 20.0;
    params.grain.size = 50.0;
    params.vignette.amount = -30.0;
    profile_both("All stages active", 1024, 768, &params);
}

#[test]
fn profile_large_image() {
    if !gpu_available() {
        eprintln!("skipping: no GPU adapter");
        return;
    }
    let mut params = Parameters::default();
    params.exposure = 0.5;
    params.contrast = 25.0;
    params.temperature = 10.0;
    params.detail.sharpening.amount = 30.0;
    params.vignette.amount = -30.0;
    profile_both("Large image (basic)", 4000, 3000, &params);
}

#[test]
fn profile_large_image_all_stages() {
    if !gpu_available() {
        eprintln!("skipping: no GPU adapter");
        return;
    }
    let mut params = Parameters::default();
    params.exposure = 0.5;
    params.contrast = 25.0;
    params.temperature = 10.0;
    params.detail.sharpening.amount = 30.0;
    params.noise_reduction.luminance = 40.0;
    params.noise_reduction.color = 20.0;
    params.dehaze.amount = 25.0;
    params.grain.amount = 20.0;
    params.grain.size = 50.0;
    params.vignette.amount = -30.0;
    profile_both("Large image (all stages)", 4000, 3000, &params);
}
