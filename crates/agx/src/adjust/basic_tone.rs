#![doc = include_str!("basic_tone.md")]

// --- Contrast (sRGB gamma space) ---

/// Apply contrast adjustment to a single channel value in sRGB gamma space.
/// Contrast range: -100 to +100. 0 = no change.
pub fn apply_contrast(value: f32, contrast: f32) -> f32 {
    if contrast == 0.0 {
        return value;
    }
    let factor = (100.0 + contrast) / 100.0;
    (0.5 + (value - 0.5) * factor).clamp(0.0, 1.0)
}

// --- Highlights (sRGB gamma space) ---

/// Apply highlights adjustment to a single channel value in sRGB gamma space.
/// Targets bright pixels (> 0.5). Range: -100 to +100.
pub fn apply_highlights(value: f32, highlights: f32) -> f32 {
    if highlights == 0.0 || value <= 0.5 {
        return value;
    }
    let weight = (value - 0.5) / 0.5; // 0 at 0.5, 1 at 1.0
    let adjustment = weight * (highlights / 100.0) * 0.5;
    (value + adjustment).clamp(0.0, 1.0)
}

// --- Shadows (sRGB gamma space) ---

/// Apply shadows adjustment to a single channel value in sRGB gamma space.
/// Targets dark pixels (< 0.5). Range: -100 to +100.
pub fn apply_shadows(value: f32, shadows: f32) -> f32 {
    if shadows == 0.0 || value >= 0.5 {
        return value;
    }
    let weight = 1.0 - value / 0.5; // 1 at 0.0, 0 at 0.5
    let adjustment = weight * (shadows / 100.0) * 0.5;
    (value + adjustment).clamp(0.0, 1.0)
}

// --- Whites (sRGB gamma space) ---

/// Apply whites adjustment to a single channel value in sRGB gamma space.
/// Targets upper-range pixels (> 0.75). Range: -100 to +100.
pub fn apply_whites(value: f32, whites: f32) -> f32 {
    if whites == 0.0 || value <= 0.75 {
        return value;
    }
    let weight = (value - 0.75) / 0.25; // 0 at 0.75, 1 at 1.0
    let adjustment = weight * (whites / 100.0) * 0.25;
    (value + adjustment).clamp(0.0, 1.0)
}

// --- Blacks (sRGB gamma space) ---

/// Apply blacks adjustment to a single channel value in sRGB gamma space.
/// Targets lower-range pixels (< 0.25). Range: -100 to +100.
pub fn apply_blacks(value: f32, blacks: f32) -> f32 {
    if blacks == 0.0 || value >= 0.25 {
        return value;
    }
    let weight = 1.0 - value / 0.25; // 1 at 0.0, 0 at 0.25
    let adjustment = weight * (blacks / 100.0) * 0.25;
    (value + adjustment).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Contrast tests ---

    #[test]
    fn contrast_zero_is_identity() {
        assert_eq!(apply_contrast(0.7, 0.0), 0.7);
    }

    #[test]
    fn contrast_positive_increases_deviation() {
        let mid = 0.8;
        let result = apply_contrast(mid, 50.0);
        // Above midpoint should move further from 0.5
        assert!(result > mid);
    }

    #[test]
    fn contrast_negative_decreases_deviation() {
        let mid = 0.8;
        let result = apply_contrast(mid, -50.0);
        assert!(result < mid);
    }

    #[test]
    fn contrast_output_clamped() {
        assert!(apply_contrast(1.0, 100.0) <= 1.0);
        assert!(apply_contrast(0.0, 100.0) >= 0.0);
    }

    // --- Highlights tests ---

    #[test]
    fn highlights_zero_is_identity() {
        assert_eq!(apply_highlights(0.8, 0.0), 0.8);
    }

    #[test]
    fn highlights_dark_pixels_unaffected() {
        assert_eq!(apply_highlights(0.3, 50.0), 0.3);
    }

    #[test]
    fn highlights_negative_darkens_bright() {
        assert!(apply_highlights(0.9, -50.0) < 0.9);
    }

    #[test]
    fn highlights_positive_brightens_bright() {
        assert!(apply_highlights(0.9, 50.0) > 0.9);
    }

    #[test]
    fn highlights_brighter_pixels_affected_more() {
        let change_at_60 = (apply_highlights(0.6, 50.0) - 0.6).abs();
        let change_at_90 = (apply_highlights(0.9, 50.0) - 0.9).abs();
        assert!(change_at_90 > change_at_60);
    }

    // --- Shadows tests ---

    #[test]
    fn shadows_zero_is_identity() {
        assert_eq!(apply_shadows(0.2, 0.0), 0.2);
    }

    #[test]
    fn shadows_bright_pixels_unaffected() {
        assert_eq!(apply_shadows(0.7, 50.0), 0.7);
    }

    #[test]
    fn shadows_positive_lifts_darks() {
        assert!(apply_shadows(0.1, 50.0) > 0.1);
    }

    #[test]
    fn shadows_negative_crushes_darks() {
        assert!(apply_shadows(0.1, -50.0) < 0.1);
    }

    #[test]
    fn shadows_darker_pixels_affected_more() {
        let change_at_10 = (apply_shadows(0.1, 50.0) - 0.1).abs();
        let change_at_40 = (apply_shadows(0.4, 50.0) - 0.4).abs();
        assert!(change_at_10 > change_at_40);
    }

    // --- Whites tests ---

    #[test]
    fn whites_zero_is_identity() {
        assert_eq!(apply_whites(0.9, 0.0), 0.9);
    }

    #[test]
    fn whites_dark_pixels_unaffected() {
        assert_eq!(apply_whites(0.5, 50.0), 0.5);
    }

    #[test]
    fn whites_positive_brightens_upper() {
        assert!(apply_whites(0.9, 50.0) > 0.9);
    }

    #[test]
    fn whites_negative_darkens_upper() {
        assert!(apply_whites(0.9, -50.0) < 0.9);
    }

    // --- Blacks tests ---

    #[test]
    fn blacks_zero_is_identity() {
        assert_eq!(apply_blacks(0.1, 0.0), 0.1);
    }

    #[test]
    fn blacks_bright_pixels_unaffected() {
        assert_eq!(apply_blacks(0.5, 50.0), 0.5);
    }

    #[test]
    fn blacks_positive_lifts() {
        assert!(apply_blacks(0.1, 50.0) > 0.1);
    }

    #[test]
    fn blacks_negative_crushes() {
        assert!(apply_blacks(0.1, -50.0) < 0.1);
    }
}
