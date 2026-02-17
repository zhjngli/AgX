# LUT Support Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add 3D LUT support — parse `.cube` files, apply via trilinear interpolation in the render pipeline, integrate with presets and CLI, and document everything.

**Architecture:** New `lut` module with `Lut3D` struct. The `.cube` parser is hand-written (~100 lines). LUT is applied in sRGB gamma space after tone adjustments, before converting back to linear. The engine holds an optional `Lut3D` on a separate field (not on `Parameters`, since LUTs aren't serializable as simple values). Presets reference LUTs via a `[lut] path = "..."` section resolved relative to the preset file.

**Tech Stack:** Rust 2021, no new dependencies (pure Rust parser + interpolation)

---

## Context

This plan implements the design in `docs/plans/2026-02-16-lut-support-design.md`. Read that document for background on what LUTs are, the `.cube` format specification, color space considerations, and the full list of design decisions.

Key points:
- `.cube` is a plain text format: header keywords (`TITLE`, `LUT_3D_SIZE`, `DOMAIN_MIN`, `DOMAIN_MAX`) followed by N^3 lines of `R G B` float triplets
- Entry ordering: R changes fastest, then G, then B
- Trilinear interpolation blends the 8 surrounding cube vertices for any input RGB
- LUT is applied in sRGB gamma space (step 5 in the render pipeline, after tone adjustments)

## Critical Files

| File | Purpose |
|------|---------|
| `crates/oxiraw/src/lut/mod.rs` | Create: `Lut3D` struct, `lookup()` with trilinear interpolation |
| `crates/oxiraw/src/lut/cube.rs` | Create: `.cube` format parser |
| `crates/oxiraw/src/lib.rs` | Modify: add `pub mod lut;` and re-export |
| `crates/oxiraw/src/error.rs` | Modify: add `Lut` error variant |
| `crates/oxiraw/src/engine/mod.rs` | Modify: add optional LUT field, apply in render pipeline |
| `crates/oxiraw/src/preset/mod.rs` | Modify: add `[lut]` section, resolve relative paths |
| `crates/oxiraw-cli/src/main.rs` | Modify: add `--lut` flag to `edit` subcommand |
| `crates/oxiraw-cli/tests/integration.rs` | Modify: add LUT integration tests |
| `docs/reference/color-spaces.md` | Create: color space reference documentation |
| `docs/reference/lut-format.md` | Create: `.cube` format reference documentation |
| `README.md` | Modify: add LUT section |
| `example/luts/identity.cube` | Create: sample identity LUT for testing |

---

## Phase 1: Core LUT Module

### Task 1.1: Add LUT error variant

**Files:**
- Modify: `crates/oxiraw/src/error.rs`

**Step 1: Write failing test**

Add to the existing `mod tests` block in `error.rs`:

```rust
#[test]
fn error_display_lut() {
    let err = OxirawError::Lut("invalid size".into());
    assert_eq!(err.to_string(), "LUT error: invalid size");
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p oxiraw error::tests::error_display_lut`
Expected: FAIL — no variant `Lut`

**Step 3: Write implementation**

Add the variant to `OxirawError` in `error.rs`, after the `Preset` variant:

```rust
#[error("LUT error: {0}")]
Lut(String),
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p oxiraw error::tests`
Expected: PASS (all 5 error tests)

**Step 5: Stage**

`git add crates/oxiraw/src/error.rs`

---

### Task 1.2: .cube parser

**Files:**
- Create: `crates/oxiraw/src/lut/cube.rs`
- Create: `crates/oxiraw/src/lut/mod.rs`
- Modify: `crates/oxiraw/src/lib.rs`

**Step 1: Write failing tests**

Create `crates/oxiraw/src/lut/mod.rs` with the `Lut3D` struct and tests. Create `crates/oxiraw/src/lut/cube.rs` with just enough to compile. Add `pub mod lut;` to `lib.rs`.

```rust
// crates/oxiraw/src/lut/mod.rs
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
        assert_eq!(lut.table.len(), 8); // 2^3
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
}
```

```rust
// crates/oxiraw/src/lut/cube.rs
// (initially empty — implementation goes here in step 3)
```

Add to `crates/oxiraw/src/lib.rs` after `pub mod error;`:

```rust
pub mod lut;
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p oxiraw lut::tests`
Expected: FAIL — `from_cube_str` and `from_cube_file` not defined

**Step 3: Write implementation**

Implement the parser in `crates/oxiraw/src/lut/cube.rs`:

```rust
// crates/oxiraw/src/lut/cube.rs

use crate::error::{OxirawError, Result};
use super::Lut3D;

/// Parse a `.cube` format string into a `Lut3D`.
///
/// The `.cube` format is a plain text format defined by Adobe for DaVinci Resolve.
/// It is the de facto standard for LUT interchange across photo and video editing tools.
///
/// # Supported keywords
///
/// - `TITLE "name"` — optional title
/// - `LUT_3D_SIZE N` — required, defines an N×N×N cube
/// - `DOMAIN_MIN r g b` — input minimum per channel (default 0.0 0.0 0.0)
/// - `DOMAIN_MAX r g b` — input maximum per channel (default 1.0 1.0 1.0)
/// - Lines starting with `#` are comments
///
/// # Data layout
///
/// After the header, each line contains three space-separated floats (R G B output).
/// Entries are ordered with R changing fastest, then G, then B.
pub fn parse_cube(text: &str) -> Result<Lut3D> {
    let mut title: Option<String> = None;
    let mut size: Option<usize> = None;
    let mut domain_min = [0.0f32, 0.0, 0.0];
    let mut domain_max = [1.0f32, 1.0, 1.0];
    let mut table: Vec<[f32; 3]> = Vec::new();

    for (line_num, line) in text.lines().enumerate() {
        let line = line.trim();

        // Skip empty lines and comments
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Parse header keywords
        if let Some(rest) = line.strip_prefix("TITLE") {
            let rest = rest.trim();
            let unquoted = rest.trim_matches('"');
            title = Some(unquoted.to_string());
            continue;
        }

        if let Some(rest) = line.strip_prefix("LUT_3D_SIZE") {
            size = Some(rest.trim().parse::<usize>().map_err(|_| {
                OxirawError::Lut(format!("line {}: invalid LUT_3D_SIZE", line_num + 1))
            })?);
            continue;
        }

        if let Some(rest) = line.strip_prefix("DOMAIN_MIN") {
            domain_min = parse_rgb_line(rest.trim(), line_num)?;
            continue;
        }

        if let Some(rest) = line.strip_prefix("DOMAIN_MAX") {
            domain_max = parse_rgb_line(rest.trim(), line_num)?;
            continue;
        }

        // Skip 1D LUT keywords (unsupported, but don't error)
        if line.starts_with("LUT_1D_SIZE") || line.starts_with("LUT_1D_INPUT_RANGE")
            || line.starts_with("LUT_3D_INPUT_RANGE") || line.starts_with("LUT_IN_VIDEO_RANGE")
            || line.starts_with("LUT_OUT_VIDEO_RANGE")
        {
            continue;
        }

        // Data line — three floats
        let rgb = parse_rgb_line(line, line_num)?;
        table.push(rgb);
    }

    let size = size.ok_or_else(|| OxirawError::Lut("missing LUT_3D_SIZE".into()))?;
    let expected = size * size * size;

    if table.len() != expected {
        return Err(OxirawError::Lut(format!(
            "expected {} entries for size {}, got {}",
            expected, size, table.len()
        )));
    }

    Ok(Lut3D {
        title,
        size,
        domain_min,
        domain_max,
        table,
    })
}

fn parse_rgb_line(line: &str, line_num: usize) -> Result<[f32; 3]> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() != 3 {
        return Err(OxirawError::Lut(format!(
            "line {}: expected 3 values, got {}",
            line_num + 1,
            parts.len()
        )));
    }
    let r = parts[0].parse::<f32>().map_err(|_| {
        OxirawError::Lut(format!("line {}: invalid float '{}'", line_num + 1, parts[0]))
    })?;
    let g = parts[1].parse::<f32>().map_err(|_| {
        OxirawError::Lut(format!("line {}: invalid float '{}'", line_num + 1, parts[1]))
    })?;
    let b = parts[2].parse::<f32>().map_err(|_| {
        OxirawError::Lut(format!("line {}: invalid float '{}'", line_num + 1, parts[2]))
    })?;
    Ok([r, g, b])
}
```

Then add the `from_cube_str` and `from_cube_file` methods to `Lut3D` in `lut/mod.rs`:

```rust
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
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p oxiraw lut::tests`
Expected: PASS (all 7 parser tests)

**Step 5: Stage**

`git add crates/oxiraw/src/lut/ crates/oxiraw/src/lib.rs`

---

### Task 1.3: Trilinear interpolation lookup

**Files:**
- Modify: `crates/oxiraw/src/lut/mod.rs`

**Step 1: Write failing tests**

Add to the `mod tests` block in `lut/mod.rs`:

```rust
// --- Identity LUT: output = input ---

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
    // Exact lattice points should return input unchanged
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
    // Between lattice points, identity LUT should still return ~input
    let (r, g, b) = lut.lookup(0.3, 0.5, 0.7);
    assert!((r - 0.3).abs() < 0.01, "Expected ~0.3, got {}", r);
    assert!((g - 0.5).abs() < 0.01, "Expected ~0.5, got {}", g);
    assert!((b - 0.7).abs() < 0.01, "Expected ~0.7, got {}", b);
}

