//! GPU-friendly parameter struct for uploading to uniform buffers.

use crate::adjust::{ColorWheel, VignetteShape};
use crate::engine::Parameters;

/// Flat, repr(C) parameter struct for GPU uniform buffers.
/// All fields are f32 or fixed-size f32 arrays — no enums, Options, or pointers.
/// Field names mirror [`Parameters`] 1:1.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
#[allow(missing_docs)]
pub struct GpuParameters {
    // Linear adjustments
    pub exposure: f32,
    pub temperature: f32,
    pub tint: f32,
    pub _pad0: f32,

    // Gamma adjustments — tone
    pub contrast: f32,
    pub highlights: f32,
    pub shadows: f32,
    pub whites: f32,

    pub blacks: f32,
    pub _pad1: [f32; 3],

    // HSL — 8 channels x 3 values = 24 floats
    pub hue_shifts: [f32; 8],
    pub sat_shifts: [f32; 8],
    pub lum_shifts: [f32; 8],

    // Color grading — 4 wheels x [r_mult, g_mult, b_mult, luminance] + balance
    pub cg_shadow_tint: [f32; 4],
    pub cg_midtone_tint: [f32; 4],
    pub cg_highlight_tint: [f32; 4],
    pub cg_global_tint: [f32; 4],
    pub cg_balance_factor: f32,
    pub cg_balance_active: f32,
    pub _pad2: [f32; 2],

    // Vignette
    pub vignette_amount: f32,
    pub vignette_shape: f32, // 0.0 = elliptical, 1.0 = circular
    pub _pad3: [f32; 2],

    // Dehaze
    pub dehaze_amount: f32,
    pub _pad4: [f32; 3],

    // Grain
    pub grain_amount: f32,
    pub grain_size: f32,
    pub grain_type: f32, // 0.0 = Fine, 1.0 = Silver, 2.0 = Harsh
    pub grain_seed: f32,

    // Image dimensions (needed by vignette, grain, etc.)
    pub width: f32,
    pub height: f32,
    pub _pad5: [f32; 2],
}

impl From<&Parameters> for GpuParameters {
    fn from(p: &Parameters) -> Self {
        let shadow_tint = wheel_to_tint_and_lum(&p.color_grading.shadows);
        let midtone_tint = wheel_to_tint_and_lum(&p.color_grading.midtones);
        let highlight_tint = wheel_to_tint_and_lum(&p.color_grading.highlights);
        let global_tint = wheel_to_tint_and_lum(&p.color_grading.global);

        Self {
            exposure: p.exposure,
            temperature: p.temperature,
            tint: p.tint,
            _pad0: 0.0,
            contrast: p.contrast,
            highlights: p.highlights,
            shadows: p.shadows,
            whites: p.whites,
            blacks: p.blacks,
            _pad1: [0.0; 3],
            hue_shifts: p.hsl.hue_shifts(),
            sat_shifts: p.hsl.saturation_shifts(),
            lum_shifts: p.hsl.luminance_shifts(),
            cg_shadow_tint: shadow_tint,
            cg_midtone_tint: midtone_tint,
            cg_highlight_tint: highlight_tint,
            cg_global_tint: global_tint,
            cg_balance_factor: 2.0_f32.powf(-p.color_grading.balance / 100.0),
            cg_balance_active: if p.color_grading.balance != 0.0 {
                1.0
            } else {
                0.0
            },
            _pad2: [0.0; 2],
            vignette_amount: p.vignette.amount,
            vignette_shape: match p.vignette.shape {
                VignetteShape::Elliptical => 0.0,
                VignetteShape::Circular => 1.0,
            },
            _pad3: [0.0; 2],
            dehaze_amount: p.dehaze.amount,
            _pad4: [0.0; 3],
            grain_amount: p.grain.amount,
            grain_size: p.grain.size,
            grain_type: match p.grain.grain_type {
                crate::adjust::grain::GrainType::Fine => 0.0,
                crate::adjust::grain::GrainType::Silver => 1.0,
                crate::adjust::grain::GrainType::Harsh => 2.0,
            },
            grain_seed: 0.0,
            width: 0.0,
            height: 0.0,
            _pad5: [0.0; 2],
        }
    }
}

fn wheel_to_tint_and_lum(wheel: &ColorWheel) -> [f32; 4] {
    let hue_rad = wheel.hue * std::f32::consts::PI / 180.0;
    let sat = wheel.saturation / 100.0;
    [
        1.0 + sat * hue_rad.cos(),
        1.0 + sat * (hue_rad - 2.0 * std::f32::consts::PI / 3.0).cos(),
        1.0 + sat * (hue_rad - 4.0 * std::f32::consts::PI / 3.0).cos(),
        wheel.luminance / 100.0,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gpu_params_is_pod() {
        let p = Parameters::default();
        let gpu: GpuParameters = (&p).into();
        let _bytes: &[u8] = bytemuck::bytes_of(&gpu);
    }

    #[test]
    fn gpu_params_default_values() {
        let p = Parameters::default();
        let gpu: GpuParameters = (&p).into();
        assert_eq!(gpu.exposure, 0.0);
        assert_eq!(gpu.contrast, 0.0);
        assert_eq!(gpu.temperature, 0.0);
        assert_eq!(gpu.vignette_amount, 0.0);
        assert_eq!(gpu.dehaze_amount, 0.0);
        assert_eq!(gpu.grain_amount, 0.0);
    }

    #[test]
    fn gpu_params_size_is_16_aligned() {
        // WGSL uniform buffers require 16-byte alignment
        assert_eq!(std::mem::size_of::<GpuParameters>() % 16, 0);
    }
}
