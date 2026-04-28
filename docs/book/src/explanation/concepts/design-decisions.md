# Design decisions

This page collects the load-bearing invariants of AgX and the choices that produced them. The bulleted list at the top is a fast scannable answer to "what defines AgX?" The narrative entries below give the reasoning behind each decision and what it costs.

## Load-bearing invariants

- [Always re-render from original](#always-re-render-from-original) — the engine holds an immutable original image and replays all adjustments from scratch on every render.
- [Declarative presets](#declarative-presets) — a preset declares parameter values, not an operation sequence.
- [sRGB-only working space](#srgb-only-working-space) — no wide-gamut, no ICC profile handling.
- [Fixed render order](#fixed-render-order) — the engine applies adjustments in a fixed, hardcoded order.
- [Dual pipeline, CPU canonical](#dual-pipeline-cpu-canonical) — CPU and GPU pipelines produce near-identical output; CPU is the deterministic source of truth.
- [Partial-parameter merge semantics](#partial-parameter-merge-semantics) — recursive through composite sections, last-write-wins at the leaf.
- [LUT in sRGB gamma](#lut-in-srgb-gamma) — LUTs apply in the perceptually-encoded space, matching how colorists author them.
- [Preset-first scope](#preset-first-scope) — no UI on the critical path; CLI and library are the surface.

## Always re-render from original

### What we chose

The engine holds an immutable original image and a mutable parameter state. Every call to `render()` starts from the original and replays every adjustment in pipeline order. There is no working buffer that mutates between renders and no operation history — a parameter change updates the state, and the next render re-derives the output from scratch.

### What we considered

The obvious alternative is incremental editing on a working buffer: apply each parameter change as a delta to the most recent rendered output. A second is an explicit operation log that records each user action and replays it on render. A third is a hybrid undo/redo stack — a working buffer plus a list of operations that can be wound back.

### Why we chose this

Photo editing operations are not mathematically commutative, so a system that promises order-independent edits has to either hide the order from the user or accept the order-sensitivity. The architecture design doc dated 2026-02-14 takes the first route: every render is a function of `(original, parameters)` evaluated in a fixed engine-defined order, so the user never has to reason about which of their adjustments came first. This sidesteps two failure modes at once — accumulated rounding error from sequential mutation of a working buffer, and the path dependence of "apply X, then Y, then undo X" that an operation log must either model exactly or paper over. Lightroom, darktable, and RawTherapee use the same pattern for the same reasons.

### What this costs

Every parameter change forces a full pipeline replay. There is no "render only the changed stage" optimization, because downstream stages see the original image rather than a partially-rendered buffer. Interactive preview is therefore more expensive than it would be in an incremental editor, and a future GUI would need a caching strategy layered on top of the engine. Dual-pipeline parallelization keeps render times fast enough for batch workflows, but the cost shows up the first time anyone proposes interactive editing as a primary use case.

## Declarative presets

### What we chose

A preset is a TOML document declaring parameter values, not an operation sequence. A preset says `exposure = +1.0`, never "apply exposure +1.0 after white balance." The engine reads the values, applies them in its own fixed order, and produces an output. The preset format does not encode pipeline order or name operations.

### What we considered

The dominant alternative across the photo-editing world is the operation log: a sidecar file that records each adjustment as the user made it, with the editor replaying the log to reconstruct the rendered image. Operation logs make it easy to support arbitrary undo/redo and editor-specific operations whose order matters. A second alternative is a partial-log hybrid where presets declare values but reserve room for operation-style entries (custom curves, masking strokes) applied in declaration order.

### Why we chose this

The architecture design doc dated 2026-02-14 frames this as a corollary of the always-re-render-from-original invariant. If the engine renders from `(original, parameters)` without a session buffer, there is no operation history to record, and a preset format that pretends there is one would leak engine internals into every TOML file. A flat parameter dictionary is also what makes presets portable across machines and durable across software versions: the file is a list of name-value pairs, not an executable sequence tied to engine version. The preset composability design doc dated 2026-03-07 builds the `extends` chain on top of this shape — layering presets is a recursive merge of parameter dictionaries, not a replay of two operation logs.

### What this costs

The engine cannot represent any edit whose meaning depends on its position in a sequence. This precludes order-sensitive features that other editors offer naturally: a per-preset override of pipeline order, or a custom "this, then that" sequence. It also constrains how local adjustments (masks, brushes) would have to work if added — they have to be portable parameter values, not hand-placed strokes that survive only in the editor that made them. The recipe model is what makes presets shareable, and any feature that breaks the recipe loses that property.

## sRGB-only working space

### What we chose

All internal processing uses the sRGB color space. The engine works in two related encodings — linear sRGB for physical operations, sRGB gamma for perceptual operations — but the gamut boundary is sRGB throughout. There is no working-space conversion, no ICC profile reading or embedding, and no support for wider gamut spaces.

### What we considered

The obvious alternative is a wider working space — Adobe RGB, ProPhoto RGB, or Display P3 — used internally regardless of input format, with conversion at the input and output boundaries. ProPhoto RGB is what Lightroom uses internally; Adobe RGB covers most professional print pipelines; Display P3 covers Apple's wide-gamut displays. A second alternative is partial color management: keep the math in sRGB but read and embed ICC profiles so input and output colors are correct on non-sRGB displays.

### Why we chose this

The architecture design doc dated 2026-02-14 scopes the MVP to sRGB only and frames it as a deliberate omission of the entire color-management subsystem rather than a temporary placeholder. Wide-gamut working spaces require gamut-aware math at every stage, profile embedding on output, cross-profile clipping policies for out-of-gamut values, and ICC parsing on input. Each is its own design space, and adding any one changes how every adjustment has to behave. Most consumer photography is sRGB end-to-end — JPEGs are sRGB, web display is sRGB, the screens most photographers grade on are sRGB — so the cost of the omission is concentrated on the professional print and log-video workflows AgX is not trying to serve.

### What this costs

Adobe RGB print pipelines, log video footage, and Display P3 displays are unreachable as inputs and outputs. A user with a wide-gamut camera will have their colors clipped to sRGB at the decode boundary. Adding wider gamuts later is not a drop-in change — every adjustment has to be revisited for gamut correctness, and the LUT pipeline has to grow a way to declare each LUT's own input space. The color spaces explanation page covers which math currently runs in linear sRGB versus sRGB gamma.

## Fixed render order

### What we chose

The engine applies adjustments in a fixed, hardcoded order. Exposure and white balance run first in linear space; dehaze and denoise follow, also in linear; the buffer is converted to sRGB gamma; the gamma-space adjustments (contrast, highlights, shadows, whites, blacks, tone curves, HSL, color grading, LUT) run in one per-pixel pass; detail, grain, and vignette run last. The order in which fields appear in a preset, in the `Parameters` struct, or in API calls has no effect on output.

### What we considered

The alternative is user-reorderable stages: a preset can declare "run dehaze after the LUT" or "run grain before detail." This would give finer creative control and let an advanced user override engine assumptions. A second alternative is partial reorder: most stages are fixed, but a handful (vignette, grain) can be marked "before" or "after" a neighboring stage.

### Why we chose this

Each stage is designed to run in the color space and pipeline position where its math is correct: exposure is a linear scaling and must precede the gamma conversion; dehaze and denoise both operate on physical light and stay in linear; contrast and tonal sliders reshape perceptual brightness and require the sRGB-gamma encoding; LUTs apply in the gamma space colorists author them in; grain and vignette are surface effects that should not be re-modified by upstream stages. Reordering any of these breaks an assumption a downstream stage depends on. The render pipeline explanation page walks through the worked examples. Hiding the order from users is also what makes the declarative-preset model work: a preset produces the same output regardless of how it was authored.

### What this costs

Stages are not user-reorderable. A photographer who wants grain before detail, or a LUT applied before tonal sliders for a log-grading workflow, has to either accept the engine's order or wait for the pluggable-pipeline design to land. The trade-off is predictability over flexibility: the same preset always produces the same output, at the cost of foreclosing creative orderings some users would value.

## Dual pipeline, CPU canonical

### What we chose

The engine has two pipeline executors that run the same stages in the same order: a CPU pipeline using Rust and rayon, and a GPU pipeline using wgpu and WGSL compute shaders. The CPU path is canonical — deterministic across platforms, used for golden-file testing, and the default when no GPU adapter is available. The GPU path is opt-in via `Engine::new_gpu_auto()` or the `--gpu` CLI flag. Cross-path consistency tests verify both produce near-identical output.

### What we considered

Three alternatives were on the table when GPU support was designed. CPU-only — keep the existing rayon pipeline and accept the throughput ceiling of consumer cores. GPU-only — port everything to wgpu and drop the CPU path, accepting the dependency on a working GPU adapter. GPU-as-default with CPU as a fallback only when no adapter is found, removing CPU's role as the testing source of truth.

### Why we chose this

The GPU acceleration design doc dated 2026-04-13 frames the decision around what AgX is for. AgX is primarily a batch CLI tool with no interactive preview, and batch throughput is already saturated by cross-image parallelism on consumer cores — GPU's latency advantage does not translate into a proportional batch speedup. Keeping CPU canonical avoids GPU floating-point variance across hardware vendors and driver versions, which would otherwise force per-vendor golden files. It eliminates the CI gap that would arise if golden output depended on a GPU adapter not every CI runner has. The GPU path remains worthwhile because per-pixel and convolution-heavy stages run dramatically faster on capable hardware, and a future interactive-preview feature would benefit directly. GPU becomes the default if and when interactive preview is added.

### What this costs

Every new adjustment is implemented twice — once in `adjust` for the CPU path, once in WGSL for the GPU path. The cross-path test surface grows with the feature set, and adding a CPU stage without a GPU dispatcher is a half-finished feature by the project's standards. Subtle floating-point differences between rayon-parallel CPU code and wgpu-parallel GPU code occasionally surface as off-by-one differences in 8-bit output, which the consistency tests catch but which require care to keep within tolerance.

## Partial-parameter merge semantics

### What we chose

A preset declares only the parameters it wants to change. Internally, the deserialization target is a `PartialParameters` type where every field is `Option<T>`: `None` means "this preset does not touch this field," `Some(0.0)` means "this preset explicitly sets this field to zero." Merging two `PartialParameters` is recursive through composite sections (HSL channel groups, color-grading regions) and last-write-wins at the leaf. A preset chain materializes into the engine's concrete `Parameters` only after every layer has been merged.

### What we considered

The straightforward alternative is full replacement: each preset declares the entire parameter set, and applying a preset overwrites engine state wholesale. This is what AgX did before the composability work, and the result was that a "warm tint" preset meant to change only temperature also reset exposure and contrast to zero — serde could not distinguish "missing field" from "explicit default." A second alternative is a flat top-level merge that replaces composite sections like HSL wholesale even when the overlay specifies one channel. A third is a noisy merge that fails when two presets both set the same field to different values.

### Why we chose this

The preset composability design doc dated 2026-03-07 builds the merge around how users actually layer presets: a base look preset, a color-grading overlay, a tint adjustment. The recursion through composite sections is what makes "specify only `red.hue`" work the way users expect — only that one channel changes, the rest of HSL is untouched. Last-write-wins at the leaf is the simplest semantic that keeps the merge associative and predictable. Cycle detection during the recursive `extends` load prevents chains from looping. The materialization step keeps the engine API unchanged: callers still receive a concrete `Parameters`, and the partial form exists only at the preset boundary.

### What this costs

Two parameter representations have to be maintained: `Parameters` (concrete, the engine's working type) and `PartialParameters` (every field optional, the preset deserialization target). Adding a new parameter means updating both, plus the materialize and merge implementations. Last-write-wins also means the merge is silent about conflicts — two presets setting the same field to different values produces a result without warning. The chosen semantic means a user who layers two presets with overlapping intent has no automatic feedback about which one won.

## LUT in sRGB gamma

### What we chose

LUTs are applied in sRGB gamma space, inside the per-pixel pass that runs after the linear stages and before detail, grain, and vignette. Pixel values are encoded to sRGB gamma before the LUT lookup, the LUT does its trilinear interpolation on those values, and the result continues through the per-pixel pass. The pipeline does not auto-convert the LUT input to a different space.

### What we considered

The alternative is to apply LUTs in linear sRGB. A pipeline that already runs dehaze, denoise, and exposure in linear could plausibly run LUTs there too. A second alternative is a per-LUT declared input space — each `.cube` file would declare whether it expects linear, sRGB gamma, or log input, and the pipeline would auto-insert conversions. A third is a global setting that picks one space for all LUTs.

### Why we chose this

The LUT support design doc dated 2026-02-16 frames LUTs as opaque numeric mappings whose correct input space is determined by how they were authored, not by what the engine prefers. The vast majority of creative `.cube` LUTs — film emulations, color grades, Instagram-style looks — are authored by colorists working on screens that display sRGB. The lattice values in those LUTs correspond to sRGB pixel values, not linear ones. Applying such a LUT to linear values produces incorrect colors. AgX picks the space that works for the largest body of existing LUTs, and applies the LUT after parametric tone adjustments — matching the standard Lightroom and Resolve workflow where the LUT is a creative grade on top of corrected exposure and contrast.

### What this costs

Log-input LUTs (S-Log3, LogC, ACEScct) used in video grading workflows are not directly supported. A colorist who wants to apply a log-to-Rec709 conversion LUT cannot use AgX's LUT pipeline as is — the input the LUT expects is not the input it receives. The fix is the per-LUT declared input space, which is on the long-term map but blocked on the wider-gamut and log-color work the sRGB-only invariant defers.

## Preset-first scope

### What we chose

AgX is a preset-first photo editing tool. The library is the source of truth — it owns the engine, the parameter set, the preset parser, and dispatch into the CPU and GPU pipelines. The CLI is a thin wrapper that turns command-line arguments into parameter overrides and preset paths into engine inputs. There is no GUI on the critical path. Every consumer — CLI today, a web service or batch worker tomorrow, a UI someday — uses the same library API.

### What we considered

The alternatives are all variations on building a UI as a primary surface: a full editing GUI as the centerpiece with the library and CLI as exports, a web editor with the engine compiled to WebAssembly, or a plug-in surface inside an existing editor where AgX provides the preset format and the host provides the UI. Each would change what AgX is fundamentally about — the centre of gravity moves from the recipe to the editing surface.

### Why we chose this

The project's framing — visible in the README and reinforced in every design doc — is that AgX is a preset-first batch editing tool, not a Lightroom replacement. The recipe model is what makes the project coherent: every feature has to fit a portable, declarative, image-independent description, and a UI on the critical path would constantly tempt features that fit a UI better than they fit a recipe. Keeping the surface CLI-and-library-first forces every feature to justify itself as a parameter on the recipe. A future UI is not ruled out, but if added it would be one more way to author a preset, not a session manager that drives the engine through hidden state.

### What this costs

AgX is not what most photo editors look like, and the reach of the project is constrained accordingly. A user who learns photo editing through GUI-driven tutorials does not have an obvious entry point — the docs assume comfort with a command line, a TOML file, and a text editor. Spot retouching, local adjustments, and brush-driven workflows are out of scope until a portable encoding for them is designed. The project trades the broad audience of a GUI editor for the durability and shareability of a recipe format.

## See also

- [Architecture](architecture.md) — how these invariants are enforced by the module graph.
- [Preset-first philosophy](philosophy.md) — the user-facing framing this list serves.
- [Render pipeline](render-pipeline.md) — the order constraint discussed in detail.
- [Color spaces](color-spaces.md) — the working-space constraint discussed in detail.
