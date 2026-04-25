# Render pipeline

A render in AgX takes an input image (decoded from JPEG, PNG, TIFF, or raw) through a fixed sequence of stages and produces an output image. Each stage applies one kind of adjustment in the color space where its math is correct.

## Stages

```mermaid
flowchart TD
    Decode["Decode<br/>(JPEG, PNG, TIFF, raw)"]
    LinearEntry["Linear sRGB"]
    WB["White balance<br/>(linear)"]
    Exposure["Exposure<br/>(linear)"]
    LinearToGamma["Convert to sRGB gamma"]
    Tonal["Contrast, highlights,<br/>shadows, whites, blacks<br/>(sRGB gamma)"]
    HSL["HSL adjustments<br/>(sRGB gamma)"]
    ColorGrading["Color grading<br/>(sRGB gamma)"]
    ToneCurves["Tone curves<br/>(sRGB gamma)"]
    LUT["LUT<br/>(sRGB gamma)"]
    Detail["Detail pass<br/>(sharpen, clarity, texture)"]
    Dehaze["Dehaze"]
    Denoise["Noise reduction"]
    Grain["Grain"]
    Vignette["Vignette"]
    GammaToLinear["Convert to linear sRGB"]
    Encode["Encode<br/>(JPEG, PNG, TIFF)"]

    Decode --> LinearEntry
    LinearEntry --> WB
    WB --> Exposure
    Exposure --> LinearToGamma
    LinearToGamma --> Tonal
    Tonal --> HSL
    HSL --> ColorGrading
    ColorGrading --> ToneCurves
    ToneCurves --> LUT
    LUT --> Denoise
    Denoise --> Dehaze
    Dehaze --> Detail
    Detail --> Grain
    Grain --> Vignette
    Vignette --> GammaToLinear
    GammaToLinear --> Encode
```

## Color space discipline

Each stage runs in the color space where its math is physically or perceptually correct. The pipeline does the linear-to-gamma conversion in the middle to switch from physical to perceptual operations.

See [Color spaces](color-spaces.md) for the linear-vs-gamma distinction and per-stage rationale.

## Why pipeline order matters

Stage order is not interchangeable. A few examples:

- **Exposure before tonal sliders.** Exposure scales linear light; tonal sliders re-shape the resulting brightness landscape. Reversing them would re-shape unexposed values, then scale the result, which produces different output.
- **LUT before detail.** The LUT applies a creative color transform; sharpening and clarity then operate on the graded result. Sharpening before the LUT would amplify noise that the LUT then re-grades.
- **Denoise before dehaze and detail.** Noise reduction smooths the image; dehaze then increases local contrast on the smoothed result. If dehaze ran first, it would amplify noise that denoise would then have to remove.
- **Grain after detail and dehaze, before vignette.** Grain is added texture; the surrounding stages should not modify it. Vignette is a final overlay that doesn't disturb grain structure.

The stage order encodes design decisions made when each adjustment was added. The [explanation pages](../../explanation/index.md) for each algorithm record the reasoning.

## See also

- [Color spaces](color-spaces.md) — the linear-vs-gamma distinction the pipeline lives in.
- [Preset model](preset-model.md) — how a preset's parameters map to pipeline stages.
- [Explanation index](../../explanation/index.md) — algorithm-by-algorithm walkthroughs in pipeline order.
