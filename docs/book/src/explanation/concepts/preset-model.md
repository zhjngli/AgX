# Preset model

A preset is a description of an edit. This page explains the design decisions behind that description: why presets describe what they change rather than the full parameter set, and the patch-on-baseline mental model that follows from that choice.

If you want to look up how `extends` resolution works mechanically, see the [preset model reference](../../reference/concepts/preset-model.md).

## Why partial parameters

Most edits change only a handful of knobs. A film-emulation preset might set saturation, white balance, a tone curve, and a LUT — leaving everything else at default. Modeling parameters as *partial* (every field is optional) means the preset only describes what it changes, and a reader can see the intent at a glance.

This also makes presets composable: a base "neutral starting point" preset can be extended by a "warm cinematic" preset that only specifies the warmth and cinematic-curve overrides.

## Mental model

Think of a preset as a *patch* applied to a baseline render, not as a complete description of an edit. The baseline is "engine defaults applied to the input image." Each preset in the `extends` chain stacks its overrides on the baseline. The final rendered image is the result of applying every override in chain order.

This mental model is why `extends` is useful: it reflects how editing actually works — small adjustments layered on top of larger style choices, each of which builds on a more general starting point.

## See also

- [Preset model reference](../../reference/concepts/preset-model.md) — the lookup view of `extends` resolution.
- [Preset format](../../reference/preset.md) — auto-generated field schema.
- [Preset-first philosophy](philosophy.md) — why AgX is built around presets in the first place.
