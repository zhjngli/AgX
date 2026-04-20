#![doc = include_str!("exposure.md")]

// --- Exposure (linear space) ---

/// Compute the exposure multiplier for the given number of stops.
/// 0 stops = 1.0 (no change), +1 stop = 2.0, -1 stop = 0.5.
pub fn exposure_factor(stops: f32) -> f32 {
    2.0f32.powf(stops)
}

/// Apply exposure to a single channel value in linear space.
pub fn apply_exposure(value: f32, factor: f32) -> f32 {
    (value * factor).max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposure_factor_zero_is_one() {
        assert_eq!(exposure_factor(0.0), 1.0);
    }

    #[test]
    fn exposure_factor_one_stop_doubles() {
        assert!((exposure_factor(1.0) - 2.0).abs() < 1e-6);
    }

    #[test]
    fn exposure_factor_neg_one_halves() {
        assert!((exposure_factor(-1.0) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn apply_exposure_multiplies() {
        assert!((apply_exposure(0.25, exposure_factor(1.0)) - 0.5).abs() < 1e-6);
    }
}
