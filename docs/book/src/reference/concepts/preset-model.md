# Preset model

A preset is a portable, human-readable description of an edit. AgX's preset model has three parts:

1. **Metadata** — name, version, author, and an optional `extends` reference.
2. **Partial parameters** — a set of overrides on the engine's default parameters. Any parameter the preset doesn't mention keeps the default.
3. **Optional LUT** — a `.cube` file path applied at the LUT stage of the pipeline.

The combination is enough to reproduce an edit from a clean image without any GUI state, sidecar file, or hidden context.

## Why partial parameters

Most edits change only a handful of knobs. A film-emulation preset might set saturation, white balance, a tone curve, and a LUT — leaving everything else at default. Modeling parameters as *partial* (every field is optional) means the preset only describes what it changes, and a reader can see the intent at a glance.

This also makes presets composable: a base "neutral starting point" preset can be extended by a "warm cinematic" preset that only specifies the warmth and cinematic-curve overrides.

## The `extends` chain

A preset can declare an `extends` field inside its `[metadata]` block to inherit from another preset:

```toml
[metadata]
name = "warm-cinematic"
extends = "neutral-base.toml"
```

AgX resolves the chain at load time:

1. Load the parent preset and its partial parameters.
2. Load the child preset and its partial parameters.
3. Merge: child overrides parent on every field the child specifies; child inherits everything else.

The chain can be arbitrarily deep. A leaf preset specifies only its incremental changes from its parent; the parent specifies its incremental changes from its parent; and so on up to a base preset (or to the engine defaults if no `extends` is set).

The merge is **recursive through composite sections, last-write-wins at the leaf**. AgX walks each top-level partial section (`tone`, `hsl`, `tone_curve`, `color_grading`, `vignette`, `dehaze`, `noise_reduction`, `grain`, `detail`) and merges fields from the parent and child by union. The child's specified fields win at the leaf level; any field the child doesn't mention is inherited from the parent.

Concretely, if the parent sets `tone_curve.luma` and the child sets `tone_curve.rgb`, the merged preset has both — the child does not replace the parent's `luma` curve just because both presets opened a `[tone_curve]` table. If both parent and child set `tone_curve.luma`, the child's curve fully replaces the parent's at that leaf — AgX does not interpolate or merge individual control points within a single curve.

## Mental model

Think of a preset as a *patch* applied to a baseline render, not as a complete description of an edit. The baseline is "engine defaults applied to the input image." Each preset in the `extends` chain stacks its overrides on the baseline. The final rendered image is the result of applying every override in chain order.

This mental model is why `extends` is useful: it reflects how editing actually works — small adjustments layered on top of larger style choices, each of which builds on a more general starting point.

## See also

- [Preset format](../preset.md) — auto-generated field-by-field schema reference.
- [Render pipeline](render-pipeline.md) — where in the pipeline each parameter takes effect.
