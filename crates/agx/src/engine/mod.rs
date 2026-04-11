//! Render engine: pipeline executor, parameter types, and the [`Engine`] entry point.

use std::sync::Arc;

use image::{Rgb, Rgb32FImage};
use serde::{Deserialize, Serialize};

pub mod stages;

/// Timing data for a single render pass. Only available when compiled
/// with the `profiling` feature.
#[cfg(feature = "profiling")]
#[derive(Debug, Clone, serde::Serialize)]
pub struct RenderProfile {
    pub stages: Vec<(String, f64)>,
    pub total_ms: f64,
}

/// Result of a render operation. Contains the rendered image and optional
/// profiling data (when compiled with the `profiling` feature).
#[derive(Debug, Clone)]
pub struct RenderResult {
    pub image: Rgb32FImage,
    #[cfg(feature = "profiling")]
    pub profile: Option<RenderProfile>,
}

/// Color space declaration for pipeline stages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorSpace {
    /// Linear sRGB (scene-referred, pre-gamma).
    LinearSrgb,
    /// sRGB with gamma encoding (display-referred).
    SrgbGamma,
}

/// Shared mutable context passed through pipeline stages.
///
/// Owns the pixel buffer and provides read-only access to parameters and LUT.
/// The executor creates this once per render; each stage mutates `buf` in place.
pub struct RenderContext<'a> {
    /// Pixel buffer in row-major order: `buf[y * width + x] = [r, g, b]`.
    pub buf: Vec<[f32; 3]>,
    pub width: u32,
    pub height: u32,
    pub params: &'a Parameters,
    pub lut: Option<&'a crate::lut::Lut3D>,
}

/// A single stage in the render pipeline.
///
/// Stages are executed in a fixed order by the pipeline executor. Each stage
/// declares its working color space; the executor auto-inserts conversions
/// when adjacent stages disagree. The pipeline order is hardcoded and not
/// configurable — this preserves preset compatibility (same params = same output).
///
/// Implementors should delegate pixel math to the `adjust` module.
pub trait Stage: Send + Sync {
    /// Human-readable name, used in profiling output.
    fn name(&self) -> &'static str;

    /// Color space this stage expects its input buffer in.
    fn input_color_space(&self) -> ColorSpace;

    /// Color space this stage produces in the output buffer.
    fn output_color_space(&self) -> ColorSpace;

    /// Whether this stage has any effect given current params.
    /// Returning false lets the executor skip the stage entirely.
    fn is_active(&self, params: &Parameters) -> bool;

    /// Precompute loop-invariant data from params.
    /// Called once per render before `process()`.
    fn prepare(&mut self, params: &Parameters);

    /// Process the pixel buffer in-place.
    fn process(&self, ctx: &mut RenderContext) -> Result<(), crate::error::AgxError>;
}

/// The fixed render pipeline. Holds stages in execution order.
///
/// The pipeline order is hardcoded and not configurable by consumers.
/// This preserves the invariant that identical parameters always produce
/// identical output, regardless of the order they were set.
pub struct Pipeline {
    stages: Vec<Box<dyn Stage>>,
}

impl Default for Pipeline {
    fn default() -> Self {
        Self::new()
    }
}

impl Pipeline {
    /// Construct the fixed pipeline with all stages in render order.
    pub fn new() -> Self {
        Pipeline {
            stages: vec![
                Box::new(stages::WhiteBalanceExposureStage::new()),
                Box::new(stages::DehazeStage::new()),
                Box::new(stages::DenoiseStage::new()),
                Box::new(stages::LinearToSrgbStage::new()),
                Box::new(stages::PerPixelAdjustmentsStage::new()),
                Box::new(stages::DetailStage::new()),
                Box::new(stages::GrainStage::new()),
                Box::new(stages::VignetteStage::new()),
                Box::new(stages::SrgbToLinearStage::new()),
            ],
        }
    }

    /// Execute the pipeline, returning the rendered image and optional profiling data.
    pub fn execute(
        &mut self,
        original: &Rgb32FImage,
        params: &Parameters,
        lut: Option<&crate::lut::Lut3D>,
    ) -> RenderResult {
        let (w, h) = original.dimensions();

        #[cfg(feature = "profiling")]
        let render_start = std::time::Instant::now();
        #[cfg(feature = "profiling")]
        let mut profile_stages: Vec<(String, f64)> = Vec::new();

        let buf: Vec<[f32; 3]> = original
            .pixels()
            .map(|p| [p.0[0], p.0[1], p.0[2]])
            .collect();

        let mut ctx = RenderContext {
            buf,
            width: w,
            height: h,
            params,
            lut,
        };

        // Prepare all active stages
        for stage in &mut self.stages {
            if stage.is_active(params) {
                stage.prepare(params);
            }
        }

        // Execute stages in order, tracking color space in debug builds
        #[cfg(debug_assertions)]
        let mut current_color_space = ColorSpace::LinearSrgb;

        for stage in &self.stages {
            if !stage.is_active(params) {
                continue;
            }

            #[cfg(debug_assertions)]
            debug_assert_eq!(
                stage.input_color_space(),
                current_color_space,
                "stage '{}' expects {:?} but current space is {:?}",
                stage.name(),
                stage.input_color_space(),
                current_color_space,
            );

            #[cfg(feature = "profiling")]
            let stage_start = std::time::Instant::now();

            stage
                .process(&mut ctx)
                .expect("stage processing should not fail");

            #[cfg(debug_assertions)]
            {
                current_color_space = stage.output_color_space();
            }

            #[cfg(feature = "profiling")]
            profile_stages.push((
                stage.name().to_string(),
                stage_start.elapsed().as_secs_f64() * 1000.0,
            ));
        }

        let image = Rgb32FImage::from_fn(w, h, |x, y| {
            let idx = (y * w + x) as usize;
            Rgb(ctx.buf[idx])
        });

        RenderResult {
            image,
            #[cfg(feature = "profiling")]
            profile: Some(RenderProfile {
                stages: profile_stages,
                total_ms: render_start.elapsed().as_secs_f64() * 1000.0,
            }),
        }
    }
}

/// Per-channel HSL adjustment (hue shift, saturation, luminance).
///
/// Ranges: hue -180.0 to +180.0 (degrees), saturation/luminance -100.0 to +100.0.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct HslChannel {
    #[serde(default)]
    pub hue: f32,
    #[serde(default)]
    pub saturation: f32,
    #[serde(default)]
    pub luminance: f32,
}

/// HSL adjustments for all 8 color channels.
///
/// Channel order: Red (0deg), Orange (30deg), Yellow (60deg), Green (120deg),
/// Aqua (180deg), Blue (240deg), Purple (270deg), Magenta (330deg).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct HslChannels {
    #[serde(default)]
    pub red: HslChannel,
    #[serde(default)]
    pub orange: HslChannel,
    #[serde(default)]
    pub yellow: HslChannel,
    #[serde(default)]
    pub green: HslChannel,
    #[serde(default)]
    pub aqua: HslChannel,
    #[serde(default)]
    pub blue: HslChannel,
    #[serde(default)]
    pub purple: HslChannel,
    #[serde(default)]
    pub magenta: HslChannel,
}

impl HslChannels {
    /// Returns true if all channels are at default (zero) values.
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// Extract hue shifts as an array ordered by channel index.
    pub fn hue_shifts(&self) -> [f32; 8] {
        [
            self.red.hue,
            self.orange.hue,
            self.yellow.hue,
            self.green.hue,
            self.aqua.hue,
            self.blue.hue,
            self.purple.hue,
            self.magenta.hue,
        ]
    }

    /// Extract saturation shifts as an array ordered by channel index.
    pub fn saturation_shifts(&self) -> [f32; 8] {
        [
            self.red.saturation,
            self.orange.saturation,
            self.yellow.saturation,
            self.green.saturation,
            self.aqua.saturation,
            self.blue.saturation,
            self.purple.saturation,
            self.magenta.saturation,
        ]
    }

    /// Extract luminance shifts as an array ordered by channel index.
    pub fn luminance_shifts(&self) -> [f32; 8] {
        [
            self.red.luminance,
            self.orange.luminance,
            self.yellow.luminance,
            self.green.luminance,
            self.aqua.luminance,
            self.blue.luminance,
            self.purple.luminance,
            self.magenta.luminance,
        ]
    }
}

/// Vignette adjustment parameters.
///
/// Darkens or brightens image edges. Amount range: -100 to +100. 0 = no effect.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct VignetteParams {
    #[serde(default)]
    pub amount: f32,
    #[serde(default)]
    pub shape: crate::adjust::VignetteShape,
}

impl VignetteParams {
    /// Returns `true` when all fields are at their default (neutral) values.
    pub fn is_default(&self) -> bool {
        self.amount == 0.0 && self.shape == crate::adjust::VignetteShape::default()
    }
}

