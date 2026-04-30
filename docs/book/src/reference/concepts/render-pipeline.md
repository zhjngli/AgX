# Render pipeline

A render in AgX takes an input image (decoded from JPEG, PNG, TIFF, or raw) through a fixed sequence of stages and produces an output image. Each stage applies one kind of adjustment in the color space where its math is correct.

## Stages

```mermaid
flowchart TD
    Decode["Decode<br/>(JPEG, PNG, TIFF, raw)"]
    LinearEntry["Linear sRGB"]
    WB["White balance<br/>(linear)"]
    Exposure["Exposure<br/>(linear)"]
    Dehaze["Dehaze<br/>(linear)"]
    Denoise["Noise reduction<br/>(linear)"]
    LinearToGamma["Convert to sRGB gamma"]
    Tonal["Contrast, highlights,<br/>shadows, whites, blacks<br/>(sRGB gamma)"]
    ToneCurves["Tone curves<br/>(sRGB gamma)"]
    HSL["HSL adjustments<br/>(sRGB gamma)"]
    ColorGrading["Color grading<br/>(sRGB gamma)"]
    LUT["LUT<br/>(sRGB gamma)"]
    Detail["Detail pass<br/>(sharpen, clarity, texture)<br/>(sRGB gamma)"]
    Grain["Grain<br/>(sRGB gamma)"]
    Vignette["Vignette<br/>(sRGB gamma)"]
    GammaToLinear["Convert to linear sRGB"]
    Encode["Encode<br/>(JPEG, PNG, TIFF)"]

    Decode --> LinearEntry
    LinearEntry --> WB
    WB --> Exposure
    Exposure --> Dehaze
    Dehaze --> Denoise
    Denoise --> LinearToGamma
    LinearToGamma --> Tonal
    Tonal --> ToneCurves
    ToneCurves --> HSL
    HSL --> ColorGrading
    ColorGrading --> LUT
    LUT --> Detail
    Detail --> Grain
    Grain --> Vignette
    Vignette --> GammaToLinear
    GammaToLinear --> Encode
```

## Color space discipline

Each stage runs in the color space where its math is physically or perceptually correct. The pipeline does the linear-to-gamma conversion in the middle to switch from physical to perceptual operations. See [Color spaces](color-spaces.md) for the linear-vs-gamma distinction and the per-stage table.

## See also

- [Why pipeline order matters](../../explanation/concepts/render-pipeline.md) — the design rationale for the stage sequence.
- [Color spaces](color-spaces.md) — the linear-vs-gamma distinction the pipeline lives in.
- [Preset model](preset-model.md) — how a preset's parameters map to pipeline stages.
- [Algorithm explanations](../../explanation/algorithms/index.md) — algorithm-by-algorithm walkthroughs in pipeline order.
