# Render pipeline

A render in AgX takes an input image (decoded from JPEG, PNG, TIFF, or raw) through a fixed sequence of stages and produces an output image. Each stage applies one kind of adjustment in the color space where its math is correct.

## Stages

```mermaid
flowchart TD
    Decode["Decode<br/>(JPEG, PNG, TIFF, raw)"]
    LinearEntry["Linear Rec.2020"]
    WB["White balance<br/>(linear Rec.2020)"]
    Exposure["Exposure<br/>(linear Rec.2020)"]
    Dehaze["Dehaze<br/>(linear Rec.2020)"]
    Denoise["Noise reduction<br/>(linear Rec.2020)"]
    ConvToGamma["Auto-conversion:<br/>Linear → Gamma Rec.2020"]
    Tonal["Contrast, highlights,<br/>shadows, whites, blacks<br/>(Gamma Rec.2020)"]
    ToneCurves["Tone curves<br/>(Gamma Rec.2020)"]
    HSL["HSL adjustments<br/>(Gamma Rec.2020)"]
    ColorGrading["Color grading<br/>(Gamma Rec.2020)"]
    ConvToLut["Auto-conversion:<br/>to LUT encoding space"]
    LUT["LUT<br/>(sRGB gamma or linear sRGB,<br/>per lut.encoding)"]
    ConvFromLut["Auto-conversion:<br/>back to Gamma Rec.2020"]
    Detail["Detail pass<br/>(sharpen, clarity, texture)<br/>(Gamma Rec.2020)"]
    Grain["Grain<br/>(Gamma Rec.2020)"]
    Vignette["Vignette<br/>(Gamma Rec.2020)"]
    ConvToLinear["Auto-conversion:<br/>Gamma → Linear Rec.2020"]
    Encode["Encode<br/>(Linear Rec.2020 → output gamut:<br/>matrix + transfer + quantize)"]

    Decode --> LinearEntry
    LinearEntry --> WB
    WB --> Exposure
    Exposure --> Dehaze
    Dehaze --> Denoise
    Denoise --> ConvToGamma
    ConvToGamma --> Tonal
    Tonal --> ToneCurves
    ToneCurves --> HSL
    HSL --> ColorGrading
    ColorGrading --> ConvToLut
    ConvToLut --> LUT
    LUT --> ConvFromLut
    ConvFromLut --> Detail
    Detail --> Grain
    Grain --> Vignette
    Vignette --> ConvToLinear
    ConvToLinear --> Encode
```

## Color space discipline

Each stage runs in the color space where its math is physically or perceptually correct. The pipeline does the linear-to-gamma conversion in the middle to switch from physical to perceptual operations. Conversions between stages are inserted automatically by the pipeline executor based on each stage's declared input and output color space. The LUT stage samples in the space declared by `lut.encoding` (`srgb` by default; `linear` for linear-light LUTs). See [Color spaces](color-spaces.md) for the linear-vs-gamma distinction and the per-stage table.

## See also

- [Why pipeline order matters](../../explanation/concepts/render-pipeline.md) — the design rationale for the stage sequence.
- [Color spaces](color-spaces.md) — the linear-vs-gamma distinction the pipeline lives in.
- [Preset model](preset-model.md) — how a preset's parameters map to pipeline stages.
- [Algorithm explanations](../../explanation/algorithms/index.md) — algorithm-by-algorithm walkthroughs in pipeline order.
