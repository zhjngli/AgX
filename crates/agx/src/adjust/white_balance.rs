#![doc = include_str!("white_balance.md")]

// --- White balance (linear space) ---

/// Apply white balance temperature and tint shifts. Returns adjusted (r, g, b) in linear space.
///
/// Temperature > 0 = warm (boost red, reduce blue).
/// Tint > 0 = magenta (reduce green).
/// Channel multipliers are normalized to preserve overall brightness.
pub fn apply_white_balance(r: f32, g: f32, b: f32, temperature: f32, tint: f32) -> (f32, f32, f32) {
    if temperature == 0.0 && tint == 0.0 {
        return (r, g, b);
    }
    let r_mult = 1.0 + temperature / 200.0;
    let b_mult = 1.0 - temperature / 200.0;
    let g_mult = 1.0 - tint / 200.0;

    // Normalize to preserve brightness
    let sum = r_mult + g_mult + b_mult;
    let norm = 3.0 / sum;

    (
        (r * r_mult * norm).max(0.0),
        (g * g_mult * norm).max(0.0),
        (b * b_mult * norm).max(0.0),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- White balance tests ---

    #[test]
    fn white_balance_zero_is_identity() {
        let (r, g, b) = apply_white_balance(0.5, 0.5, 0.5, 0.0, 0.0);
        assert!((r - 0.5).abs() < 1e-6);
        assert!((g - 0.5).abs() < 1e-6);
        assert!((b - 0.5).abs() < 1e-6);
    }

    #[test]
    fn white_balance_warm_boosts_red_reduces_blue() {
        let (r, _g, b) = apply_white_balance(0.5, 0.5, 0.5, 50.0, 0.0);
        assert!(r > 0.5);
        assert!(b < 0.5);
    }

    #[test]
    fn white_balance_cool_boosts_blue_reduces_red() {
        let (r, _g, b) = apply_white_balance(0.5, 0.5, 0.5, -50.0, 0.0);
        assert!(r < 0.5);
        assert!(b > 0.5);
    }

    #[test]
    fn white_balance_tint_positive_reduces_green() {
        let (_r, g, _b) = apply_white_balance(0.5, 0.5, 0.5, 0.0, 50.0);
        assert!(g < 0.5);
    }

    #[test]
    fn white_balance_output_non_negative() {
        let (r, g, b) = apply_white_balance(0.5, 0.5, 0.5, 100.0, 100.0);
        assert!(r >= 0.0);
        assert!(g >= 0.0);
        assert!(b >= 0.0);
    }
}