#[test]
fn lookup_clamps_out_of_range() {
    let lut = make_identity_lut(17);
    // Values outside 0-1 should be clamped
    let (r, g, b) = lut.lookup(-0.5, 1.5, 0.5);
    assert!((r - 0.0).abs() < 1e-6, "Negative should clamp to 0, got {}", r);
    assert!((g - 1.0).abs() < 1e-6, "Above 1 should clamp to 1, got {}", g);
    assert!((b - 0.5).abs() < 0.01);
}

#[test]
fn lookup_transforms_values() {
    // Build a simple LUT that inverts: output = 1-input
    let size = 2;
    let table = vec![
        // b=0, g=0: r=0..1
        [1.0, 1.0, 1.0], // (0,0,0) → (1,1,1)
        [0.0, 1.0, 1.0], // (1,0,0) → (0,1,1)
        // b=0, g=1: r=0..1
        [1.0, 0.0, 1.0], // (0,1,0) → (1,0,1)
        [0.0, 0.0, 1.0], // (1,1,0) → (0,0,1)
        // b=1, g=0: r=0..1
        [1.0, 1.0, 0.0], // (0,0,1) → (1,1,0)
        [0.0, 1.0, 0.0], // (1,0,1) → (0,1,0)
        // b=1, g=1: r=0..1
        [1.0, 0.0, 0.0], // (0,1,1) → (1,0,0)
        [0.0, 0.0, 0.0], // (1,1,1) → (0,0,0)
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
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p oxiraw lut::tests`
Expected: FAIL — `lookup` method not defined

**Step 3: Write implementation**

Add to the `impl Lut3D` block in `lut/mod.rs`:

```rust
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
        .clamp(0.0, 1.0) * n;
    let gx = ((g - self.domain_min[1]) / (self.domain_max[1] - self.domain_min[1]))
        .clamp(0.0, 1.0) * n;
    let bx = ((b - self.domain_min[2]) / (self.domain_max[2] - self.domain_min[2]))
        .clamp(0.0, 1.0) * n;

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
    let idx = |r: usize, g: usize, b: usize| -> &[f32; 3] {
        &self.table[r + g * s + b * s * s]
    };

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
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p oxiraw lut::tests`
Expected: PASS (all 11 LUT tests)

**Step 5: Stage**

`git add crates/oxiraw/src/lut/mod.rs`

---

## Phase 2: Engine + Preset Integration

### Task 2.1: Engine LUT integration

**Files:**
- Modify: `crates/oxiraw/src/engine/mod.rs`

**Step 1: Write failing tests**

Add to the `mod tests` block in `engine/mod.rs`:

```rust
#[test]
fn render_with_identity_lut_is_identity() {
    let img = make_test_image(0.5, 0.3, 0.1);
    let mut engine = Engine::new(img);
    // Build an identity LUT
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
    engine.set_lut(Some(lut));

    let rendered = engine.render();
    let orig = engine.original().get_pixel(0, 0);
    let rend = rendered.get_pixel(0, 0);
    for i in 0..3 {
        assert!(
            (orig.0[i] - rend.0[i]).abs() < 0.01,
            "Channel {}: expected ~{}, got {}",
            i, orig.0[i], rend.0[i]
        );
    }
}

#[test]
fn render_with_no_lut_unchanged() {
    // Verify existing behavior: no LUT = same as before
    let img = make_test_image(0.5, 0.3, 0.1);
    let engine = Engine::new(img);
    assert!(engine.lut().is_none());
    let rendered = engine.render();
    let orig = engine.original().get_pixel(0, 0);
    let rend = rendered.get_pixel(0, 0);
    for i in 0..3 {
        assert!((orig.0[i] - rend.0[i]).abs() < 1e-5);
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p oxiraw engine::tests`
Expected: FAIL — `set_lut` and `lut` methods not defined

**Step 3: Write implementation**

Add to `Engine` struct in `engine/mod.rs`:

1. Add a `lut` field:

```rust
pub struct Engine {
    original: Rgb32FImage,
    params: Parameters,
    lut: Option<crate::lut::Lut3D>,
}
```

2. Update `Engine::new()` to initialize `lut: None`.

3. Add accessor methods:

```rust
/// Get a reference to the current LUT, if any.
pub fn lut(&self) -> Option<&crate::lut::Lut3D> {
    self.lut.as_ref()
}

/// Set or clear the 3D LUT.
pub fn set_lut(&mut self, lut: Option<crate::lut::Lut3D>) {
    self.lut = lut;
}
```

4. Update `apply_preset` to also set the LUT (for now, presets don't have LUTs yet, so clear it):

```rust
pub fn apply_preset(&mut self, preset: &crate::preset::Preset) {
    self.params = preset.params.clone();
    self.lut = preset.lut.clone();
}
```

5. In `render()`, insert LUT application after blacks (step 8), before converting back to linear (step 9):

```rust
// 9. LUT (sRGB gamma space)
if let Some(lut) = &self.lut {
    let (lr, lg, lb) = lut.lookup(sr, sg, sb);
    sr = lr;
    sg = lg;
    sb = lb;
}

// 10. Convert back to linear space
let (lr, lg, lb) = adjust::srgb_to_linear(sr, sg, sb);
```

6. Update the render doc comment to include the LUT step.

**Step 4: Run test to verify it passes**

Run: `cargo test -p oxiraw engine::tests`
Expected: PASS (all 9 engine tests)

**Step 5: Stage**

`git add crates/oxiraw/src/engine/mod.rs`

---

### Task 2.2: Preset LUT support

**Files:**
- Modify: `crates/oxiraw/src/preset/mod.rs`

**Step 1: Write failing tests**

Add to `mod tests` in `preset/mod.rs`:

```rust
#[test]
fn preset_with_lut_path_loads_lut() {
    let temp_dir = std::env::temp_dir();
    let cube_path = temp_dir.join("oxiraw_preset_test.cube");
    let preset_path = temp_dir.join("oxiraw_preset_test.toml");

    // Write a minimal identity 2x2x2 .cube file
    std::fs::write(&cube_path, "\
LUT_3D_SIZE 2
0.0 0.0 0.0
1.0 0.0 0.0
0.0 1.0 0.0
1.0 1.0 0.0
0.0 0.0 1.0
1.0 0.0 1.0
0.0 1.0 1.0
1.0 1.0 1.0
").unwrap();

    // Write a preset referencing the LUT by filename only (relative)
    let toml_content = format!(
        "[metadata]\nname = \"LUT Test\"\n\n[tone]\nexposure = 0.5\n\n[lut]\npath = \"{}\"\n",
        cube_path.file_name().unwrap().to_str().unwrap()
    );
    std::fs::write(&preset_path, &toml_content).unwrap();

    let preset = Preset::load_from_file(&preset_path).unwrap();
    assert_eq!(preset.params.exposure, 0.5);
    assert!(preset.lut.is_some());
    assert_eq!(preset.lut.as_ref().unwrap().size, 2);

    let _ = std::fs::remove_file(&cube_path);
    let _ = std::fs::remove_file(&preset_path);
}

#[test]
fn preset_without_lut_section_has_no_lut() {
    let toml_str = "[metadata]\nname = \"No LUT\"\n\n[tone]\nexposure = 1.0\n";
    let preset = Preset::from_toml(toml_str).unwrap();
    assert!(preset.lut.is_none());
}

#[test]
fn preset_with_missing_lut_file_returns_error() {
    let temp_dir = std::env::temp_dir();
    let preset_path = temp_dir.join("oxiraw_missing_lut_test.toml");
    std::fs::write(&preset_path,
        "[metadata]\nname = \"Bad\"\n\n[lut]\npath = \"nonexistent.cube\"\n"
    ).unwrap();

    let result = Preset::load_from_file(&preset_path);
    assert!(result.is_err());

    let _ = std::fs::remove_file(&preset_path);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p oxiraw preset::tests`
Expected: FAIL — `lut` field doesn't exist on `Preset`

**Step 3: Write implementation**

1. Add a `LutSection` struct and update `PresetRaw`:

```rust
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct LutSection {
    #[serde(default)]
    path: Option<String>,
}
```

Add `#[serde(default)] lut: LutSection` to `PresetRaw`.

2. Add `lut` field to `Preset`:

```rust
pub struct Preset {
    pub metadata: PresetMetadata,
    pub params: Parameters,
    pub lut: Option<crate::lut::Lut3D>,
}
```

Update `Default` for `Preset` to set `lut: None`.

Update `PartialEq` — since `Lut3D` doesn't derive `PartialEq`, implement it manually by comparing `metadata` and `params` only (the LUT is loaded from a file path, not a value type). Alternatively, derive `PartialEq` on `Lut3D`.

3. Update `from_toml` to store the LUT path string without loading (loading requires a base directory):

The tricky part: `from_toml` doesn't know the file path, so it can't resolve relative paths. The solution:
- `from_toml` stores the raw LUT path string but doesn't load the LUT (sets `lut: None`)
- `load_from_file` calls `from_toml`, then if a LUT path was specified, resolves it relative to the preset file's parent directory and loads it
- Add a helper: `from_toml_with_base_dir(toml_str, base_dir)` that does both

```rust
pub fn from_toml(toml_str: &str) -> Result<Self> {
    let raw: PresetRaw =
        toml::from_str(toml_str).map_err(|e| OxirawError::Preset(e.to_string()))?;
    Ok(Self {
        metadata: raw.metadata,
        params: Parameters { /* same as before */ },
        lut: None, // No base dir to resolve path
    })
}

pub fn load_from_file(path: &std::path::Path) -> Result<Self> {
    let content = std::fs::read_to_string(path)?;
    let raw: PresetRaw =
        toml::from_str(&content).map_err(|e| OxirawError::Preset(e.to_string()))?;

    let base_dir = path.parent().unwrap_or(std::path::Path::new("."));

    let lut = if let Some(lut_path_str) = &raw.lut.path {
        let lut_path = base_dir.join(lut_path_str);
        Some(crate::lut::Lut3D::from_cube_file(&lut_path)?)
    } else {
        None
    };

    Ok(Self {
        metadata: raw.metadata,
        params: Parameters {
            exposure: raw.tone.exposure,
            contrast: raw.tone.contrast,
            highlights: raw.tone.highlights,
            shadows: raw.tone.shadows,
            whites: raw.tone.whites,
            blacks: raw.tone.blacks,
            temperature: raw.white_balance.temperature,
            tint: raw.white_balance.tint,
        },
        lut,
    })
}
```

4. Note: `to_toml` and `save_to_file` will NOT serialize the LUT data back. The LUT is loaded from a file reference — serialization only stores the path in the TOML. For now, `to_toml` does not include the `[lut]` section (since we don't track the original path). This is acceptable for the MVP.

**Step 4: Run test to verify it passes**

Run: `cargo test -p oxiraw preset::tests`
Expected: PASS (all 10 preset tests)

Also run: `cargo test -p oxiraw` to verify nothing is broken.

**Step 5: Stage**

`git add crates/oxiraw/src/preset/mod.rs`

---

## Phase 3: CLI + Integration Tests

### Task 3.1: CLI `--lut` flag

**Files:**
- Modify: `crates/oxiraw-cli/src/main.rs`

**Step 1: Write implementation**

Add `--lut` flag to the `Edit` subcommand:

```rust
/// Path to a .cube LUT file
#[arg(long)]
lut: Option<PathBuf>,
```

Update the `Edit` arm in `main()` to pass `lut` through.

Update `run_edit` to accept `lut: Option<&Path>` and, if provided, load and set it on the engine:

```rust
if let Some(lut_path) = lut {
    let lut = oxiraw::lut::Lut3D::from_cube_file(lut_path)?;
    engine.set_lut(Some(lut));
}
```

Also update `run_apply` — the preset's LUT is already loaded by `Preset::load_from_file`, so `engine.apply_preset(&preset)` handles it. No changes needed there.

**Step 2: Verify CLI compiles**

Run: `cargo run -p oxiraw-cli -- edit --help`
Expected: shows `--lut <LUT>` in the help output

**Step 3: Stage**

`git add crates/oxiraw-cli/src/main.rs`

---

### Task 3.2: CLI integration tests

**Files:**
- Modify: `crates/oxiraw-cli/tests/integration.rs`

**Step 1: Write tests**

Add to the existing integration test file:

```rust
fn create_identity_cube(path: &std::path::Path) {
    let mut lines = String::from("LUT_3D_SIZE 2\n");
    // Identity 2x2x2: output = input at lattice points
    for b in 0..2 {
        for g in 0..2 {
            for r in 0..2 {
                lines.push_str(&format!("{}.0 {}.0 {}.0\n", r, g, b));
            }
        }
    }
    std::fs::write(path, lines).unwrap();
}

#[test]
fn cli_edit_with_lut() {
    let temp_dir = std::env::temp_dir();
    let input = temp_dir.join("oxiraw_cli_lut_in.png");
    let lut_path = temp_dir.join("oxiraw_cli_test.cube");
    let output = temp_dir.join("oxiraw_cli_lut_out.png");

    create_test_png(&input);
    create_identity_cube(&lut_path);

    let status = cli_bin()
        .args([
            "edit",
            "-i", input.to_str().unwrap(),
            "-o", output.to_str().unwrap(),
            "--lut", lut_path.to_str().unwrap(),
        ])
        .status()
        .expect("failed to run CLI");

    assert!(status.success(), "CLI edit with LUT should succeed");
    assert!(output.exists(), "Output file should exist");

    let _ = std::fs::remove_file(&input);
    let _ = std::fs::remove_file(&lut_path);
    let _ = std::fs::remove_file(&output);
}

#[test]
fn cli_apply_preset_with_lut() {
    let temp_dir = std::env::temp_dir();
    let input = temp_dir.join("oxiraw_cli_preset_lut_in.png");
    let lut_path = temp_dir.join("oxiraw_cli_preset_lut.cube");
    let preset_path = temp_dir.join("oxiraw_cli_preset_lut.toml");
    let output = temp_dir.join("oxiraw_cli_preset_lut_out.png");

    create_test_png(&input);
    create_identity_cube(&lut_path);

    let preset_content = format!(
        "[metadata]\nname = \"LUT Preset\"\n\n[tone]\nexposure = 0.5\n\n[lut]\npath = \"{}\"\n",
        lut_path.file_name().unwrap().to_str().unwrap()
    );
    std::fs::write(&preset_path, &preset_content).unwrap();

    let status = cli_bin()
        .args([
            "apply",
            "-i", input.to_str().unwrap(),
            "-p", preset_path.to_str().unwrap(),
            "-o", output.to_str().unwrap(),
        ])
        .status()
        .expect("failed to run CLI");

    assert!(status.success(), "CLI apply with LUT preset should succeed");
    assert!(output.exists());

    let _ = std::fs::remove_file(&input);
    let _ = std::fs::remove_file(&lut_path);
    let _ = std::fs::remove_file(&preset_path);
    let _ = std::fs::remove_file(&output);
}
```

**Step 2: Run tests**

Run: `cargo test -p oxiraw-cli`
Expected: PASS (all 5 CLI tests)

**Step 3: Stage**

`git add crates/oxiraw-cli/tests/integration.rs`

---

## Phase 4: Documentation + Examples

### Task 4.1: Color spaces reference doc

**Files:**
- Create: `docs/reference/color-spaces.md`

Write the color space explanation covering:
- Linear vs sRGB gamma (with the photon/perception analogy)
- Why exposure/WB live in linear space (physics)
- Why contrast/highlights/shadows live in sRGB gamma space (perceptual)
- Why LUTs live in sRGB gamma space (designed on screens)
- The conversion formulas (power of 2.2, palette crate)
- The full pipeline diagram
- Future: wider gamut, log spaces

**Stage:** `git add docs/reference/color-spaces.md`

---

### Task 4.2: LUT format reference doc

**Files:**
- Create: `docs/reference/lut-format.md`

Write the `.cube` format reference covering:
- What a LUT is (1D vs 3D)
- The `.cube` header keywords with examples
- Data layout and entry ordering
- Trilinear interpolation (conceptual explanation)
- What oxiraw supports and doesn't support (no 1D, no shaper, no tetrahedral)
- Common LUT sizes and their trade-offs (17 vs 33 vs 65)
- Where to find free .cube LUTs

**Stage:** `git add docs/reference/lut-format.md`

---

### Task 4.3: Sample LUT + README updates

**Files:**
- Create: `example/luts/identity.cube` — a 17x17x17 identity LUT (output = input)
- Modify: `README.md` — add LUT section to features, quick start, library usage
- Modify: `example/README.md` — add LUT section
- Modify: `crates/oxiraw/src/lib.rs` — add `pub use lut::Lut3D;` re-export

For the identity LUT, write a small script or generate it inline — 4,913 lines of `r g b` values where output = input.

Update README.md features list:

```markdown
- **3D LUT support**: Apply `.cube` LUT files for color grading and film emulation
```

Add a LUT section to Quick Start:

```bash
# Apply a LUT
cargo run -p oxiraw-cli -- edit -i photo.jpg -o graded.jpg --lut film.cube

# Combine a preset with a LUT
cargo run -p oxiraw-cli -- edit -i photo.jpg -o graded.jpg --exposure 0.5 --lut film.cube
```

Add LUT to the library usage example showing `engine.set_lut(...)`.

**Stage:** `git add example/luts/ README.md example/README.md crates/oxiraw/src/lib.rs`

---

## Phase 5: Final Verification

### Task 5.1: Full test suite

Run: `cargo test --workspace`

Expected: All tests pass (~70+ tests across both crates).

No changes to stage — just verification.

---

## Summary

| Phase | Tasks | Tests Added | Key Deliverable |
|-------|-------|-------------|-----------------|
| 1 | 1.1–1.3 | ~12 | LUT module: parser + trilinear interpolation |
| 2 | 2.1–2.2 | ~5 | Engine + preset integration |
| 3 | 3.1–3.2 | ~2 | CLI `--lut` flag + integration tests |
| 4 | 4.1–4.3 | 0 | Docs + sample LUT + README |
| 5 | 5.1 | 0 | Final verification |