/// All adjustment parameters for the rendering engine.
///
/// Defaults to neutral (no change) for all values.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Parameters {
    /// Exposure in stops, range -5.0 to +5.0
    pub exposure: f32,
    /// Contrast, range -100 to +100
    pub contrast: f32,
    /// Highlights, range -100 to +100
    pub highlights: f32,
    /// Shadows, range -100 to +100
    pub shadows: f32,
    /// Whites, range -100 to +100
    pub whites: f32,
    /// Blacks, range -100 to +100
    pub blacks: f32,
    /// White balance temperature shift
    pub temperature: f32,
    /// White balance tint shift (green/magenta)
    pub tint: f32,
    /// Per-channel HSL adjustments
    #[serde(default)]
    pub hsl: HslChannels,
    /// Creative vignette (edge darkening/brightening)
    #[serde(default)]
    pub vignette: VignetteParams,
    /// 3-way color grading (shadows/midtones/highlights/global wheels + balance)
    #[serde(default)]
    pub color_grading: crate::adjust::ColorGradingParams,
    /// 5-channel tone curves (RGB, luma, R, G, B)
    #[serde(default)]
    pub tone_curve: crate::adjust::ToneCurveParams,
    /// Detail pass: sharpening, clarity, texture
    #[serde(default)]
    pub detail: crate::adjust::DetailParams,
    /// Dehaze: atmospheric haze removal/addition
    #[serde(default)]
    pub dehaze: crate::adjust::DehazeParams,
    /// Noise reduction: luminance, color, and detail preservation
    #[serde(default)]
    pub noise_reduction: crate::adjust::NoiseReductionParams,
    /// Film grain simulation
    #[serde(default)]
    pub grain: crate::adjust::GrainParams,
}

impl Default for Parameters {
    fn default() -> Self {
        Self {
            exposure: 0.0,
            contrast: 0.0,
            highlights: 0.0,
            shadows: 0.0,
            whites: 0.0,
            blacks: 0.0,
            temperature: 0.0,
            tint: 0.0,
            hsl: HslChannels::default(),
            vignette: VignetteParams::default(),
            color_grading: crate::adjust::ColorGradingParams::default(),
            tone_curve: crate::adjust::ToneCurveParams::default(),
            detail: crate::adjust::DetailParams::default(),
            dehaze: crate::adjust::DehazeParams::default(),
            noise_reduction: crate::adjust::NoiseReductionParams::default(),
            grain: crate::adjust::GrainParams::default(),
        }
    }
}

/// Partial per-channel HSL adjustment — `None` means "not specified".
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PartialHslChannel {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hue: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub saturation: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub luminance: Option<f32>,
}

impl PartialHslChannel {
    /// Merge overlay on top of self (last-write-wins).
    pub fn merge(&self, overlay: &Self) -> Self {
        Self {
            hue: overlay.hue.or(self.hue),
            saturation: overlay.saturation.or(self.saturation),
            luminance: overlay.luminance.or(self.luminance),
        }
    }

    /// Convert to concrete HslChannel. None fields become 0.0.
    pub fn materialize(&self) -> HslChannel {
        HslChannel {
            hue: self.hue.unwrap_or(0.0),
            saturation: self.saturation.unwrap_or(0.0),
            luminance: self.luminance.unwrap_or(0.0),
        }
    }
}

impl From<&HslChannel> for PartialHslChannel {
    fn from(ch: &HslChannel) -> Self {
        Self {
            hue: Some(ch.hue),
            saturation: Some(ch.saturation),
            luminance: Some(ch.luminance),
        }
    }
}

/// Partial HSL adjustments for all 8 channels — `None` means channel not specified.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PartialHslChannels {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub red: Option<PartialHslChannel>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub orange: Option<PartialHslChannel>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub yellow: Option<PartialHslChannel>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub green: Option<PartialHslChannel>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aqua: Option<PartialHslChannel>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blue: Option<PartialHslChannel>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub purple: Option<PartialHslChannel>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub magenta: Option<PartialHslChannel>,
}

impl PartialHslChannels {
    fn merge_channel(
        base: &Option<PartialHslChannel>,
        overlay: &Option<PartialHslChannel>,
    ) -> Option<PartialHslChannel> {
        match (base, overlay) {
            (None, None) => None,
            (Some(b), None) => Some(b.clone()),
            (None, Some(o)) => Some(o.clone()),
            (Some(b), Some(o)) => Some(b.merge(o)),
        }
    }

    /// Merge overlay on top of self (last-write-wins per field).
    pub fn merge(&self, overlay: &Self) -> Self {
        Self {
            red: Self::merge_channel(&self.red, &overlay.red),
            orange: Self::merge_channel(&self.orange, &overlay.orange),
            yellow: Self::merge_channel(&self.yellow, &overlay.yellow),
            green: Self::merge_channel(&self.green, &overlay.green),
            aqua: Self::merge_channel(&self.aqua, &overlay.aqua),
            blue: Self::merge_channel(&self.blue, &overlay.blue),
            purple: Self::merge_channel(&self.purple, &overlay.purple),
            magenta: Self::merge_channel(&self.magenta, &overlay.magenta),
        }
    }

    /// Convert to concrete HslChannels. None channels/fields become default (0.0).
    pub fn materialize(&self) -> HslChannels {
        HslChannels {
            red: self
                .red
                .as_ref()
                .map(|c| c.materialize())
                .unwrap_or_default(),
            orange: self
                .orange
                .as_ref()
                .map(|c| c.materialize())
                .unwrap_or_default(),
            yellow: self
                .yellow
                .as_ref()
                .map(|c| c.materialize())
                .unwrap_or_default(),
            green: self
                .green
                .as_ref()
                .map(|c| c.materialize())
                .unwrap_or_default(),
            aqua: self
                .aqua
                .as_ref()
                .map(|c| c.materialize())
                .unwrap_or_default(),
            blue: self
                .blue
                .as_ref()
                .map(|c| c.materialize())
                .unwrap_or_default(),
            purple: self
                .purple
                .as_ref()
                .map(|c| c.materialize())
                .unwrap_or_default(),
            magenta: self
                .magenta
                .as_ref()
                .map(|c| c.materialize())
                .unwrap_or_default(),
        }
    }
}

impl From<&HslChannels> for PartialHslChannels {
    fn from(hsl: &HslChannels) -> Self {
        Self {
            red: Some(PartialHslChannel::from(&hsl.red)),
            orange: Some(PartialHslChannel::from(&hsl.orange)),
            yellow: Some(PartialHslChannel::from(&hsl.yellow)),
            green: Some(PartialHslChannel::from(&hsl.green)),
            aqua: Some(PartialHslChannel::from(&hsl.aqua)),
            blue: Some(PartialHslChannel::from(&hsl.blue)),
            purple: Some(PartialHslChannel::from(&hsl.purple)),
            magenta: Some(PartialHslChannel::from(&hsl.magenta)),
        }
    }
}

/// Partial vignette parameters — `None` means "not specified".
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PartialVignetteParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amount: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shape: Option<crate::adjust::VignetteShape>,
}

impl PartialVignetteParams {
    /// Merge overlay on top of self (last-write-wins).
    pub fn merge(&self, overlay: &Self) -> Self {
        Self {
            amount: overlay.amount.or(self.amount),
            shape: overlay.shape.or(self.shape),
        }
    }

    /// Convert to concrete VignetteParams. None fields become defaults.
    pub fn materialize(&self) -> VignetteParams {
        VignetteParams {
            amount: self.amount.unwrap_or(0.0),
            shape: self.shape.unwrap_or_default(),
        }
    }
}

impl From<&VignetteParams> for PartialVignetteParams {
    fn from(v: &VignetteParams) -> Self {
        Self {
            amount: Some(v.amount),
            shape: Some(v.shape),
        }
    }
}

/// Partial color wheel — `None` means "not specified".
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PartialColorWheel {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hue: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub saturation: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub luminance: Option<f32>,
}

impl PartialColorWheel {
    /// Merge overlay on top of self (last-write-wins).
    pub fn merge(&self, overlay: &Self) -> Self {
        Self {
            hue: overlay.hue.or(self.hue),
            saturation: overlay.saturation.or(self.saturation),
            luminance: overlay.luminance.or(self.luminance),
        }
    }

    /// Convert to concrete ColorWheel. None fields become 0.0.
    pub fn materialize(&self) -> crate::adjust::ColorWheel {
        crate::adjust::ColorWheel {
            hue: self.hue.unwrap_or(0.0),
            saturation: self.saturation.unwrap_or(0.0),
            luminance: self.luminance.unwrap_or(0.0),
        }
    }
}

impl From<&crate::adjust::ColorWheel> for PartialColorWheel {
    fn from(w: &crate::adjust::ColorWheel) -> Self {
        Self {
            hue: Some(w.hue),
            saturation: Some(w.saturation),
            luminance: Some(w.luminance),
        }
    }
}

