# LUT Format Reference

This document describes 3D LUTs, the `.cube` file format, and how oxiraw handles them.

## What Is a LUT?

A LUT (Look-Up Table) is a pre-computed color transformation. Instead of defining a transformation as a formula (like "multiply by 2"), a LUT stores the result for every possible input. Given an input RGB color, you look up the corresponding output RGB color in the table.

LUTs are widely used for:
- **Film emulation**: Mimicking the look of specific film stocks (Portra 400, Ektar 100, Tri-X)
- **Color grading**: Applying a cinematic color grade (teal and orange, bleach bypass, etc.)
- **Technical transforms**: Converting between color spaces or log curves
- **Creative looks**: Any arbitrary color transformation

### 1D vs 3D LUTs

**1D LUT**: Three separate curves, one per channel (R, G, B). Each channel is transformed independently. Fast but limited: cannot do cross-channel effects (e.g., "when red is high, boost blue"). Essentially the same as three tone curves.

**3D LUT**: A three-dimensional grid indexed by input R, G, B. Each grid point stores an output RGB value. Because the grid is indexed by all three channels simultaneously, 3D LUTs can represent any color transformation, including cross-channel effects. This is what oxiraw supports.

## The `.cube` Format

The `.cube` format was defined by Adobe for use in DaVinci Resolve and has become the de facto standard for LUT interchange. It is supported by virtually every editing tool: Lightroom, Photoshop, Resolve, Capture One, Final Cut Pro, Premiere, Affinity Photo, and many more.

It is a plain text file with a simple structure.

### Header Keywords

```
TITLE "Film Emulation"
LUT_3D_SIZE 33
DOMAIN_MIN 0.0 0.0 0.0
DOMAIN_MAX 1.0 1.0 1.0
```

| Keyword | Required | Default | Description |
|---------|----------|---------|-------------|
| `TITLE "name"` | No | none | Descriptive name for the LUT |
| `LUT_3D_SIZE N` | Yes | - | Cube dimension: creates N x N x N entries |
| `DOMAIN_MIN r g b` | No | `0.0 0.0 0.0` | Minimum input value per channel |
| `DOMAIN_MAX r g b` | No | `1.0 1.0 1.0` | Maximum input value per channel |

Lines starting with `#` are comments and are ignored.

### Data Section

After the header, each line contains three space-separated floating-point numbers representing the output R, G, B values for one grid point:

```
0.000000 0.000000 0.000000
0.003906 0.000000 0.000000
0.007812 0.000000 0.000000
...
```

There must be exactly N^3 data lines (e.g., 35,937 lines for a 33x33x33 LUT).

### Entry Ordering

Entries are ordered with **R changing fastest**, then G, then B. In pseudocode:

```
for b in 0..N:
    for g in 0..N:
        for r in 0..N:
            write output_rgb[r][g][b]
```

The flat array index for input (r, g, b) is: `r + g*N + b*N*N`.

## Trilinear Interpolation

A 33x33x33 LUT only stores output values for 33 evenly-spaced points along each axis. For input values that fall between lattice points, the output is **trilinearly interpolated** from the 8 surrounding grid points.

This is analogous to bilinear interpolation in 2D (used in image scaling), extended to 3D:

1. Find the cell containing the input point (the 8 surrounding lattice vertices)
2. Compute the fractional position within the cell (0.0 to 1.0 in each axis)
3. Interpolate along R (4 pairs -> 4 values)
4. Interpolate along G (2 pairs -> 2 values)
5. Interpolate along B (1 pair -> 1 value)

The result smoothly blends between grid points, producing continuous color transitions.

## Common LUT Sizes

| Size | Entries | File Size (~) | Quality | Use Case |
|------|---------|--------------|---------|----------|
| 17 | 4,913 | ~100 KB | Good | Lightweight, fast loading |
| 33 | 35,937 | ~700 KB | Very good | Standard for most LUTs |
| 65 | 274,625 | ~5 MB | Excellent | High precision, technical use |

Most creative LUTs use size 33, which provides excellent quality with reasonable file size. Larger sizes offer diminishing returns for most color grades.

## What oxiraw Supports

**Supported:**
- 3D LUTs in `.cube` format
- `TITLE`, `LUT_3D_SIZE`, `DOMAIN_MIN`, `DOMAIN_MAX` header keywords
- Trilinear interpolation
- Any cube size (commonly 17, 33, 65)
- Comments (`#` lines)
- Applied in sRGB gamma space after tone adjustments

**Not supported (currently):**
- 1D LUTs (`LUT_1D_SIZE` keyword is ignored, not an error)
- Shaper LUTs (1D pre-processing before 3D lookup)
- Tetrahedral interpolation (trilinear is used instead; the quality difference is minimal)
- Non-sRGB input spaces (log curves, linear)
- `.3dl`, `.csp`, `.icc`, or other LUT formats

## Where to Find `.cube` LUTs

Many free LUTs are available online:
- Film emulation packs (Fuji, Kodak, etc.)
- Cinematic color grades
- Black and white conversions
- Technical conversion LUTs

When using third-party LUTs, verify they expect sRGB gamma input (most creative LUTs do). LUTs designed for video log input (S-Log3, LogC) will produce incorrect results in oxiraw.
