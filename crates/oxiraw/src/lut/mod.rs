pub mod cube;

/// A 3D Look-Up Table for color transformation.
///
/// Maps input RGB values to output RGB values via a pre-computed 3D lattice.
/// Input values between lattice points are trilinearly interpolated.
///
/// # Format
///
/// The standard interchange format is `.cube` (Adobe/Resolve). Use
/// [`Lut3D::from_cube_str`] or [`Lut3D::from_cube_file`] to load one.
///
/// # Color Space
///
/// LUTs are color-space-dependent. Most creative `.cube` LUTs expect sRGB
/// gamma input in the 0.0–1.0 range. The oxiraw engine applies the LUT in
/// sRGB gamma space after tone adjustments.
#[derive(Debug, Clone)]
pub struct Lut3D {
    /// Optional title from the .cube file header.
    pub title: Option<String>,
    /// Cube dimension — N in N×N×N (e.g. 33 means 35,937 entries).
    pub size: usize,
    /// Minimum input value per channel (default [0.0, 0.0, 0.0]).
    pub domain_min: [f32; 3],
    /// Maximum input value per channel (default [1.0, 1.0, 1.0]).
    pub domain_max: [f32; 3],
    /// The lookup table data — `size^3` RGB output entries.
    /// Ordered with R changing fastest, then G, then B.
    pub table: Vec<[f32; 3]>,
}

impl Lut3D {
    /// Parse a 3D LUT from a `.cube` format string.
    pub fn from_cube_str(text: &str) -> crate::error::Result<Self> {
        cube::parse_cube(text)
    }

    /// Load a 3D LUT from a `.cube` file.
    pub fn from_cube_file(path: &std::path::Path) -> crate::error::Result<Self> {
        let text = std::fs::read_to_string(path)?;
        Self::from_cube_str(&text)
    }