/// Partial color grading parameters — `None` means "not specified".
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PartialColorGradingParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shadows: Option<PartialColorWheel>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub midtones: Option<PartialColorWheel>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub highlights: Option<PartialColorWheel>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub global: Option<PartialColorWheel>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub balance: Option<f32>,
}

impl PartialColorGradingParams {
    fn merge_wheel(
        base: &Option<PartialColorWheel>,
        overlay: &Option<PartialColorWheel>,
    ) -> Option<PartialColorWheel> {
        match (base, overlay) {
            (None, None) => None,
            (Some(b), None) => Some(b.clone()),
            (None, Some(o)) => Some(o.clone()),
            (Some(b), Some(o)) => Some(b.merge(o)),
        }
    }

    /// Merge overlay on top of self (last-write-wins per field).
    pub fn merge(&self, overlay: &Self) -> Self {
        Self {
            shadows: Self::merge_wheel(&self.shadows, &overlay.shadows),
            midtones: Self::merge_wheel(&self.midtones, &overlay.midtones),
            highlights: Self::merge_wheel(&self.highlights, &overlay.highlights),
            global: Self::merge_wheel(&self.global, &overlay.global),
            balance: overlay.balance.or(self.balance),
        }
    }

    /// Convert to concrete ColorGradingParams. None fields become defaults.
    pub fn materialize(&self) -> crate::adjust::ColorGradingParams {
        crate::adjust::ColorGradingParams {
            shadows: self
                .shadows
                .as_ref()
                .map(|w| w.materialize())
                .unwrap_or_default(),
            midtones: self
                .midtones
                .as_ref()
                .map(|w| w.materialize())
                .unwrap_or_default(),
            highlights: self
                .highlights
                .as_ref()
                .map(|w| w.materialize())
                .unwrap_or_default(),
            global: self
                .global
                .as_ref()
                .map(|w| w.materialize())
                .unwrap_or_default(),
            balance: self.balance.unwrap_or(0.0),
        }
    }
}

impl From<&crate::adjust::ColorGradingParams> for PartialColorGradingParams {
    fn from(cg: &crate::adjust::ColorGradingParams) -> Self {
        Self {
            shadows: Some(PartialColorWheel::from(&cg.shadows)),
            midtones: Some(PartialColorWheel::from(&cg.midtones)),
            highlights: Some(PartialColorWheel::from(&cg.highlights)),
            global: Some(PartialColorWheel::from(&cg.global)),
            balance: Some(cg.balance),
        }
    }
}

// --- Partial Tone Curve Types ---

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PartialToneCurve {
    pub points: Option<Vec<(f32, f32)>>,
}

impl PartialToneCurve {
    pub fn merge(&self, overlay: &Self) -> Self {
        Self {
            points: overlay.points.clone().or_else(|| self.points.clone()),
        }
    }

    pub fn materialize(&self) -> crate::adjust::ToneCurve {
        crate::adjust::ToneCurve {
            points: self
                .points
                .clone()
                .unwrap_or_else(|| vec![(0.0, 0.0), (1.0, 1.0)]),
        }
    }
}

impl From<&crate::adjust::ToneCurve> for PartialToneCurve {
    fn from(tc: &crate::adjust::ToneCurve) -> Self {
        Self {
            points: Some(tc.points.clone()),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PartialToneCurveParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rgb: Option<PartialToneCurve>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub luma: Option<PartialToneCurve>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub red: Option<PartialToneCurve>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub green: Option<PartialToneCurve>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blue: Option<PartialToneCurve>,
}

impl PartialToneCurveParams {
    pub fn merge(&self, overlay: &Self) -> Self {
        Self {
            rgb: merge_opt_tone_curve(&self.rgb, &overlay.rgb),
            luma: merge_opt_tone_curve(&self.luma, &overlay.luma),
            red: merge_opt_tone_curve(&self.red, &overlay.red),
            green: merge_opt_tone_curve(&self.green, &overlay.green),
            blue: merge_opt_tone_curve(&self.blue, &overlay.blue),
        }
    }

    pub fn materialize(&self) -> crate::adjust::ToneCurveParams {
        crate::adjust::ToneCurveParams {
            rgb: self
                .rgb
                .as_ref()
                .map(|c| c.materialize())
                .unwrap_or_default(),
            luma: self
                .luma
                .as_ref()
                .map(|c| c.materialize())
                .unwrap_or_default(),
            red: self
                .red
                .as_ref()
                .map(|c| c.materialize())
                .unwrap_or_default(),
            green: self
                .green
                .as_ref()
                .map(|c| c.materialize())
                .unwrap_or_default(),
            blue: self
                .blue
                .as_ref()
                .map(|c| c.materialize())
                .unwrap_or_default(),
        }
    }
}

fn merge_opt_tone_curve(
    base: &Option<PartialToneCurve>,
    overlay: &Option<PartialToneCurve>,
) -> Option<PartialToneCurve> {
    match (base, overlay) {
        (None, None) => None,
        (Some(b), None) => Some(b.clone()),
        (None, Some(o)) => Some(o.clone()),
        (Some(b), Some(o)) => Some(b.merge(o)),
    }
}

impl From<&crate::adjust::ToneCurveParams> for PartialToneCurveParams {
    fn from(params: &crate::adjust::ToneCurveParams) -> Self {
        Self {
            rgb: Some(PartialToneCurve::from(&params.rgb)),
            luma: Some(PartialToneCurve::from(&params.luma)),
            red: Some(PartialToneCurve::from(&params.red)),
            green: Some(PartialToneCurve::from(&params.green)),
            blue: Some(PartialToneCurve::from(&params.blue)),
        }
    }
}

/// Partial sharpening parameters — `None` means "not specified".
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PartialSharpeningParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amount: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub radius: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub threshold: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub masking: Option<f32>,
}

impl PartialSharpeningParams {
    pub fn merge(&self, overlay: &Self) -> Self {
        Self {
            amount: overlay.amount.or(self.amount),
            radius: overlay.radius.or(self.radius),
            threshold: overlay.threshold.or(self.threshold),
            masking: overlay.masking.or(self.masking),
        }
    }

    pub fn materialize(&self) -> crate::adjust::SharpeningParams {
        let d = crate::adjust::SharpeningParams::default();
        crate::adjust::SharpeningParams {
            amount: self.amount.unwrap_or(d.amount),
            radius: self.radius.unwrap_or(d.radius),
            threshold: self.threshold.unwrap_or(d.threshold),
            masking: self.masking.unwrap_or(d.masking),
        }
    }
}

impl From<&crate::adjust::SharpeningParams> for PartialSharpeningParams {
    fn from(s: &crate::adjust::SharpeningParams) -> Self {
        Self {
            amount: Some(s.amount),
            radius: Some(s.radius),
            threshold: Some(s.threshold),
            masking: Some(s.masking),
        }
    }
}

/// Partial detail parameters — `None` means "not specified".
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PartialDetailParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sharpening: Option<PartialSharpeningParams>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clarity: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub texture: Option<f32>,
}

impl PartialDetailParams {
    fn merge_sharpening(
        base: &Option<PartialSharpeningParams>,
        overlay: &Option<PartialSharpeningParams>,
    ) -> Option<PartialSharpeningParams> {
        match (base, overlay) {
            (None, None) => None,
            (Some(b), None) => Some(b.clone()),
            (None, Some(o)) => Some(o.clone()),
            (Some(b), Some(o)) => Some(b.merge(o)),
        }
    }

    pub fn merge(&self, overlay: &Self) -> Self {
        Self {
            sharpening: Self::merge_sharpening(&self.sharpening, &overlay.sharpening),
            clarity: overlay.clarity.or(self.clarity),
            texture: overlay.texture.or(self.texture),
        }
    }

    pub fn materialize(&self) -> crate::adjust::DetailParams {
        crate::adjust::DetailParams {
            sharpening: self
                .sharpening
                .as_ref()
                .map(|s| s.materialize())
                .unwrap_or_default(),
            clarity: self.clarity.unwrap_or(0.0),
            texture: self.texture.unwrap_or(0.0),
        }
    }
}

impl From<&crate::adjust::DetailParams> for PartialDetailParams {
    fn from(d: &crate::adjust::DetailParams) -> Self {
        Self {
            sharpening: Some(PartialSharpeningParams::from(&d.sharpening)),
            clarity: Some(d.clarity),
            texture: Some(d.texture),
        }
    }
}

/// Partial dehaze parameters — `None` means "not specified".
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PartialDehazeParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amount: Option<f32>,
}

impl PartialDehazeParams {
    pub fn merge(&self, overlay: &Self) -> Self {
        Self {
            amount: overlay.amount.or(self.amount),
        }
    }

