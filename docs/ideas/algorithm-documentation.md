# Algorithm Documentation

**Category:** Documentation
**Status:** Backlog

## Problem / Opportunity

AgX implements several image processing algorithms drawn from academic literature and industry practice: Dark Channel Prior (dehaze), guided filter, unsharp mask, wavelet denoising, etc. The code implements these algorithms but doesn't explain the math, the papers they come from, or why specific constants were chosen.

A human-readable reference document (or set of documents) explaining each algorithm would help:
- Contributors understand *why* the code does what it does, not just *what* it does
- Users understand what each slider actually controls under the hood
- Future maintainers evaluate trade-offs when modifying or replacing algorithms

## Key Considerations

- One document per algorithm or group of related algorithms
- Include: intuition, math (kept accessible), paper references, why we chose specific constants/thresholds
- Reference the source code locations where each algorithm is implemented
- Keep it separate from code comments — this is explanatory prose, not API docs
- Could live in `docs/algorithms/` or similar

## Scope

Cover at minimum:
- Dark Channel Prior and atmospheric scattering model (dehaze)
- Guided filter (dehaze refinement)
- Unsharp mask and frequency separation (detail pass)
- Wavelet denoising / à trous decomposition (noise reduction, once implemented)
- Tone curve interpolation (Fritsch-Carlson monotone cubic)
- Luminance weighting for color grading (3-way lift/gamma/gain)
