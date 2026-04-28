# Conceptual explanations

This sub-section discusses the architectural and design choices that shape AgX. Pages are oriented around understanding *why* AgX is built the way it is, not *what* to look up.

If you want to look up a specific fact (a CLI flag, a preset field, a color-space conversion formula), see the [reference section](../../reference/cli.md). If you want to learn AgX from scratch, start with the [tutorials](../../tutorials/getting-started.md).

## Pages

- [Architecture](architecture.md) — how the codebase is layered and why those layers are load-bearing.
- [Preset-first philosophy](philosophy.md) — what "preset-first" means and what AgX deliberately is and isn't.
- [Design decisions](design-decisions.md) — load-bearing invariants and the choices that produced them.
- [Render pipeline](render-pipeline.md) — why the pipeline runs stages in the order it does.
- [Preset model](preset-model.md) — the patch-on-baseline mental model behind partial parameters.
- [Color spaces](color-spaces.md) — why operations live in linear vs sRGB gamma space.
- [How AgX generates its bundled LUTs](lut-generation.md) — the design choices behind the `agx-lut-gen` crate.

## See also

- [Algorithm explanations](../algorithms/index.md) — per-algorithm walkthroughs.
- [Conceptual reference](../../reference/concepts/index.md) — lookup-style coverage of the same topics.