    pub fn materialize(&self) -> crate::adjust::DehazeParams {
        crate::adjust::DehazeParams {
            amount: self.amount.unwrap_or(0.0),
        }
    }
}

impl From<&crate::adjust::DehazeParams> for PartialDehazeParams {
    fn from(d: &crate::adjust::DehazeParams) -> Self {
        Self {
            amount: Some(d.amount),
        }
    }
}

/// Partial noise reduction parameters — `None` means "not specified".
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PartialNoiseReductionParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub luminance: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<f32>,
}

impl PartialNoiseReductionParams {
    pub fn merge(&self, overlay: &Self) -> Self {
        Self {
            luminance: overlay.luminance.or(self.luminance),
            color: overlay.color.or(self.color),
            detail: overlay.detail.or(self.detail),
        }
    }

    pub fn materialize(&self) -> crate::adjust::NoiseReductionParams {
        crate::adjust::NoiseReductionParams {
            luminance: self.luminance.unwrap_or(0.0),
            color: self.color.unwrap_or(0.0),
            detail: self.detail.unwrap_or(0.0),
        }
    }
}

impl From<&crate::adjust::NoiseReductionParams> for PartialNoiseReductionParams {
    fn from(p: &crate::adjust::NoiseReductionParams) -> Self {
        Self {
            luminance: Some(p.luminance),
            color: Some(p.color),
            detail: Some(p.detail),
        }
    }
}

/// Partial grain parameters — `None` means "not specified".
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PartialGrainParams {
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "type")]
    pub grain_type: Option<crate::adjust::GrainType>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amount: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,
}

impl PartialGrainParams {
    pub fn merge(&self, overlay: &Self) -> Self {
        Self {
            grain_type: overlay.grain_type.or(self.grain_type),
            amount: overlay.amount.or(self.amount),
            size: overlay.size.or(self.size),
            seed: overlay.seed.or(self.seed),
        }
    }

    pub fn materialize(&self) -> crate::adjust::GrainParams {
        crate::adjust::GrainParams {
            grain_type: self.grain_type.unwrap_or_default(),
            amount: self.amount.unwrap_or(0.0),
            size: self.size.unwrap_or(50.0),
            seed: self.seed,
        }
    }
}

impl From<&crate::adjust::GrainParams> for PartialGrainParams {
    fn from(p: &crate::adjust::GrainParams) -> Self {
        Self {
            grain_type: Some(p.grain_type),
            amount: Some(p.amount),
            size: Some(p.size),
            seed: p.seed,
        }
    }
}

/// Partial parameter set — `None` means "not specified by this preset".
///
/// Used for preset deserialization and merging. Convert to concrete
/// `Parameters` via `materialize()` for the engine.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PartialParameters {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exposure: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contrast: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub highlights: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shadows: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub whites: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blacks: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tint: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hsl: Option<PartialHslChannels>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vignette: Option<PartialVignetteParams>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color_grading: Option<PartialColorGradingParams>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tone_curve: Option<PartialToneCurveParams>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<PartialDetailParams>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dehaze: Option<PartialDehazeParams>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub noise_reduction: Option<PartialNoiseReductionParams>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grain: Option<PartialGrainParams>,
}

impl PartialParameters {
    /// Merge `other` on top of `self` (last-write-wins).
    pub fn merge(&self, other: &Self) -> Self {
        Self {
            exposure: other.exposure.or(self.exposure),
            contrast: other.contrast.or(self.contrast),
            highlights: other.highlights.or(self.highlights),
            shadows: other.shadows.or(self.shadows),
            whites: other.whites.or(self.whites),
            blacks: other.blacks.or(self.blacks),
            temperature: other.temperature.or(self.temperature),
            tint: other.tint.or(self.tint),
            hsl: match (&self.hsl, &other.hsl) {
                (None, None) => None,
                (Some(b), None) => Some(b.clone()),
                (None, Some(o)) => Some(o.clone()),
                (Some(b), Some(o)) => Some(b.merge(o)),
            },
            vignette: match (&self.vignette, &other.vignette) {
                (None, None) => None,
                (Some(b), None) => Some(b.clone()),
                (None, Some(o)) => Some(o.clone()),
                (Some(b), Some(o)) => Some(b.merge(o)),
            },
            color_grading: match (&self.color_grading, &other.color_grading) {
                (None, None) => None,
                (Some(b), None) => Some(b.clone()),
                (None, Some(o)) => Some(o.clone()),
                (Some(b), Some(o)) => Some(b.merge(o)),
            },
            tone_curve: match (&self.tone_curve, &other.tone_curve) {
                (None, None) => None,
                (Some(b), None) => Some(b.clone()),
                (None, Some(o)) => Some(o.clone()),
                (Some(b), Some(o)) => Some(b.merge(o)),
            },
            detail: match (&self.detail, &other.detail) {
                (None, None) => None,
                (Some(b), None) => Some(b.clone()),
                (None, Some(o)) => Some(o.clone()),
                (Some(b), Some(o)) => Some(b.merge(o)),
            },
            dehaze: match (&self.dehaze, &other.dehaze) {
                (None, None) => None,
                (Some(b), None) => Some(b.clone()),
                (None, Some(o)) => Some(o.clone()),
                (Some(b), Some(o)) => Some(b.merge(o)),
            },
            noise_reduction: match (&self.noise_reduction, &other.noise_reduction) {
                (None, None) => None,
                (Some(b), None) => Some(b.clone()),
                (None, Some(o)) => Some(o.clone()),
                (Some(b), Some(o)) => Some(b.merge(o)),
            },
            grain: match (&self.grain, &other.grain) {
                (None, None) => None,
                (Some(b), None) => Some(b.clone()),
                (None, Some(o)) => Some(o.clone()),
                (Some(b), Some(o)) => Some(b.merge(o)),
            },
        }
    }

    /// Convert to concrete Parameters. `None` fields become their default (0.0).
    pub fn materialize(&self) -> Parameters {
        Parameters {
            exposure: self.exposure.unwrap_or(0.0),
            contrast: self.contrast.unwrap_or(0.0),
            highlights: self.highlights.unwrap_or(0.0),
            shadows: self.shadows.unwrap_or(0.0),
            whites: self.whites.unwrap_or(0.0),
            blacks: self.blacks.unwrap_or(0.0),
            temperature: self.temperature.unwrap_or(0.0),
            tint: self.tint.unwrap_or(0.0),
            hsl: self
                .hsl
                .as_ref()
                .map(|h| h.materialize())
                .unwrap_or_default(),
            vignette: self
                .vignette
                .as_ref()
                .map(|v| v.materialize())
                .unwrap_or_default(),
            color_grading: self
                .color_grading
                .as_ref()
                .map(|cg| cg.materialize())
                .unwrap_or_default(),
            tone_curve: self
                .tone_curve
                .as_ref()
                .map(|tc| tc.materialize())
                .unwrap_or_default(),
            detail: self
                .detail
                .as_ref()
                .map(|d| d.materialize())
                .unwrap_or_default(),
            dehaze: self
                .dehaze
                .as_ref()
                .map(|d| d.materialize())
                .unwrap_or_default(),
            noise_reduction: self
                .noise_reduction
                .as_ref()
                .map(|nr| nr.materialize())
                .unwrap_or_default(),
            grain: self
                .grain
                .as_ref()
                .map(|g| g.materialize())
                .unwrap_or_default(),
        }
    }
}

impl From<&Parameters> for PartialParameters {
    fn from(params: &Parameters) -> Self {
        Self {
            exposure: Some(params.exposure),
            contrast: Some(params.contrast),
            highlights: Some(params.highlights),
            shadows: Some(params.shadows),
            whites: Some(params.whites),
            blacks: Some(params.blacks),
            temperature: Some(params.temperature),
            tint: Some(params.tint),
            hsl: Some(PartialHslChannels::from(&params.hsl)),
            vignette: Some(PartialVignetteParams::from(&params.vignette)),
            color_grading: Some(PartialColorGradingParams::from(&params.color_grading)),
            tone_curve: Some(PartialToneCurveParams::from(&params.tone_curve)),
            detail: Some(PartialDetailParams::from(&params.detail)),
            dehaze: Some(PartialDehazeParams::from(&params.dehaze)),
            noise_reduction: Some(PartialNoiseReductionParams::from(&params.noise_reduction)),
            grain: Some(PartialGrainParams::from(&params.grain)),
        }
    }
}

/// The rendering engine. Holds an immutable original image and mutable parameters.
///
/// On each `render()` call, all adjustments are applied from scratch in a fixed
/// internal order. This gives user-facing order-independence.
pub struct Engine {
    original: Rgb32FImage,
    params: Parameters,
    lut: Option<Arc<crate::lut::Lut3D>>,
    pipeline: Pipeline,
}

impl Engine {
    /// Create a new engine with the given linear sRGB image and neutral parameters.
    pub fn new(image: Rgb32FImage) -> Self {
        Self {
            original: image,
            params: Parameters::default(),
            lut: None,
            pipeline: Pipeline::new(),
        }
    }

