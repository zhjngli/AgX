use serde::{Deserialize, Serialize};

/// Noise reduction parameters.
///
/// - `luminance`: 0–100, strength of luminance denoising
/// - `color`: 0–100, strength of chroma denoising
/// - `detail`: 0–100, finest-scale protection (higher = more detail kept)
///
/// When all three are zero, the noise reduction pass is skipped entirely.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NoiseReductionParams {
    #[serde(default)]
    pub luminance: f32,
    #[serde(default)]
    pub color: f32,
    #[serde(default)]
    pub detail: f32,
}

impl Default for NoiseReductionParams {
    fn default() -> Self {
        Self {
            luminance: 0.0,
            color: 0.0,
            detail: 0.0,
        }
    }
}

impl NoiseReductionParams {
    /// Returns true when no noise reduction effect would be applied.
    pub fn is_neutral(&self) -> bool {
        self.luminance == 0.0 && self.color == 0.0 && self.detail == 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_params_are_neutral() {
        let p = NoiseReductionParams::default();
        assert!(p.is_neutral());
        assert_eq!(p.luminance, 0.0);
        assert_eq!(p.color, 0.0);
        assert_eq!(p.detail, 0.0);
    }

    #[test]
    fn non_zero_is_not_neutral() {
        let p = NoiseReductionParams {
            luminance: 10.0,
            color: 0.0,
            detail: 0.0,
        };
        assert!(!p.is_neutral());
        let p2 = NoiseReductionParams {
            luminance: 0.0,
            color: 5.0,
            detail: 0.0,
        };
        assert!(!p2.is_neutral());
        let p3 = NoiseReductionParams {
            luminance: 0.0,
            color: 0.0,
            detail: 50.0,
        };
        assert!(!p3.is_neutral());
    }
}
