# Preset-first photo editing

AgX is a preset-first photo editing tool. This page explains what "preset-first" means, what it implies for the shape of the project, and what AgX deliberately is and is not.

## Presets as recipes

A preset in AgX is a recipe for an edit: a complete, declarative statement of which knobs to turn and how far. It is not a log of operations the user performed. The TOML document says `exposure = +0.5` and `contrast = 15`; it does not say "the user dragged the exposure slider, then opened the curves panel, then undid the curve change." The engine reads the recipe, applies every value in a fixed pipeline order, and produces an image. The same recipe applied to the same input always produces the same output.

This shape makes a preset durable. The recipe is just a list of parameter values, so it survives software upgrades the same way any plain-text data format does — as long as the parameter names mean what they used to mean, the file still describes the same edit. The recipe is also legible without the editor that produced it: a reader can open the TOML, see what the preset does, and reason about it without launching the application. And because the recipe captures *intent* — "I want this much warmth, this much contrast" — rather than the GUI clicks that produced it, sharing a preset does not leak whatever session state the author happened to have open.

The contrast with operation-log editors is sharp. Most photo editors store edits as a sequence of operations: applied filter X with parameter set Y, then filter Z, then undid filter X. The sequence is meaningful — reordering it changes the output — and the operation names and parameter shapes are tied to the editor's internal data model. Sidecar files from operation-log editors tend to be opaque, version-locked, and brittle across upgrades. AgX trades the flexibility of arbitrary operation sequences for the simplicity of a flat parameter dictionary, and gets shareability and durability in exchange. The mechanics of how recipes layer on top of one another are covered on the [preset model](preset-model.md) page.

## Batch-oriented, not pixel-level

AgX is good at one specific thing: applying a consistent look to many images. The library exposes a render engine that consumes a parameter set and produces a buffer; the CLI wraps that engine in preset-oriented subcommands — `apply`, `multi-apply`, and `batch-apply` — alongside `edit` and `batch-edit` for inline flag-based workflows. All are designed around throughput rather than interactivity. A user with a folder of 500 images and a single preset can render the lot in one command and walk away. That use case is the centre of gravity for every design decision.

What AgX deliberately is not is a pixel-level retouching tool. There is no spot-heal brush and no clone-stamp surface. There is no undo stack — the engine renders from the original image on every call, and the parameter set is the only state. There are no local adjustments yet: every adjustment is global, applied uniformly to every pixel in the image. None of these are oversights. They are coherent omissions that follow from the recipe model.

Each missing feature would fight the recipe. Spot retouching's output depends on hand-placed brushes on a specific image; encoding that in a portable, image-independent recipe is a different design space from "this much exposure, this much contrast." An undo stack is a per-session GUI construct — it presupposes a user sitting in front of an editor, which presupposes a UI, which presupposes a session, none of which the recipe model has. Local adjustments are tractable but not trivial: they require a portable way to encode masks, and any encoding scheme has to survive being applied to images of different sizes and orientations than the one it was authored on. Local adjustments are on the long-term map; they are not free.

The roadmap follows from this. Features that strengthen the recipe model — more expressive parameters, better composability, more efficient batch dispatch — come first. Features that would make AgX a Lightroom replacement — a full editing GUI, an asset catalog, layer-style local edits before the portability problem is solved — come never, or come only in a form that fits the recipe. The point is not to apologize for what is missing; the point is that every absent feature is absent on purpose.

## Shareable by design

Presets are plain text. They live in TOML files; they sit in a Git repository as cleanly as any other source file; a `git diff` between two preset versions reads as a list of parameter changes. Two collaborators iterating on a look can review each other's changes the way they review code. Nothing about the format depends on opaque binary blobs, embedded thumbnails, or editor-specific identifiers.

This makes presets forkable. A preset author can publish a "neutral starting point" preset that fixes white balance and gives the input a sensible tonal baseline; another author can fork it, add a tone curve and a LUT, and publish the result without the original author's involvement. The `extends` mechanism makes the fork structural rather than copy-paste — the child preset declares what it overrides on top of its parent — so a fix to the parent propagates to every preset that extends it.

The longer-term bet is that a preset format which is shareable enough accumulates an ecosystem. If presets travel as easily as code does — readable, diffable, version-controllable, composable — then preset libraries, preset marketplaces, and preset remixing become possible in a way they are not for editor-locked sidecar formats. AgX's design wagers that a portable recipe format is a precondition for that ecosystem, and that the ecosystem itself is worth building toward.

## CLI and API first

There is no GUI on the critical path. The library is the source of truth: it owns the engine, the parameter set, the preset parser, and the dispatch into the CPU and GPU pipelines. The CLI is a thin wrapper that turns command-line arguments into parameter overrides and preset paths into engine inputs. Any other consumer — a batch worker, a web service, a third-party application — would consume the same library API. The recipe model is what makes this work: every surface that produces a preset can drive the engine the same way.

If a UI is added, nothing about the architecture changes. A UI would consume the same library API, expose sliders that map to parameter values, and write the result to a TOML file. The UI's job in that future would be to help a user produce a preset, not to manage a session — there is no session to manage, because the engine is stateless across renders. The recipe stays canonical; the UI is one more way to author one. The [architecture](architecture.md) page covers how this constraint shapes the module dependency graph: `agx-cli` depends only on the public library API, the same surface a UI would have to use.

## See also

- [Preset model](preset-model.md) — the patch-on-baseline mental model that follows from the recipe view.
- [Architecture](architecture.md) — how the codebase is shaped to serve the philosophy.
- [Design decisions](design-decisions.md) — the cross-cutting invariants this philosophy implies.
