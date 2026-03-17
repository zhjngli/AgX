use crate::transforms::*;

/// Apply a per-channel transform to (r, g, b).
fn map_channels(r: f32, g: f32, b: f32, f: impl Fn(f32) -> f32) -> (f32, f32, f32) {
    (f(r), f(g), f(b))
}

/// Clamp all channels to [0, 1].
fn clamp_rgb(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    (r.clamp(0.0, 1.0), g.clamp(0.0, 1.0), b.clamp(0.0, 1.0))
}

/// A named look: title + transform function.
pub struct Look {
    pub name: &'static str,
    pub transform: fn(f32, f32, f32) -> (f32, f32, f32),
}

/// Return all 6 film-inspired looks.
pub fn all_looks() -> Vec<Look> {
    vec![
        Look {
            name: "portra_400",
            transform: portra_400,
        },
        Look {
            name: "neo_noir",
            transform: neo_noir,
        },
        Look {
            name: "blade_runner",
            transform: blade_runner,
        },
        Look {
            name: "cinema_warm",
            transform: cinema_warm,
        },
        Look {
            name: "kodachrome_64",
            transform: kodachrome_64,
        },
        Look {
            name: "nordic_fade",
            transform: nordic_fade,
        },
    ]
}

/// Portra 400 — soft film-shoulder highlight curve, blue shadow lift,
/// gentle desaturation, warm highlight gain.
fn portra_400(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    // Film shoulder for soft highlight rolloff
    let (r, g, b) = map_channels(r, g, b, |x| film_shoulder(x, 0.25));

    // Lift/gamma/gain: blue shadow lift, warm highlight gain
    let r = lift_gamma_gain(r, 0.02, 1.0, 1.02);
    let g = lift_gamma_gain(g, 0.01, 1.0, 1.0);
    let b = lift_gamma_gain(b, 0.04, 1.0, 0.97);

    // Gentle desaturation
    let (r, g, b) = scale_saturation(r, g, b, 0.88);

    clamp_rgb(r, g, b)
}

/// Neo Noir — aggressive S-curve, cool shadow lift, teal shadow push,
/// heavy desaturation.
fn neo_noir(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    // Aggressive S-curve for high contrast
    let (r, g, b) = map_channels(r, g, b, |x| s_curve(x, 0.7, 0.5));

    // Cool shadow lift
    let r = lift_gamma_gain(r, 0.01, 1.0, 1.0);
    let g = lift_gamma_gain(g, 0.01, 1.0, 1.0);
    let b = lift_gamma_gain(b, 0.03, 1.0, 0.98);

    // Cool shadow hue rotation
    let (r, g, b) = hue_rotate_in_lum_range(r, g, b, 30.0, 0.2, 0.4);

    // Heavy desaturation
    let (r, g, b) = scale_saturation(r, g, b, 0.45);

    clamp_rgb(r, g, b)
}

/// Blade Runner — moderate S-curve, teal shadows + warm/orange highlights,
/// slight saturation boost.
fn blade_runner(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    // Moderate S-curve
    let (r, g, b) = map_channels(r, g, b, |x| s_curve(x, 0.4, 0.5));

    // Teal shadows: elevated blue and green lift
    // Warm/orange highlights: elevated red gain, reduced blue gain
    let r = lift_gamma_gain(r, 0.01, 1.0, 1.06);
    let g = lift_gamma_gain(g, 0.03, 1.0, 1.0);
    let b = lift_gamma_gain(b, 0.04, 1.0, 0.92);

    // Slight saturation boost
    let (r, g, b) = scale_saturation(r, g, b, 1.05);

    clamp_rgb(r, g, b)
}