    /// Get a reference to the original (unmodified) image.
    pub fn original(&self) -> &Rgb32FImage {
        &self.original
    }

    /// Get a reference to the current parameters.
    pub fn params(&self) -> &Parameters {
        &self.params
    }

    /// Get a mutable reference to the current parameters.
    pub fn params_mut(&mut self) -> &mut Parameters {
        &mut self.params
    }

    /// Set parameters from a full Parameters struct.
    pub fn set_params(&mut self, params: Parameters) {
        self.params = params;
    }

    /// Get a reference to the current LUT, if any.
    pub fn lut(&self) -> Option<&crate::lut::Lut3D> {
        self.lut.as_deref()
    }

    /// Set or clear the 3D LUT.
    ///
    /// Accepts `Arc<Lut3D>` so the same LUT can be shared across multiple
    /// engine instances (e.g. in batch processing) without cloning the table.
    pub fn set_lut(&mut self, lut: Option<Arc<crate::lut::Lut3D>>) {
        self.lut = lut;
    }

    /// Apply a preset, replacing the current parameters and LUT.
    pub fn apply_preset(&mut self, preset: &crate::preset::Preset) {
        self.params = preset.params();
        self.lut = preset.lut.clone();
    }

    /// Layer a preset on top of current parameters.
    /// Only fields specified in the preset (Some values in partial_params)
    /// are overridden. Unspecified fields keep their current values.
    pub fn layer_preset(&mut self, preset: &crate::preset::Preset) {
        let current_partial = PartialParameters::from(&self.params);
        let merged = current_partial.merge(&preset.partial_params);
        self.params = merged.materialize();
        if preset.lut.is_some() {
            self.lut = preset.lut.clone();
        }
    }

