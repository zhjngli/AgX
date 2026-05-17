#![doc = include_str!("vignette.md")]

use serde::{Deserialize, Serialize};

// --- Vignette (sRGB gamma space, position-dependent) ---

/// Vignette falloff geometry.
#[cfg_attr(
    any(feature = "docgen", feature = "validate"),
    derive(schemars::JsonSchema)
)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VignetteShape {
    /// Elliptical falloff matching the image aspect ratio (default).
    #[default]
    Elliptical,
    /// Circular falloff centered on the image.
    Circular,
}

impl std::fmt::Display for VignetteShape {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Elliptical => write!(f, "elliptical"),
            Self::Circular => write!(f, "circular"),
        }
    }
}

impl std::str::FromStr for VignetteShape {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "elliptical" => Ok(Self::Elliptical),
            "circular" => Ok(Self::Circular),
            _ => Err(format!(
                "invalid vignette shape '{s}'. Use: elliptical or circular"
            )),
        }
    }
}

/// Precomputed loop-invariant values for vignette rendering.
///
/// Create once per render via [`VignettePrecomputed::new`], then call
/// [`apply_vignette_pre`] per pixel. This avoids recomputing `half_w`,
/// `half_h`, `strength`, and per-axis reciprocals on every pixel.
#[derive(Debug, Clone, Copy)]
pub struct VignettePrecomputed {
    half_w: f32,
    half_h: f32,
    inv_x: f32,
    inv_y: f32,
    strength: f32,
}

impl VignettePrecomputed {
    /// Precompute vignette geometry from amount, shape, and image dimensions.
    pub fn new(amount: f32, shape: VignetteShape, w: u32, h: u32) -> Self {
        let half_w = w as f32 / 2.0;
        let half_h = h as f32 / 2.0;
        let (inv_x, inv_y) = match shape {
            VignetteShape::Elliptical => (1.0 / half_w, 1.0 / half_h),
            VignetteShape::Circular => {
                let inv_r = 1.0 / half_w.max(half_h);
                (inv_r, inv_r)
            }
        };
        Self {
            half_w,
            half_h,
            inv_x,
            inv_y,
            strength: amount / 100.0,
        }
    }
}

/// Apply creative vignette using precomputed invariants (hot path).
///
/// Call [`VignettePrecomputed::new`] once, then this function per pixel.
pub fn apply_vignette_pre(
    r: f32,
    g: f32,
    b: f32,
    pre: &VignettePrecomputed,
    x: u32,
    y: u32,
) -> (f32, f32, f32) {
    let dx = (x as f32 - pre.half_w) * pre.inv_x;
    let dy = (y as f32 - pre.half_h) * pre.inv_y;
    let d_sq = dx * dx + dy * dy;

    // Domain-safety: base is used as a weight (factor = base * base). Without the
    // clamp, pixels outside the normalized unit circle (d_sq > 1) would produce a
    // negative base and therefore a negative factor, which would invert the vignette
    // effect for extreme corners. Keeping the clamp bounds the weight to [0, 1].
    let base = (1.0 - d_sq).clamp(0.0, 1.0);
    let factor = base * base;
    let multiplier = 1.0 + pre.strength * (1.0 - factor);

    // Output unclamped; the final clamp is at encode.
    (r * multiplier, g * multiplier, b * multiplier)
}

/// Apply creative vignette to an sRGB gamma pixel (convenience wrapper).
///
/// Darkens (negative amount) or brightens (positive amount) edges based on
/// distance from center. Amount range: -100 to +100. 0 = no effect.
///
/// For batch pixel processing, prefer [`VignettePrecomputed`] + [`apply_vignette_pre`].
#[allow(clippy::too_many_arguments)]
pub fn apply_vignette(
    r: f32,
    g: f32,
    b: f32,
    amount: f32,
    shape: VignetteShape,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
) -> (f32, f32, f32) {
    if amount == 0.0 {
        return (r, g, b);
    }
    apply_vignette_pre(
        r,
        g,
        b,
        &VignettePrecomputed::new(amount, shape, w, h),
        x,
        y,
    )
}

