# Architecture

AgX is a layered Rust library and CLI. This page discusses why the layers are shaped the way they are, what the contracts between them are, and which constraints are load-bearing.

If you want to look up the dependency rules or run the architectural tests, see `ARCHITECTURE.md` at the repo root — that file is the contract; this page is the discussion.

## The layers

The `agx` crate splits its source into eight modules: `error` (the foundation type), `adjust` (per-pixel and per-buffer math), `lut` (3D LUT parsing and lookup), `decode` (files into pixels), `metadata` (EXIF and ICC bytes), `encode` (pixels into files), `preset` (TOML serialization of parameter sets), and `engine` (orchestration, including the dual CPU and GPU pipelines). The `agx-cli` crate sits on top, consuming only the public library API. Three workspace crates exist for development: `agx-docgen` generates reference markdown, `agx-lut-gen` produces the bundled `.cube` LUTs, and `agx-e2e` runs golden-file tests.

The dependency direction is strictly bottom-up. `error` is the foundation — every other module imports from it; it imports from nothing. Above sit `adjust`, `lut`, `decode`, `metadata`, and `encode` — mostly siblings, with `decode → metadata → encode` forming a small sub-layering for raw-file I/O (metadata reads two helpers from decode; encode reads `ImageMetadata` from metadata). `preset` consumes `Parameters` from engine and `Lut3D` from lut, but defines no engine logic. At the top, `engine` pulls in `adjust`, `lut`, and `preset` and orchestrates rendering. `agx-cli` depends only on `agx`, never on internal modules. The structural test in `crates/agx/tests/architecture.rs` enforces these rules and fails any change that crosses a forbidden boundary.

## Why this shape

`adjust` cannot import from `engine` because `adjust` is pure pixel math. Each function takes scalar or buffer inputs and returns the same, with the color space documented per function. There is no state and no orchestration — the engine decides iteration order and pipeline sequencing. Letting `adjust` reach into `engine` would entangle math with state, and the math would no longer be testable in isolation. The parameter structs and pre-compute helpers in `adjust` that the GPU path depends on also require `adjust` to stay free of orchestration concerns — the GPU path reimplements pixel math in WGSL, sharing those structs without calling into `adjust` for computation.

`engine` sits at the top of the library because it owns orchestration: which adjustments to apply, in which order, and which of the two pipelines (CPU via Rust + rayon, or GPU via wgpu + WGSL) to dispatch on. Putting orchestration anywhere else would push pipeline knowledge into the math modules — exactly what the `adjust` boundary forbids. `Parameters` lives in `engine` because the parameter shape is dictated by what the pipeline consumes.

`agx-cli` depends only on the public library API for the same reason any other consumer would. A future GUI, a batch worker, or a third-party application all need the same surface. If the CLI needs functionality the library does not expose, the right answer is to expose it, not to reach past the API.

`decode` and `encode` are siblings of `adjust`, not stages of a pipeline. Image I/O is independent of editing math; tying them together would force every consumer to think about both. A caller with decoded pixels already in memory should be able to use the engine without a file system, and one that wants to inspect output should be able to call `encode` without running an edit.

`preset` depends on `lut` (for `Lut3D`) and `engine` (for `Parameters`); these types are what a TOML document declares. The dependency rules permit `engine` to import from `preset` — and it does: `apply_preset()` and `layer_preset()` read parameter values out of a parsed preset. The conceptual flow is still one-directional: preset values flow into the engine; the engine never asks preset to compute anything. Serialization is a preset concern, not an engine concern.

`metadata` stays out of every other module because it carries no semantic load on the rendered image. EXIF bytes flow from input to output without being interpreted. ICC profile bytes on output come from the encoder, which always labels the file as sRGB. Pixel logic never touches metadata; metadata never touches pixels.

## Core invariants and why they exist

The five invariants from `ARCHITECTURE.md` each carry weight. This section discusses what each one prevents.

### Always re-render from original

The engine holds an immutable original image and a mutable parameter state. Every `render()` call starts from the original and replays all adjustments in pipeline order. There is no incremental editing surface and no operation history.

The invariant prevents two failure modes. The first is accumulated rounding error from sequential editing — repeatedly mutating a working buffer drifts pixels away from where a direct render would land them. The second is the order-sensitivity of "apply X, then Y, then undo X," which an editor with operation state must either model precisely or paper over. Re-rendering sidesteps both: every render is a function of `(original, parameters)`, with no path dependence.

The cost is that every parameter change forces a full pipeline replay. Dual-pipeline parallelization has made this acceptable on consumer hardware — rayon plus the optional GPU path keep render times fast enough for batch workflows.

### Declarative presets

A preset is a TOML document declaring parameter values, not an operation sequence. A preset says `exposure = +1.0`, never "apply exposure +1.0 after white balance." Parameters are partial; values not declared fall back to defaults or to whatever a parent preset declared via `extends`.

The declarative shape is what makes presets portable, composable, and inspectable. A reader can open a TOML file and see exactly which knobs the preset turns. The `extends` chain works because there is no hidden order to reconcile — applying preset B on top of preset A is a recursive last-write-wins merge, not a replay of two operation logs.