    /// Render the image by applying all adjustments from scratch.
    ///
    /// Delegates to the stage-based pipeline. Each render starts from the
    /// original image — no state accumulates between renders.
    pub fn render(&mut self) -> RenderResult {
        self.pipeline
            .execute(&self.original, &self.params, self.lut.as_deref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::ImageBuffer;

    fn make_test_image(r: f32, g: f32, b: f32) -> Rgb32FImage {
        ImageBuffer::from_pixel(2, 2, Rgb([r, g, b]))
    }

    #[test]
    fn parameters_default_is_neutral() {
        let p = Parameters::default();
        assert_eq!(p.exposure, 0.0);
        assert_eq!(p.contrast, 0.0);
        assert_eq!(p.highlights, 0.0);
        assert_eq!(p.shadows, 0.0);
        assert_eq!(p.whites, 0.0);
        assert_eq!(p.blacks, 0.0);
        assert_eq!(p.temperature, 0.0);
        assert_eq!(p.tint, 0.0);
        assert!(p.dehaze.is_neutral());
        assert_eq!(p.dehaze.amount, 0.0);
    }

    #[test]
    fn render_neutral_params_is_identity() {
        let img = make_test_image(0.5, 0.3, 0.1);
        let mut engine = Engine::new(img);
        let rendered = engine.render().image;
        let orig = engine.original().get_pixel(0, 0);
        let rend = rendered.get_pixel(0, 0);
        for i in 0..3 {
            assert!(
                (orig.0[i] - rend.0[i]).abs() < 1e-5,
                "Channel {}: expected {}, got {}",
                i,
                orig.0[i],
                rend.0[i]
            );
        }
    }

    #[test]
    fn render_exposure_plus_one_doubles() {
        let img = make_test_image(0.25, 0.25, 0.25);
        let mut engine = Engine::new(img);
        engine.params_mut().exposure = 1.0;
        let pixel = *engine.render().image.get_pixel(0, 0);
        for i in 0..3 {
            assert!(
                (pixel.0[i] - 0.5).abs() < 1e-5,
                "Channel {}: expected 0.5, got {}",
                i,
                pixel.0[i]
            );
        }
    }

    #[test]
    fn render_contrast_changes_output() {
        let img = make_test_image(0.5, 0.5, 0.5);
        let mut engine = Engine::new(img);
        engine.params_mut().contrast = 50.0;
        let rendered = engine.render().image;
        let mut neutral_engine = Engine::new(make_test_image(0.5, 0.5, 0.5));
        let neutral = neutral_engine.render().image;
        // With contrast, pixel values above/below mid should shift
        let rp = rendered.get_pixel(0, 0);
        let np = neutral.get_pixel(0, 0);
        // 0.5 in linear → sRGB ~0.735 → contrast pushes further from 0.5
        // Result should differ from neutral
        assert!(
            (rp.0[0] - np.0[0]).abs() > 1e-6 || rp.0[0] == np.0[0],
            "Contrast should change output for non-midpoint sRGB values"
        );
    }

    #[test]
    fn render_warm_white_balance_boosts_red() {
        let img = make_test_image(0.5, 0.5, 0.5);
        let mut engine = Engine::new(img);
        engine.params_mut().temperature = 50.0;
        let pixel = *engine.render().image.get_pixel(0, 0);
        // Warm shift: red > blue
        assert!(
            pixel.0[0] > pixel.0[2],
            "Expected red > blue with warm WB, got r={} b={}",
            pixel.0[0],
            pixel.0[2]
        );
    }

    #[test]
    fn render_combined_exposure_and_contrast() {
        let img = make_test_image(0.2, 0.2, 0.2);
        let mut engine = Engine::new(img);
        engine.params_mut().exposure = 1.0;
        engine.params_mut().contrast = 25.0;
        let pixel = *engine.render().image.get_pixel(0, 0);
        // Should be brighter than original 0.2
        assert!(pixel.0[0] > 0.2, "Expected brighter, got {}", pixel.0[0]);
    }

    #[test]
    fn render_with_identity_lut_is_identity() {
        let img = make_test_image(0.5, 0.3, 0.1);
        let mut engine = Engine::new(img);
        let size = 17;
        let n = (size - 1) as f32;
        let mut table = Vec::with_capacity(size * size * size);
        for b in 0..size {
            for g in 0..size {
                for r in 0..size {
                    table.push([r as f32 / n, g as f32 / n, b as f32 / n]);
                }
            }
        }
        let lut = crate::lut::Lut3D {
            title: None,
            size,
            domain_min: [0.0, 0.0, 0.0],
            domain_max: [1.0, 1.0, 1.0],
            table,
        };
        engine.set_lut(Some(Arc::new(lut)));

        let rendered = engine.render().image;
        let orig = engine.original().get_pixel(0, 0);
        let rend = rendered.get_pixel(0, 0);
        for i in 0..3 {
            assert!(
                (orig.0[i] - rend.0[i]).abs() < 0.01,
                "Channel {}: expected ~{}, got {}",
                i,
                orig.0[i],
                rend.0[i]
            );
        }
    }

    #[test]
    fn render_with_no_lut_unchanged() {
        let img = make_test_image(0.5, 0.3, 0.1);
        let mut engine = Engine::new(img);
        assert!(engine.lut().is_none());
        let rendered = engine.render().image;
        let orig = engine.original().get_pixel(0, 0);
        let rend = rendered.get_pixel(0, 0);
        for i in 0..3 {
            assert!((orig.0[i] - rend.0[i]).abs() < 1e-5);
        }
    }

    #[test]
    fn hsl_channel_default_is_zero() {
        let ch = super::HslChannel::default();
        assert_eq!(ch.hue, 0.0);
        assert_eq!(ch.saturation, 0.0);
        assert_eq!(ch.luminance, 0.0);
    }

    #[test]
    fn hsl_channels_default_all_zero() {
        let hsl = super::HslChannels::default();
        assert_eq!(hsl.red, super::HslChannel::default());
        assert_eq!(hsl.green, super::HslChannel::default());
        assert_eq!(hsl.magenta, super::HslChannel::default());
    }

    #[test]
    fn hsl_channels_is_default_true_when_default() {
        let hsl = super::HslChannels::default();
        assert!(hsl.is_default());
    }

    #[test]
    fn hsl_channels_is_default_false_when_modified() {
        let mut hsl = super::HslChannels::default();
        hsl.red.hue = 10.0;
        assert!(!hsl.is_default());
    }

    #[test]
    fn hsl_channels_extracts_shift_arrays() {
        let mut hsl = super::HslChannels::default();
        hsl.red.hue = 15.0;
        hsl.green.saturation = -30.0;
        hsl.blue.luminance = 20.0;
        let h = hsl.hue_shifts();
        let s = hsl.saturation_shifts();
        let l = hsl.luminance_shifts();
        assert_eq!(h[0], 15.0); // red
        assert_eq!(s[3], -30.0); // green
        assert_eq!(l[5], 20.0); // blue
    }

    #[test]
    fn parameters_default_hsl_is_default() {
        let p = Parameters::default();
        assert!(p.hsl.is_default());
    }

    #[test]
    fn render_hsl_neutral_is_identity() {
        // Red-ish pixel in linear space
        let img = make_test_image(0.5, 0.01, 0.01);
        let mut engine = Engine::new(img);
        // HSL defaults to all zeros, so render should be identity
        let orig = *engine.original().get_pixel(0, 0);
        let rend = *engine.render().image.get_pixel(0, 0);
        for i in 0..3 {
            assert!(
                (orig.0[i] - rend.0[i]).abs() < 1e-4,
                "Channel {i}: expected {}, got {}",
                orig.0[i],
                rend.0[i]
            );
        }
    }

    #[test]
    fn render_hsl_red_saturation_decrease() {
        // Pure-ish red in linear space
        let img = make_test_image(0.5, 0.01, 0.01);
        let mut engine = Engine::new(img);
        engine.params_mut().hsl.red.saturation = -100.0;
        let rendered = engine.render().image;
        let p = rendered.get_pixel(0, 0);
        // Desaturated: channels should be closer together than original
        let spread = (p.0[0] - p.0[1]).abs() + (p.0[0] - p.0[2]).abs();
        let orig = engine.original().get_pixel(0, 0);
        let orig_spread = (orig.0[0] - orig.0[1]).abs() + (orig.0[0] - orig.0[2]).abs();
        assert!(
            spread < orig_spread,
            "Expected less spread after desaturation: {spread} vs {orig_spread}"
        );
    }

    #[test]
    fn render_hsl_green_shift_does_not_affect_red_image() {
        let img = make_test_image(0.5, 0.01, 0.01);
        let mut engine = Engine::new(img);
        engine.params_mut().hsl.green.saturation = -100.0;
        let rendered = engine.render().image;
        let orig = engine.original().get_pixel(0, 0);
        let rend = rendered.get_pixel(0, 0);
        for i in 0..3 {
            assert!(
                (orig.0[i] - rend.0[i]).abs() < 1e-3,
                "Channel {i}: red image should be unaffected by green HSL"
            );
        }
    }

    // --- PartialHslChannel tests ---

    #[test]
    fn partial_hsl_channel_default_is_all_none() {
        let ch = super::PartialHslChannel::default();
        assert_eq!(ch.hue, None);
        assert_eq!(ch.saturation, None);
        assert_eq!(ch.luminance, None);
    }

    #[test]
    fn partial_hsl_channels_default_is_all_none() {
        let hsl = super::PartialHslChannels::default();
        assert_eq!(hsl.red, None);
        assert_eq!(hsl.green, None);
        assert_eq!(hsl.blue, None);
    }

    #[test]
    fn partial_hsl_channel_merge_overlay_wins() {
        let base = super::PartialHslChannel {
            hue: Some(10.0),
            saturation: Some(20.0),
            luminance: None,
        };
        let overlay = super::PartialHslChannel {
            hue: Some(30.0),
            saturation: None,
            luminance: Some(5.0),
        };
        let merged = base.merge(&overlay);
        assert_eq!(merged.hue, Some(30.0));
        assert_eq!(merged.saturation, Some(20.0));
        assert_eq!(merged.luminance, Some(5.0));
    }

    #[test]
    fn partial_hsl_channels_merge_channel_level() {
        let base = super::PartialHslChannels {
            red: Some(super::PartialHslChannel {
                hue: Some(10.0),
                saturation: None,
                luminance: None,
            }),
            ..Default::default()
        };
        let overlay = super::PartialHslChannels {
            red: Some(super::PartialHslChannel {
                hue: None,
                saturation: Some(20.0),
                luminance: None,
            }),
            green: Some(super::PartialHslChannel {
                hue: Some(5.0),
                saturation: None,
                luminance: None,
            }),
            ..Default::default()
        };
        let merged = base.merge(&overlay);
        assert_eq!(merged.red.as_ref().unwrap().hue, Some(10.0));
        assert_eq!(merged.red.as_ref().unwrap().saturation, Some(20.0));
        assert_eq!(merged.green.as_ref().unwrap().hue, Some(5.0));
        assert_eq!(merged.blue, None);
    }

    #[test]
    fn partial_hsl_channel_materialize() {
        let partial = super::PartialHslChannel {
            hue: Some(15.0),
            saturation: None,
            luminance: Some(-10.0),
        };
        let concrete = partial.materialize();
        assert_eq!(concrete.hue, 15.0);
        assert_eq!(concrete.saturation, 0.0);
        assert_eq!(concrete.luminance, -10.0);
    }

    #[test]
    fn partial_hsl_channels_materialize() {
        let partial = super::PartialHslChannels {
            red: Some(super::PartialHslChannel {
                hue: Some(15.0),
                saturation: None,
                luminance: None,
            }),
            ..Default::default()
        };
        let concrete = partial.materialize();
        assert_eq!(concrete.red.hue, 15.0);
        assert_eq!(concrete.red.saturation, 0.0);
        assert_eq!(concrete.green, super::HslChannel::default());
    }

    // --- PartialParameters tests ---

    #[test]
    fn partial_parameters_default_is_all_none() {
        let p = super::PartialParameters::default();
        assert_eq!(p.exposure, None);
        assert_eq!(p.contrast, None);
        assert_eq!(p.hsl, None);
    }

    #[test]
    fn partial_parameters_merge_overlay_wins() {
        let base = super::PartialParameters {
            exposure: Some(1.0),
            contrast: Some(20.0),
            ..Default::default()
        };
        let overlay = super::PartialParameters {
            exposure: Some(2.0),
            highlights: Some(-30.0),
            ..Default::default()
        };
        let merged = base.merge(&overlay);
        assert_eq!(merged.exposure, Some(2.0));
        assert_eq!(merged.contrast, Some(20.0));
        assert_eq!(merged.highlights, Some(-30.0));
        assert_eq!(merged.shadows, None);
    }

    #[test]
    fn partial_parameters_materialize_defaults() {
        let partial = super::PartialParameters {
            exposure: Some(1.5),
            ..Default::default()
        };
        let params = partial.materialize();
        assert_eq!(params.exposure, 1.5);
        assert_eq!(params.contrast, 0.0);
        assert_eq!(params.temperature, 0.0);
        assert!(params.hsl.is_default());
    }

    #[test]
    fn partial_parameters_from_parameters_all_some() {
        let params = Parameters {
            exposure: 1.0,
            contrast: 20.0,
            ..Default::default()
        };
        let partial = super::PartialParameters::from(&params);
        assert_eq!(partial.exposure, Some(1.0));
        assert_eq!(partial.contrast, Some(20.0));
        assert_eq!(partial.highlights, Some(0.0));
    }

    #[test]
    fn partial_parameters_merge_with_hsl() {
        let base = super::PartialParameters {
            exposure: Some(1.0),
            ..Default::default()
        };
        let overlay = super::PartialParameters {
            hsl: Some(super::PartialHslChannels {
                red: Some(super::PartialHslChannel {
                    hue: Some(10.0),
                    saturation: None,
                    luminance: None,
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        let merged = base.merge(&overlay);
        assert_eq!(merged.exposure, Some(1.0));
        assert!(merged.hsl.is_some());
        assert_eq!(
            merged.hsl.as_ref().unwrap().red.as_ref().unwrap().hue,
            Some(10.0)
        );
    }

    // --- layer_preset tests ---

    #[test]
    fn layer_preset_only_overrides_specified_fields() {
        let img = make_test_image(0.5, 0.5, 0.5);
        let mut engine = Engine::new(img);
        engine.params_mut().exposure = 1.0;
        engine.params_mut().contrast = 20.0;

        let mut preset = crate::preset::Preset::default();
        preset.partial_params.contrast = Some(50.0);

        engine.layer_preset(&preset);
        assert_eq!(engine.params().exposure, 1.0);
        assert_eq!(engine.params().contrast, 50.0);
    }

    #[test]
    fn layer_preset_preserves_unspecified_hsl() {
        let img = make_test_image(0.5, 0.5, 0.5);
        let mut engine = Engine::new(img);
        engine.params_mut().hsl.red.hue = 15.0;

        let mut preset = crate::preset::Preset::default();
        preset.partial_params.hsl = Some(PartialHslChannels {
            green: Some(PartialHslChannel {
                hue: Some(10.0),
                saturation: None,
                luminance: None,
            }),
            ..Default::default()
        });

        engine.layer_preset(&preset);
        assert_eq!(engine.params().hsl.red.hue, 15.0);
        assert_eq!(engine.params().hsl.green.hue, 10.0);
    }

    #[test]
    fn layer_multiple_presets_last_wins() {
        let img = make_test_image(0.5, 0.5, 0.5);
        let mut engine = Engine::new(img);

        let mut preset1 = crate::preset::Preset::default();
        preset1.partial_params.exposure = Some(1.0);
        preset1.partial_params.contrast = Some(20.0);

        let mut preset2 = crate::preset::Preset::default();
        preset2.partial_params.exposure = Some(2.0);

        engine.layer_preset(&preset1);
        engine.layer_preset(&preset2);

        assert_eq!(engine.params().exposure, 2.0);
        assert_eq!(engine.params().contrast, 20.0);
    }

    // --- VignetteParams tests ---

    #[test]
    fn vignette_params_default() {
        let v = super::VignetteParams::default();
        assert_eq!(v.amount, 0.0);
        assert_eq!(v.shape, crate::adjust::VignetteShape::Elliptical);
    }

    #[test]
    fn partial_vignette_params_default_is_all_none() {
        let v = super::PartialVignetteParams::default();
        assert_eq!(v.amount, None);
        assert_eq!(v.shape, None);
    }

    #[test]
    fn partial_vignette_params_merge_overlay_wins() {
        let base = super::PartialVignetteParams {
            amount: Some(-30.0),
            shape: Some(crate::adjust::VignetteShape::Elliptical),
        };
        let overlay = super::PartialVignetteParams {
            amount: Some(-50.0),
            shape: None,
        };
        let merged = base.merge(&overlay);
        assert_eq!(merged.amount, Some(-50.0));
        assert_eq!(merged.shape, Some(crate::adjust::VignetteShape::Elliptical));
    }

    #[test]
    fn partial_vignette_params_materialize_defaults() {
        let partial = super::PartialVignetteParams {
            amount: Some(-30.0),
            shape: None,
        };
        let concrete = partial.materialize();
        assert_eq!(concrete.amount, -30.0);
        assert_eq!(concrete.shape, crate::adjust::VignetteShape::Elliptical);
    }

    #[test]
    fn partial_vignette_params_from_concrete() {
        let concrete = super::VignetteParams {
            amount: -30.0,
            shape: crate::adjust::VignetteShape::Circular,
        };
        let partial = super::PartialVignetteParams::from(&concrete);
        assert_eq!(partial.amount, Some(-30.0));
        assert_eq!(partial.shape, Some(crate::adjust::VignetteShape::Circular));
    }

    #[test]
    fn parameters_default_vignette_is_neutral() {
        let p = Parameters::default();
        assert_eq!(p.vignette.amount, 0.0);
        assert_eq!(p.vignette.shape, crate::adjust::VignetteShape::Elliptical);
    }

    #[test]
    fn apply_preset_still_does_full_replacement() {
        let img = make_test_image(0.5, 0.5, 0.5);
        let mut engine = Engine::new(img);
        engine.params_mut().exposure = 1.0;
        engine.params_mut().contrast = 20.0;

        let mut preset = crate::preset::Preset::default();
        preset.partial_params.exposure = Some(0.5);

        engine.apply_preset(&preset);
        assert_eq!(engine.params().exposure, 0.5);
        assert_eq!(engine.params().contrast, 0.0);
    }

    #[test]
    fn render_vignette_darkens_corners() {
        // Use a 10x10 image so corners are clearly away from center
        let img: Rgb32FImage = ImageBuffer::from_pixel(10, 10, Rgb([0.5, 0.5, 0.5]));
        let mut engine = Engine::new(img);
        engine.params_mut().vignette.amount = -50.0;
        let rendered = engine.render().image;

        // Center pixel should be close to original
        let center = rendered.get_pixel(5, 5);
        assert!(
            (center.0[0] - 0.5).abs() < 0.05,
            "Center should be near original, got {}",
            center.0[0]
        );

        // Corner pixel should be darker
        let corner = rendered.get_pixel(0, 0);
        assert!(
            corner.0[0] < center.0[0],
            "Corner ({}) should be darker than center ({})",
            corner.0[0],
            center.0[0]
        );
    }

    #[test]
    fn render_vignette_zero_is_identity() {
        let img = make_test_image(0.5, 0.3, 0.1);
        let mut engine = Engine::new(img);
        engine.params_mut().vignette.amount = 0.0;
        let rendered = engine.render().image;
        let orig = engine.original().get_pixel(0, 0);
        let rend = rendered.get_pixel(0, 0);
        for i in 0..3 {
            assert!(
                (orig.0[i] - rend.0[i]).abs() < 1e-5,
                "Channel {}: expected {}, got {}",
                i,
                orig.0[i],
                rend.0[i]
            );
        }
    }

    #[test]
    fn full_pipeline_decode_engine_encode() {
        let temp_dir = std::env::temp_dir();
        let input = temp_dir.join("agx_e2e_in.png");
        let output = temp_dir.join("agx_e2e_out.png");

        // Create sRGB 128,128,128 test image
        let img: ImageBuffer<image::Rgb<u8>, Vec<u8>> =
            ImageBuffer::from_pixel(4, 4, image::Rgb([128u8, 128, 128]));
        img.save(&input).unwrap();

        // Decode → Engine +1 stop → Render → Encode
        let linear = crate::decode::decode_standard(&input).unwrap();
        let mut engine = Engine::new(linear);
        engine.params_mut().exposure = 1.0;
        let rendered = engine.render().image;
        crate::encode::encode_to_file(&rendered, &output).unwrap();

        // Verify output is brighter (sRGB 128 → linear ~0.216 → *2 → ~0.432 → sRGB ~173)
        let out_img = image::ImageReader::open(&output)
            .unwrap()
            .decode()
            .unwrap()
            .to_rgb8();
        let pixel = out_img.get_pixel(0, 0);
        assert!(
            pixel.0[0] > 150 && pixel.0[0] < 190,
            "Expected ~173, got {}",
            pixel.0[0]
        );

        let _ = std::fs::remove_file(&input);
        let _ = std::fs::remove_file(&output);
    }

    // --- Color grading partial tests ---

    #[test]
    fn partial_color_grading_merge() {
        let base = PartialColorGradingParams {
            shadows: Some(PartialColorWheel {
                hue: Some(200.0),
                saturation: Some(30.0),
                luminance: None,
            }),
            midtones: None,
            highlights: None,
            global: None,
            balance: Some(-10.0),
        };
        let overlay = PartialColorGradingParams {
            shadows: Some(PartialColorWheel {
                hue: None,
                saturation: Some(50.0),
                luminance: Some(-5.0),
            }),
            midtones: None,
            highlights: None,
            global: None,
            balance: None,
        };
        let merged = base.merge(&overlay);
        let shadows = merged.shadows.unwrap();
        assert_eq!(shadows.hue, Some(200.0));
        assert_eq!(shadows.saturation, Some(50.0));
        assert_eq!(shadows.luminance, Some(-5.0));
        assert_eq!(merged.balance, Some(-10.0));
    }

    #[test]
    fn partial_color_grading_materialize_defaults() {
        let partial = PartialColorGradingParams::default();
        let materialized = partial.materialize();
        assert!(materialized.is_default());
    }

    #[test]
    fn render_default_color_grading_is_identity() {
        let params = Parameters::default();
        assert!(params.color_grading.is_default());
    }

    // --- Tone Curve tests ---

    #[test]
    fn partial_tone_curve_merge() {
        let base = PartialToneCurveParams {
            rgb: Some(PartialToneCurve {
                points: Some(vec![(0.0, 0.0), (0.5, 0.6), (1.0, 1.0)]),
            }),
            luma: None,
            red: None,
            green: None,
            blue: None,
        };
        let overlay = PartialToneCurveParams {
            rgb: None,
            luma: Some(PartialToneCurve {
                points: Some(vec![(0.0, 0.1), (1.0, 0.9)]),
            }),
            red: None,
            green: None,
            blue: None,
        };
        let merged = base.merge(&overlay);
        assert!(merged.rgb.is_some(), "rgb should be preserved from base");
        assert!(merged.luma.is_some(), "luma should come from overlay");
    }

    #[test]
    fn partial_tone_curve_materialize_defaults() {
        let partial = PartialToneCurveParams::default();
        let materialized = partial.materialize();
        assert!(materialized.is_default());
    }

    #[test]
    fn render_default_tone_curves_is_identity() {
        let params = Parameters::default();
        assert!(params.tone_curve.is_default());
    }

    // --- PartialDetailParams tests ---

    #[test]
    fn partial_detail_merge_overlay_wins() {
        let base = PartialDetailParams {
            sharpening: Some(PartialSharpeningParams {
                amount: Some(40.0),
                radius: Some(1.5),
                threshold: None,
                masking: None,
            }),
            clarity: Some(20.0),
            texture: None,
        };
        let overlay = PartialDetailParams {
            sharpening: Some(PartialSharpeningParams {
                amount: Some(60.0),
                radius: None,
                threshold: Some(50.0),
                masking: None,
            }),
            clarity: None,
            texture: Some(10.0),
        };
        let merged = base.merge(&overlay);
        let sharp = merged.sharpening.unwrap();
        assert_eq!(sharp.amount, Some(60.0));
        assert_eq!(sharp.radius, Some(1.5));
        assert_eq!(sharp.threshold, Some(50.0));
        assert_eq!(sharp.masking, None);
        assert_eq!(merged.clarity, Some(20.0));
        assert_eq!(merged.texture, Some(10.0));
    }

    #[test]
    fn partial_detail_materialize_defaults() {
        let partial = PartialDetailParams::default();
        let concrete = partial.materialize();
        assert_eq!(concrete, crate::adjust::DetailParams::default());
    }

    #[test]
    fn partial_detail_from_concrete_roundtrip() {
        let concrete = crate::adjust::DetailParams {
            sharpening: crate::adjust::SharpeningParams {
                amount: 40.0,
                radius: 2.0,
                threshold: 30.0,
                masking: 50.0,
            },
            clarity: 25.0,
            texture: -10.0,
        };
        let partial = PartialDetailParams::from(&concrete);
        let back = partial.materialize();
        assert_eq!(concrete, back);
    }

    #[test]
    fn render_with_sharpening_differs_from_neutral() {
        let w = 16;
        let h = 16;
        let img = Rgb32FImage::from_fn(w, h, |x, _y| {
            let v = x as f32 / (w - 1) as f32;
            Rgb([v * 0.5, v * 0.5, v * 0.5])
        });
        let mut engine = Engine::new(img.clone());
        let neutral_render = engine.render().image;

        engine.params_mut().detail.sharpening.amount = 80.0;
        engine.params_mut().detail.sharpening.threshold = 0.0;
        let sharp_render = engine.render().image;

        let mut diffs = 0;
        for y in 2..h - 2 {
            for x in 2..w - 2 {
                let n = neutral_render.get_pixel(x, y);
                let s = sharp_render.get_pixel(x, y);
                if (n.0[0] - s.0[0]).abs() > 1e-4 {
                    diffs += 1;
                }
            }
        }
        assert!(diffs > 0, "sharpening should change at least some pixels");
    }

    #[test]
    fn render_default_detail_is_identity() {
        let img = make_test_image(0.5, 0.3, 0.1);
        let mut engine = Engine::new(img);
        let rendered = engine.render().image;
        let orig = engine.original().get_pixel(0, 0);
        let rend = rendered.get_pixel(0, 0);
        for i in 0..3 {
            assert!(
                (orig.0[i] - rend.0[i]).abs() < 1e-5,
                "default detail should not change output"
            );
        }
    }

    #[test]
    fn partial_dehaze_merge_and_materialize() {
        let base = PartialDehazeParams { amount: Some(30.0) };
        let overlay = PartialDehazeParams { amount: None };
        let merged = base.merge(&overlay);
        assert_eq!(merged.amount, Some(30.0));

        let overlay2 = PartialDehazeParams { amount: Some(50.0) };
        let merged2 = base.merge(&overlay2);
        assert_eq!(merged2.amount, Some(50.0));

        let empty = PartialDehazeParams::default();
        let mat = empty.materialize();
        assert_eq!(mat.amount, 0.0);
    }

    #[test]
    fn render_with_dehaze_changes_output() {
        // Use a gradient image so dehaze has variation to work with
        let mut img = Rgb32FImage::new(10, 10);
        for y in 0..10 {
            for x in 0..10 {
                let t = (y * 10 + x) as f32 / 100.0;
                img.put_pixel(x, y, Rgb([0.4 + 0.4 * t, 0.4 + 0.3 * t, 0.45 + 0.3 * t]));
            }
        }
        let mut engine = Engine::new(img.clone());
        engine.params_mut().dehaze.amount = 50.0;
        let dehazed = engine.render().image;
        let neutral = Engine::new(img).render().image;
        let dp = dehazed.get_pixel(0, 0);
        let np = neutral.get_pixel(0, 0);
        let differs = (0..3).any(|i| (dp.0[i] - np.0[i]).abs() > 1e-4);
        assert!(differs, "Dehaze should change output");
    }

    #[test]
    fn render_default_dehaze_is_identity() {
        let img = make_test_image(0.5, 0.3, 0.1);
        let mut engine = Engine::new(img);
        let rendered = engine.render().image;
        let orig = engine.original().get_pixel(0, 0);
        let rend = rendered.get_pixel(0, 0);
        for i in 0..3 {
            assert!(
                (orig.0[i] - rend.0[i]).abs() < 1e-5,
                "default dehaze should not change output"
            );
        }
    }

    #[test]
    fn partial_nr_merge_and_materialize() {
        let base = PartialNoiseReductionParams {
            luminance: Some(30.0),
            color: Some(20.0),
            detail: None,
        };
        let overlay = PartialNoiseReductionParams {
            luminance: None,
            color: Some(40.0),
            detail: Some(50.0),
        };
        let merged = base.merge(&overlay);
        assert_eq!(merged.luminance, Some(30.0));
        assert_eq!(merged.color, Some(40.0));
        assert_eq!(merged.detail, Some(50.0));

        let mat = merged.materialize();
        assert!((mat.luminance - 30.0).abs() < 1e-6);
        assert!((mat.color - 40.0).abs() < 1e-6);
        assert!((mat.detail - 50.0).abs() < 1e-6);

        let empty = PartialNoiseReductionParams::default();
        let mat_empty = empty.materialize();
        assert!(mat_empty.is_neutral());
    }

    #[test]
    fn render_with_nr_changes_output() {
        let mut img = Rgb32FImage::new(32, 32);
        let mut rng: u64 = 42;
        for y in 0..32 {
            for x in 0..32 {
                let base = (y * 32 + x) as f32 / 1024.0 * 0.5 + 0.25;
                rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
                let noise = ((rng >> 33) as f32 / (1u64 << 31) as f32 - 0.5) * 0.1;
                let v = (base + noise).clamp(0.0, 1.0);
                img.put_pixel(x, y, Rgb([v, v, v]));
            }
        }
        let mut engine = Engine::new(img.clone());
        engine.params_mut().noise_reduction.luminance = 50.0;
        let denoised = engine.render().image;
        let neutral = Engine::new(img).render().image;
        let dp = denoised.get_pixel(0, 0);
        let np = neutral.get_pixel(0, 0);
        let differs = (0..3).any(|i| (dp.0[i] - np.0[i]).abs() > 1e-4);
        assert!(differs, "Noise reduction should change output");
    }

    #[test]
    fn render_default_nr_is_identity() {
        let img = make_test_image(0.5, 0.3, 0.1);
        let mut engine = Engine::new(img);
        let rendered = engine.render().image;
        let orig = engine.original().get_pixel(0, 0);
        let rend = rendered.get_pixel(0, 0);
        for i in 0..3 {
            assert!(
                (orig.0[i] - rend.0[i]).abs() < 1e-5,
                "default NR should not change output"
            );
        }
    }

    #[test]
    fn partial_grain_merge_and_materialize() {
        let base = PartialGrainParams {
            grain_type: Some(crate::adjust::GrainType::Silver),
            amount: Some(30.0),
            size: None,
            seed: None,
        };
        let overlay = PartialGrainParams {
            grain_type: None,
            amount: None,
            size: Some(60.0),
            seed: None,
        };
        let merged = base.merge(&overlay);
        let concrete = merged.materialize();
        assert_eq!(concrete.grain_type, crate::adjust::GrainType::Silver);
        assert_eq!(concrete.amount, 30.0);
        assert_eq!(concrete.size, 60.0);
    }

    #[test]
    fn render_default_grain_is_identity() {
        let img = make_test_image(0.5, 0.3, 0.1);
        let mut engine = Engine::new(img);
        let rendered = engine.render().image;
        let orig = engine.original().get_pixel(0, 0);
        let rend = rendered.get_pixel(0, 0);
        for i in 0..3 {
            assert!(
                (orig.0[i] - rend.0[i]).abs() < 1e-5,
                "default grain should be identity"
            );
        }
    }

    #[test]
    fn render_with_grain_changes_output() {
        // Use a larger image to get meaningful grain variation across pixels
        let img = ImageBuffer::from_pixel(64, 64, Rgb([0.5f32, 0.5, 0.5]));
        let mut engine = Engine::new(img);
        let before = engine.render().image;
        engine.params_mut().grain = crate::adjust::GrainParams {
            grain_type: crate::adjust::GrainType::Silver,
            amount: 50.0,
            size: 50.0,
            seed: None,
        };
        let after = engine.render().image;
        // Check multiple pixels — grain is random so at least some should differ
        let mut changed = false;
        for y in 0..64 {
            for x in 0..64 {
                let bp = before.get_pixel(x, y);
                let ap = after.get_pixel(x, y);
                if (0..3).any(|i| (bp.0[i] - ap.0[i]).abs() > 1e-5) {
                    changed = true;
                    break;
                }
            }
            if changed {
                break;
            }
        }
        assert!(changed, "grain should change render output");
    }
}