/// Apply vignette to an sRGB gamma buffer in-place using precomputed invariants.
pub fn apply_vignette_buffer(
    buf: &mut [[f32; 3]],
    width: u32,
    height: u32,
    pre: &VignettePrecomputed,
) {
    for y in 0..height {
        for x in 0..width {
            let idx = (y * width + x) as usize;
            let [r, g, b] = buf[idx];
            let (r, g, b) = apply_vignette_pre(r, g, b, pre, x, y);
            buf[idx] = [r, g, b];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Vignette tests ---

    #[test]
    fn vignette_zero_amount_is_identity() {
        let (r, g, b) = super::apply_vignette(
            0.8,
            0.5,
            0.3,
            0.0,
            super::VignetteShape::Elliptical,
            0,
            0,
            100,
            100,
        );
        assert!((r - 0.8).abs() < 1e-6);
        assert!((g - 0.5).abs() < 1e-6);
        assert!((b - 0.3).abs() < 1e-6);
    }

    #[test]
    fn vignette_center_pixel_unchanged() {
        // 100x100 image: half_w = 50.0. Pixel (50, 50) → dx = 0, dy = 0 → factor = 1.0 exactly.
        let (r, g, b) = super::apply_vignette(
            0.8,
            0.5,
            0.3,
            -50.0,
            super::VignetteShape::Elliptical,
            50,
            50,
            100,
            100,
        );
        assert!((r - 0.8).abs() < 1e-6, "r: expected 0.8, got {r}");
        assert!((g - 0.5).abs() < 1e-6, "g: expected 0.5, got {g}");
        assert!((b - 0.3).abs() < 1e-6, "b: expected 0.3, got {b}");
    }

    #[test]
    fn vignette_corner_darkened() {
        let (r, _g, _b) = super::apply_vignette(
            0.8,
            0.5,
            0.3,
            -50.0,
            super::VignetteShape::Elliptical,
            0,
            0,
            100,
            100,
        );
        assert!(r < 0.8, "Corner should be darkened, got r={r}");
    }

    #[test]
    fn vignette_corner_brightened() {
        let (r, _g, _b) = super::apply_vignette(
            0.5,
            0.5,
            0.5,
            50.0,
            super::VignetteShape::Elliptical,
            0,
            0,
            100,
            100,
        );
        assert!(r > 0.5, "Corner should be brightened, got r={r}");
    }

    #[test]
    fn vignette_preserves_out_of_range_input() {
        // OOG red input (1.2) + negative vignette (darkening): multiplier < 1.0
        // but output is unclamped so the result is finite and not pinned to 1.0.
        let (r, g, b) = super::apply_vignette(
            1.2,
            0.5,
            0.5,
            -50.0,
            super::VignetteShape::Elliptical,
            50,
            50,
            100,
            100,
        );
        // Center pixel at (50, 50) in 100x100: d_sq ≈ 0, factor ≈ 1, multiplier ≈ 1.
        // So output ≈ input (1.2), which is OOG but finite.
        assert!(r.is_finite(), "vignette produced non-finite r: {r}");
        assert!(g.is_finite(), "vignette produced non-finite g: {g}");
        assert!(b.is_finite(), "vignette produced non-finite b: {b}");
        // Near-center: multiplier ≈ 1.0, so r ≈ 1.2 (not clamped)
        assert!(r > 1.0, "vignette clamped OOG input at center: {r}");
    }

    #[test]
    fn vignette_circular_top_bottom_darker_than_sides() {
        // 3:2 wide image (300x200). Circular radius = max(150, 100) = 150.
        // Left-center (0, 100): dx=150, dy=0 → d²=(150/150)²=1.0 → factor=0 → full effect.
        // Top-center (150, 0): dx=0, dy=100 → d²=(100/150)²=0.444 → factor=(0.556)²=0.309.
        // Left/right edges are further from center than top/bottom in circular mode on a wide image.
        let (r_top, _, _) = super::apply_vignette(
            0.8,
            0.8,
            0.8,
            -100.0,
            super::VignetteShape::Circular,
            150,
            0,
            300,
            200,
        );
        let (r_left, _, _) = super::apply_vignette(
            0.8,
            0.8,
            0.8,
            -100.0,
            super::VignetteShape::Circular,
            0,
            100,
            300,
            200,
        );
        assert!(
            r_left < r_top,
            "Circular: left edge ({r_left}) should be darker than top edge ({r_top}) on wide image"
        );
    }

    #[test]
    fn vignette_elliptical_edges_even() {
        // 3:2 aspect ratio image (300x200). Elliptical mode: normalized by half_w and half_h.
        // Top-center (150, 0): d² = (0/150)² + (100/100)² = 1.0
        // Left-center (0, 100): d² = (150/150)² + (0/100)² = 1.0
        // Both should have the same darkening.
        let (r_top, _, _) = super::apply_vignette(
            0.8,
            0.8,
            0.8,
            -50.0,
            super::VignetteShape::Elliptical,
            150,
            0,
            300,
            200,
        );
        let (r_left, _, _) = super::apply_vignette(
            0.8,
            0.8,
            0.8,
            -50.0,
            super::VignetteShape::Elliptical,
            0,
            100,
            300,
            200,
        );
        let (r_bottom, _, _) = super::apply_vignette(
            0.8,
            0.8,
            0.8,
            -50.0,
            super::VignetteShape::Elliptical,
            150,
            199,
            300,
            200,
        );
        let (r_right, _, _) = super::apply_vignette(
            0.8,
            0.8,
            0.8,
            -50.0,
            super::VignetteShape::Elliptical,
            299,
            100,
            300,
            200,
        );
        let eps = 0.02; // small tolerance for edge pixel asymmetry
        assert!(
            (r_top - r_left).abs() < eps,
            "Top ({r_top}) and left ({r_left}) should be equal"
        );
        assert!(
            (r_top - r_bottom).abs() < eps,
            "Top ({r_top}) and bottom ({r_bottom}) should be equal"
        );
        assert!(
            (r_top - r_right).abs() < eps,
            "Top ({r_top}) and right ({r_right}) should be equal"
        );
    }

    #[test]
    fn vignette_buffer_darkens_corners() {
        let w = 4u32;
        let h = 4u32;
        let mut buf: Vec<[f32; 3]> = vec![[0.5, 0.5, 0.5]; (w * h) as usize];
        let pre = VignettePrecomputed::new(-50.0, VignetteShape::Elliptical, w, h);
        apply_vignette_buffer(&mut buf, w, h, &pre);
        // Center pixel should be unchanged (or close)
        let center = buf[(w + 1) as usize];
        // Corner pixel should be darker
        let corner = buf[0];
        assert!(
            corner[0] < center[0],
            "corner ({}) should be darker than center ({})",
            corner[0],
            center[0]
        );
    }
}
