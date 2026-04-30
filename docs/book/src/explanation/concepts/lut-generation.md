# How AgX generates its bundled LUTs

AgX bundles a set of `.cube` LUTs derived from film-emulation parameters. The `agx-lut-gen` crate produces these as a build step. This page explains the design choices behind the generation process — why the LUTs are pre-built rather than computed at runtime, and what the parameters mean.

AgX ships a set of generated `.cube` LUTs used by the e2e test pipeline and as starting points for film-emulation looks. The generator lives in the `agx-lut-gen` crate (`crates/agx-lut-gen/`); the bundled outputs live in `crates/agx-e2e/fixtures/looks/luts/`.

The generator is a small Rust binary that, for each named LUT, evaluates a transformation function over a regular 3D grid in input RGB and writes the result as a `.cube` file. The current set splits into two categories:

- **Film stocks:** Portra 400, Kodachrome 64, Cinestill 800T, Tmax 100, Tri-X 400.
- **Stylistic looks:** Blade Runner, Cinema Warm, Dune, Faded BW, High Contrast BW, Neo Noir.

Why generate LUTs in code rather than authoring them in a colorist tool:

- **Reproducibility.** Each LUT is the deterministic output of a versioned function. Bug fixes or design changes propagate by re-running the generator.
- **Test stability.** The e2e suite compares rendered output against committed golden images. A regenerated LUT that introduces unintended color shifts shows up as a visual diff in the test suite.
- **Onboarding.** Contributors can read the generator source to understand how each look is constructed, rather than treating each LUT as an opaque file.

## See also

- [LUT format](../../reference/concepts/lut-format.md) — the `.cube` file format reference.
