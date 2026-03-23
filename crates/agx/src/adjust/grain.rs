use serde::{Deserialize, Serialize};

use super::{LUMA_B, LUMA_G, LUMA_R};

/// Grain type controlling the internal character of the noise.
///
/// Each type maps to a combination of octave weights, contrast curve,
/// and luminance falloff strength. Matches Capture One's grain type model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum GrainType {
    Fine,
    #[default]
    Silver,
    Soft,
    Cubic,
    Tabular,
    Harsh,
}

/// Film grain simulation parameters.
///
/// - `grain_type`: selects the grain character (default: Silver)
/// - `amount`: 0–100, intensity of grain effect (0 = no grain)
/// - `size`: 0–100, fine to coarse grain (default: 50)
/// - `chromatic`: 0–100, strength of per-channel color variation (0 = luminance-only)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GrainParams {
    #[serde(default)]
    pub grain_type: GrainType,
    #[serde(default)]
    pub amount: f32,
    #[serde(default = "default_size")]
    pub size: f32,
    #[serde(default)]
    pub chromatic: f32,
}

fn default_size() -> f32 {
    50.0
}

impl Default for GrainParams {
    fn default() -> Self {
        Self {
            grain_type: GrainType::default(),
            amount: 0.0,
            size: 50.0,
            chromatic: 0.0,
        }
    }
}

impl GrainParams {
    /// Returns true when no grain effect would be applied.
    pub fn is_neutral(&self) -> bool {
        self.amount == 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grain_params_default_is_neutral() {
        let p = GrainParams::default();
        assert!(p.is_neutral());
    }

    #[test]
    fn grain_params_nonzero_amount_not_neutral() {
        let p = GrainParams {
            amount: 50.0,
            ..Default::default()
        };
        assert!(!p.is_neutral());
    }

    #[test]
    fn grain_type_default_is_silver() {
        assert_eq!(GrainType::default(), GrainType::Silver);
    }
}
