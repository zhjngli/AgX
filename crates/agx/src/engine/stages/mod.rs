//! Pluggable pipeline stages that the render engine executes in sequence.

/// Linear ↔ gamma color-space conversion stages within the Rec.2020 working space.
pub mod color_space_conversion;
/// Atmospheric haze removal/addition stage.
pub mod dehaze;
/// Wavelet-based noise reduction stage.
pub mod denoise;
/// Sharpening, clarity, and texture stage.
pub mod detail;
/// Film grain simulation stage.
pub mod grain;
/// Per-pixel adjustments: contrast, tone curves, HSL, color grading, LUT.
pub mod per_pixel;
/// Edge vignette darkening/brightening stage.
pub mod vignette;
/// White balance and exposure correction stage.
pub mod white_balance_exposure;

pub use color_space_conversion::{GammaToLinearStage, LinearToGammaStage};
pub use dehaze::DehazeStage;
pub use denoise::DenoiseStage;
pub use detail::DetailStage;
pub use grain::GrainStage;
pub use per_pixel::PerPixelAdjustmentsStage;
pub use vignette::VignetteStage;
pub use white_balance_exposure::WhiteBalanceExposureStage;