/// Cinema Warm — soft film shoulder, golden midtone via red->green crossfeed,
/// warm overall shift via lift/gamma/gain.
fn cinema_warm(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    // Soft film shoulder
    let (r, g, b) = map_channels(r, g, b, |x| film_shoulder(x, 0.20));

    // Golden midtone via red->green crossfeed
    let (r, g, b) = crossfeed(r, g, b, 0, 1, 0.04);

    // Warm overall shift
    let r = lift_gamma_gain(r, 0.02, 0.95, 1.03);
    let g = lift_gamma_gain(g, 0.01, 1.0, 1.0);
    let b = lift_gamma_gain(b, 0.0, 1.05, 0.95);

    clamp_rgb(r, g, b)
}

/// Kodachrome 64 — per-channel S-curves (strong red/blue, moderate green),
/// red->green crossfeed, elevated saturation, deep blacks.
fn kodachrome_64(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    // Per-channel S-curves with different strengths
    let r = s_curve(r, 0.5, 0.5);
    let g = s_curve(g, 0.3, 0.5);
    let b = s_curve(b, 0.5, 0.5);

    // Red->green crossfeed for warmth
    let (r, g, b) = crossfeed(r, g, b, 0, 1, 0.03);

    // Elevated saturation
    let (r, g, b) = scale_saturation(r, g, b, 1.12);

    clamp_rgb(r, g, b)
}

/// Nordic Fade — lifted blacks, compressed highlights, cool hue rotation,
/// heavy desaturation, slight green midtone elevation.
fn nordic_fade(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    // Lifted blacks + compressed highlights
    let (r, g, b) = map_channels(r, g, b, |x| lift_gamma_gain(x, 0.10, 1.0, 0.88));

    // Slight green midtone elevation
    let g = lift_gamma_gain(g, 0.0, 0.92, 1.0);

    // Cool hue rotation in midtones
    let (r, g, b) = hue_rotate_in_lum_range(r, g, b, -15.0, 0.5, 0.8);

    // Heavy desaturation
    let (r, g, b) = scale_saturation(r, g, b, 0.60);

    clamp_rgb(r, g, b)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn in_range(v: f32) -> bool {
        (0.0..=1.0).contains(&v)
    }

    /// Every look must produce output in [0,1] for any input in [0,1].
    #[test]
    fn all_looks_clamp_output() {
        let test_values = [0.0, 0.1, 0.25, 0.5, 0.75, 0.9, 1.0];
        for look in all_looks() {
            for &r in &test_values {
                for &g in &test_values {
                    for &b in &test_values {
                        let (ro, go, bo) = (look.transform)(r, g, b);
                        assert!(
                            in_range(ro) && in_range(go) && in_range(bo),
                            "{}: ({},{},{}) -> ({},{},{})",
                            look.name,
                            r,
                            g,
                            b,
                            ro,
                            go,
                            bo
                        );
                    }
                }
            }
        }
    }

    /// Looks should not be identity — they should actually change colors.
    #[test]
    fn looks_are_not_identity() {
        let test_rgb = (0.5, 0.3, 0.7);
        for look in all_looks() {
            let (r, g, b) = (look.transform)(test_rgb.0, test_rgb.1, test_rgb.2);
            let unchanged = (r - test_rgb.0).abs() < 0.001
                && (g - test_rgb.1).abs() < 0.001
                && (b - test_rgb.2).abs() < 0.001;
            assert!(!unchanged, "{} should not be identity", look.name);
        }
    }

    /// Looks should produce distinguishable results from each other.
    #[test]
    fn looks_are_distinct() {
        let test_rgb = (0.5, 0.3, 0.7);
        let looks = all_looks();
        for i in 0..looks.len() {
            for j in (i + 1)..looks.len() {
                let (r1, g1, b1) = (looks[i].transform)(test_rgb.0, test_rgb.1, test_rgb.2);
                let (r2, g2, b2) = (looks[j].transform)(test_rgb.0, test_rgb.1, test_rgb.2);
                let diff = (r1 - r2).abs() + (g1 - g2).abs() + (b1 - b2).abs();
                assert!(
                    diff > 0.01,
                    "{} and {} produce nearly identical results",
                    looks[i].name,
                    looks[j].name
                );
            }
        }
    }
}