    /// Look up an RGB value in the 3D LUT using trilinear interpolation.
    ///
    /// Input values are clamped to the domain range. For values between
    /// lattice points, the 8 surrounding cube vertices are blended.
    ///
    /// # Arguments
    ///
    /// * `r`, `g`, `b` — input color values (typically 0.0–1.0 in sRGB gamma space)
    ///
    /// # Returns
    ///
    /// The transformed (r, g, b) output color.
    pub fn lookup(&self, r: f32, g: f32, b: f32) -> (f32, f32, f32) {
        let n = (self.size - 1) as f32;

        // Normalize input to 0..1 range within domain, then scale to lattice coordinates
        let rx = ((r - self.domain_min[0]) / (self.domain_max[0] - self.domain_min[0]))
            .clamp(0.0, 1.0)
            * n;
        let gx = ((g - self.domain_min[1]) / (self.domain_max[1] - self.domain_min[1]))
            .clamp(0.0, 1.0)
            * n;
        let bx = ((b - self.domain_min[2]) / (self.domain_max[2] - self.domain_min[2]))
            .clamp(0.0, 1.0)
            * n;

        // Integer lattice indices (lower corner of the cell)
        let r0 = (rx.floor() as usize).min(self.size - 2);
        let g0 = (gx.floor() as usize).min(self.size - 2);
        let b0 = (bx.floor() as usize).min(self.size - 2);

        // Fractional position within the cell
        let fr = rx - r0 as f32;
        let fg = gx - g0 as f32;
        let fb = bx - b0 as f32;

        // Index into flat table: index = r + g*size + b*size*size
        let s = self.size;
        let idx =
            |r: usize, g: usize, b: usize| -> &[f32; 3] { &self.table[r + g * s + b * s * s] };

        // Fetch 8 corners of the cube cell
        let c000 = idx(r0, g0, b0);
        let c100 = idx(r0 + 1, g0, b0);
        let c010 = idx(r0, g0 + 1, b0);
        let c110 = idx(r0 + 1, g0 + 1, b0);
        let c001 = idx(r0, g0, b0 + 1);
        let c101 = idx(r0 + 1, g0, b0 + 1);
        let c011 = idx(r0, g0 + 1, b0 + 1);
        let c111 = idx(r0 + 1, g0 + 1, b0 + 1);

        // Trilinear interpolation
        let mut out = [0.0f32; 3];
        for i in 0..3 {
            let c00 = c000[i] * (1.0 - fr) + c100[i] * fr;
            let c10 = c010[i] * (1.0 - fr) + c110[i] * fr;
            let c01 = c001[i] * (1.0 - fr) + c101[i] * fr;
            let c11 = c011[i] * (1.0 - fr) + c111[i] * fr;

            let c0 = c00 * (1.0 - fg) + c10 * fg;
            let c1 = c01 * (1.0 - fg) + c11 * fg;

            out[i] = c0 * (1.0 - fb) + c1 * fb;
        }

        (out[0], out[1], out[2])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_cube() {
        let cube_text = "\
LUT_3D_SIZE 2
0.0 0.0 0.0
1.0 0.0 0.0
0.0 1.0 0.0
1.0 1.0 0.0
0.0 0.0 1.0
1.0 0.0 1.0
0.0 1.0 1.0
1.0 1.0 1.0
";
        let lut = Lut3D::from_cube_str(cube_text).unwrap();
        assert_eq!(lut.size, 2);
        assert_eq!(lut.table.len(), 8);
        assert_eq!(lut.domain_min, [0.0, 0.0, 0.0]);
        assert_eq!(lut.domain_max, [1.0, 1.0, 1.0]);
    }

    #[test]
    fn parse_cube_with_header() {
        let cube_text = "\
TITLE \"Test LUT\"
LUT_3D_SIZE 2
DOMAIN_MIN 0.0 0.0 0.0
DOMAIN_MAX 1.0 1.0 1.0
# This is a comment
0.0 0.0 0.0
1.0 0.0 0.0
0.0 1.0 0.0
1.0 1.0 0.0
0.0 0.0 1.0
1.0 0.0 1.0
0.0 1.0 1.0
1.0 1.0 1.0
";
        let lut = Lut3D::from_cube_str(cube_text).unwrap();
        assert_eq!(lut.title.as_deref(), Some("Test LUT"));
        assert_eq!(lut.size, 2);
    }

    #[test]
    fn parse_cube_missing_size_returns_error() {
        let cube_text = "0.0 0.0 0.0\n";
        let result = Lut3D::from_cube_str(cube_text);
        assert!(result.is_err());
    }

    #[test]
    fn parse_cube_wrong_entry_count_returns_error() {
        let cube_text = "\
LUT_3D_SIZE 2
0.0 0.0 0.0
1.0 0.0 0.0
";
        let result = Lut3D::from_cube_str(cube_text);
        assert!(result.is_err());
    }

    #[test]
    fn parse_cube_malformed_line_returns_error() {
        let cube_text = "\
LUT_3D_SIZE 2
0.0 0.0 0.0
not a number
0.0 1.0 0.0
1.0 1.0 0.0
0.0 0.0 1.0
1.0 0.0 1.0
0.0 1.0 1.0
1.0 1.0 1.0
";
        let result = Lut3D::from_cube_str(cube_text);
        assert!(result.is_err());
    }

    #[test]
    fn load_cube_file() {
        let temp_path = std::env::temp_dir().join("oxiraw_test.cube");
        let cube_text = "\
LUT_3D_SIZE 2
0.0 0.0 0.0
1.0 0.0 0.0
0.0 1.0 0.0
1.0 1.0 0.0
0.0 0.0 1.0
1.0 0.0 1.0
0.0 1.0 1.0
1.0 1.0 1.0
";
        std::fs::write(&temp_path, cube_text).unwrap();
        let lut = Lut3D::from_cube_file(&temp_path).unwrap();
        assert_eq!(lut.size, 2);
        assert_eq!(lut.table.len(), 8);
        let _ = std::fs::remove_file(&temp_path);
    }

    #[test]
    fn load_nonexistent_cube_file_returns_error() {
        let result = Lut3D::from_cube_file(std::path::Path::new("/nonexistent/file.cube"));
        assert!(result.is_err());
    }

    // --- Trilinear interpolation tests ---

    fn make_identity_lut(size: usize) -> Lut3D {
        let n = size as f32 - 1.0;
        let mut table = Vec::with_capacity(size * size * size);
        for b in 0..size {
            for g in 0..size {
                for r in 0..size {
                    table.push([r as f32 / n, g as f32 / n, b as f32 / n]);
                }
            }
        }
        Lut3D {
            title: None,
            size,
            domain_min: [0.0, 0.0, 0.0],
            domain_max: [1.0, 1.0, 1.0],
            table,
        }
    }

    #[test]
    fn lookup_identity_at_lattice_points() {
        let lut = make_identity_lut(17);
        let (r, g, b) = lut.lookup(0.0, 0.0, 0.0);
        assert!((r - 0.0).abs() < 1e-6);
        assert!((g - 0.0).abs() < 1e-6);
        assert!((b - 0.0).abs() < 1e-6);

        let (r, g, b) = lut.lookup(1.0, 1.0, 1.0);
        assert!((r - 1.0).abs() < 1e-6);
        assert!((g - 1.0).abs() < 1e-6);
        assert!((b - 1.0).abs() < 1e-6);
    }

    #[test]
    fn lookup_identity_interpolated() {
        let lut = make_identity_lut(17);
        let (r, g, b) = lut.lookup(0.3, 0.5, 0.7);
        assert!((r - 0.3).abs() < 0.01, "Expected ~0.3, got {}", r);
        assert!((g - 0.5).abs() < 0.01, "Expected ~0.5, got {}", g);
        assert!((b - 0.7).abs() < 0.01, "Expected ~0.7, got {}", b);
    }

    #[test]
    fn lookup_clamps_out_of_range() {
        let lut = make_identity_lut(17);
        let (r, g, b) = lut.lookup(-0.5, 1.5, 0.5);
        assert!(
            (r - 0.0).abs() < 1e-6,
            "Negative should clamp to 0, got {}",
            r
        );
        assert!(
            (g - 1.0).abs() < 1e-6,
            "Above 1 should clamp to 1, got {}",
            g
        );
        assert!((b - 0.5).abs() < 0.01);
    }

    #[test]
    fn lookup_transforms_values() {
        // Build a simple LUT that inverts: output = 1-input
        let size = 2;
        let table = vec![
            // b=0, g=0: r=0..1
            [1.0, 1.0, 1.0], // (0,0,0) -> (1,1,1)
            [0.0, 1.0, 1.0], // (1,0,0) -> (0,1,1)
            // b=0, g=1: r=0..1
            [1.0, 0.0, 1.0], // (0,1,0) -> (1,0,1)
            [0.0, 0.0, 1.0], // (1,1,0) -> (0,0,1)
            // b=1, g=0: r=0..1
            [1.0, 1.0, 0.0], // (0,0,1) -> (1,1,0)
            [0.0, 1.0, 0.0], // (1,0,1) -> (0,1,0)
            // b=1, g=1: r=0..1
            [1.0, 0.0, 0.0], // (0,1,1) -> (1,0,0)
            [0.0, 0.0, 0.0], // (1,1,1) -> (0,0,0)
        ];
        let lut = Lut3D {
            title: None,
            size,
            domain_min: [0.0, 0.0, 0.0],
            domain_max: [1.0, 1.0, 1.0],
            table,
        };

        let (r, g, b) = lut.lookup(0.0, 0.0, 0.0);
        assert!((r - 1.0).abs() < 1e-6);
        assert!((g - 1.0).abs() < 1e-6);
        assert!((b - 1.0).abs() < 1e-6);

        let (r, g, b) = lut.lookup(1.0, 1.0, 1.0);
        assert!((r - 0.0).abs() < 1e-6);
        assert!((g - 0.0).abs() < 1e-6);
        assert!((b - 0.0).abs() < 1e-6);

        // Midpoint of an inversion LUT should be ~0.5
        let (r, g, b) = lut.lookup(0.5, 0.5, 0.5);
        assert!((r - 0.5).abs() < 1e-6);
        assert!((g - 0.5).abs() < 1e-6);
        assert!((b - 0.5).abs() < 1e-6);
    }
}