The alternative considered was an operation log, which is what most photo editors use under the hood. That model was rejected because it ties presets to engine version (operation names, parameter shapes, and ordering rules become part of the preset's contract) and reintroduces the order-sensitivity that the always-re-render-from-original invariant exists to remove.

### Wide working space (linear Rec.2020)

All internal processing uses linear Rec.2020 for physical operations and gamma-encoded Rec.2020 (the sRGB transfer curve applied to Rec.2020 linear values) for perceptual operations. The sRGB transfer curve shape carries over, so anchor points like the 0.5 midtone keep their perceptual meaning; what's different is the wider gamut underneath. Decode converts inputs (sRGB / BT.709 matrix, Display P3 matrix, BT.2020 SDR identity) into linear Rec.2020; encode converts linear Rec.2020 to 8-bit sRGB at output.

What this avoids is squashing wide-gamut inputs at the decode boundary. iPhone HEIC photos tagged Display P3 keep their vivid reds and saturated greens through every edit; the final clamp to display gamut happens only at encode. ICC profile reading from input images, wide-gamut output, and HDR transfer curves (PQ/HLG) are intentionally out of scope at this revision; HDR HEIC sources fall back to "treat as sRGB" with a stderr warning. The [color spaces](color-spaces.md) page covers the per-stage placement and the conversion matrices.

### Fixed render order

The engine applies adjustments in a fixed, hardcoded order. The order in which fields appear in a preset, in `Parameters`, or in API calls has no effect on the output. Render order is an engine implementation detail, not a user-facing concept.

This works because each stage is designed to run in the color space and pipeline position where its math is correct. Exposure runs first, in linear space, before any tonal stage that operates on perceptual values; dehaze runs before denoise so denoise can clean up artifacts dehaze has amplified; LUTs run as their own stage after the per-pixel adjustments, sampling in the space declared by the LUT's encoding (sRGB gamma for `srgb`, linear sRGB for `linear`), with the executor inserting the conversion bracket automatically; detail, grain, and vignette follow. Moving a stage breaks an assumption a downstream stage depends on. The [render pipeline](render-pipeline.md) page walks through the worked examples.

The cost is that stages are not user-reorderable. The trade-off was made for predictability: a preset produces the same output regardless of how it was authored.

### Dual pipeline, same output

The engine has CPU and GPU pipelines that execute the same stages in the same order. CPU is the canonical path — deterministic across platforms, used for golden-file testing and as the fallback when no GPU adapter is available. GPU is opt-in via `Engine::new_gpu_auto()` or `--gpu`, and runs the same stage list on a wgpu device using WGSL compute shaders. Cross-path consistency tests in `gpu_consistency.rs` verify both produce near-identical output.

Both paths exist because the trade-offs are different. CPU is portable, deterministic, and easy to reason about; the test suite trusts it. GPU is faster on machines with capable hardware, especially for per-pixel and convolution-heavy stages, and is opt-in precisely because the canonical path cannot depend on a GPU vendor's driver.

The cost is that every new adjustment is implemented twice — once in `adjust`, once in WGSL — and the cross-path test surface grows with the feature set. Adding a CPU stage without a GPU dispatcher is a half-finished feature.

## Negative constraints in practice

The "what does NOT exist" list in `ARCHITECTURE.md` is as load-bearing as the dependency rules. The invariants describe what the system does; the negative constraints describe what each module deliberately is not.

The most common temptation is to push file I/O into `adjust`. An adjustment might want a profile, a calibration table, or an auxiliary asset; reading from disk inside the math function is the path of least resistance. The right answer is to load the asset upstream — in `decode` if it is part of the input file, in `preset` if a TOML document references it, or in the engine if it is a runtime resource — and pass the parsed data in as a parameter.

When GPU support landed, dual-path coverage forced new boundary discipline. The temptation was to scatter wgpu types and shader bindings across modules that already held the CPU implementation. The architecture instead keeps GPU code inside `engine/gpu/`, with WGSL shaders dispatched from per-stage functions. The CPU path delegates math to `adjust`; the GPU path reimplements the same algorithms in WGSL. Neither leaks into `adjust`.

A subtler temptation is to short-circuit the rules with re-exports. If `engine` re-exported a type from `adjust`, a sibling could import it from `engine` and technically pass the test. This is still a violation in spirit: the dependency the rule prevents has just been laundered. When a re-export is the right tool — hoisting `Parameters` so consumers do not have to know its internal location — it is a documented public API choice, not a way to dodge a rule.

The structural test catches drift early, but interpreting a failure is the contributor's job. The right reaction is "find the right module" or "extract a shared type to a lower layer," not "weaken the test."

## When the architecture should evolve

The rules are not eternal. When a new feature genuinely needs a boundary change, AgX evolves the architecture deliberately rather than letting boundaries drift: any new cross-module dependency is justified explicitly, the structural tests that enforce the rules are updated to match, and the change is reviewed under the same scrutiny as any other architectural shift. The goal is not to prevent change, but to make boundary changes visible and intentional rather than accidental.

## See also

- [Design decisions](design-decisions.md) — the cross-cutting decisions that produced these invariants.
- [Render pipeline](render-pipeline.md) — why the pipeline order is fixed.
- [Color spaces](color-spaces.md) — why linear Rec.2020 is the working space.
- [Preset model](preset-model.md) — how presets fit into the layered design.
