# Backlog

Future work for AgX — epics, sub-tasks, and bugs. Each file is an "epic" with sub-tasks that can be worked on independently. When all sub-tasks in an epic are complete, remove the file from this directory.

Some epics have detailed sub-task docs (e.g., `grain-size-algorithm.md` is a sub-task of `processing-parity.md`). These live as their own files when the investigation is deep enough to warrant it, but are tracked under their parent epic in the roadmap.

## Adding New Items

Items come in different sizes. Use the right level for what you're capturing:

### Epic (top-level idea file)

A broad feature area with multiple sub-tasks. Create a new `<name>.md` file and add it to the roadmap and category tables.

**Format:** overview, sub-tasks checklist, considerations, related links. See any existing file for the template.

### Sub-task (checklist item in an epic)

A concrete work item within an existing epic. Add a `- [ ]` checkbox line to the parent epic's Sub-tasks section.

If a sub-task has enough depth to warrant detailed investigation (proposed fixes, code analysis, test plans), give it its own `.md` file and link to it from the parent epic. Add a "Parent epic" blockquote at the top linking back to the parent. See [grain-size-algorithm.md](grain-size-algorithm.md) for an example.

### Bug (sub-task of an epic)

A known defect in an existing feature. Add it under a **Bug fixes** heading in the relevant epic's sub-tasks. If the bug needs investigation notes, give it its own file linked from the epic (same as a detailed sub-task).

## Roadmap

Prioritized by alignment with the project philosophy: preset-first batch editing via CLI and API. Features that make presets more expressive or batch workflows faster come first. Developer velocity improvements are also high priority.

### Near-term

Practical improvements to existing functionality — small scope, clear value.

| Priority | Idea | Why now |
|----------|------|---------|
| 1 | [Processing Parity](processing-parity.md) | Includes grain size bug fix; per-feature verification improves output quality |
| 2 | [Multi-Preset CLI](multi-preset-cli.md) | Cuts e2e test time in half; improves batch workflows |
| 3 | [Parallel CI E2E](parallel-ci-e2e.md) | Reduces CI wall-clock time; improves developer velocity |
| 4 | [Preset Tooling](preset-tooling.md) | Validation command catches preset errors before processing |
| 5 | [Performance](performance.md) | Profiled — parallelize per-pixel loop and Gaussian blur for 50-60% render speedup |

### Mid-term

Larger efforts that expand AgX's capabilities or improve code quality.

| Priority | Idea | Why next |
|----------|------|----------|
| 6 | [Documentation Initiative](documentation-initiative.md) | Coherent docs system (mdbook site, rustdoc, auto-gen CLI/preset ref, algorithm explanations) |
| 7 | [Pluggable Pipeline](pluggable-pipeline.md) | Architectural improvement — 4 neighborhood ops justify the abstraction now |
| 8 | [HEIC/HEIF Support](heic-support.md) | iPhone is the most popular camera; unblocks a large user base |
| 9 | [Ecosystem Interop](ecosystem-interop.md) | Lightroom XMP import alone would unlock existing preset libraries |
| 10 | [Color Management](color-management.md) | Wide gamut and ICC profiles are the gap between consumer and professional |

### Long-term

Major features that require significant design work or change the project's scope.

| Priority | Idea | Notes |
|----------|------|-------|
| 11 | [Geometric Corrections](geometric-corrections.md) | Lens corrections, perspective, crop/rotation |
| 12 | [Local Adjustments](local-adjustments.md) | Major architectural change to the render model |
| 13 | [Platform and Distribution](platform-and-distribution.md) | REST API, GPU, WASM, preset marketplace |
| 14 | [UI](ui.md) | Desktop and web interfaces |
| 15 | [Advanced Research](advanced-research.md) | AI editing, HDR merge, panorama, focus stacking |

## By Category

### Editing

| File | Summary |
|------|---------|
| [local-adjustments.md](local-adjustments.md) | Brushes, gradients, and radial filters for per-region edits |
| [geometric-corrections.md](geometric-corrections.md) | Lens corrections, perspective, crop and rotation |

### Pipeline and Infrastructure

| File | Summary |
|------|---------|
| [performance.md](performance.md) | Data-driven render optimization roadmap (profiled, prioritized P1-P6) |
| [multi-preset-cli.md](multi-preset-cli.md) | Decode once, apply N presets per CLI invocation |
| [pluggable-pipeline.md](pluggable-pipeline.md) | Stage-based render pipeline with caching and color-space awareness |
| [parallel-ci-e2e.md](parallel-ci-e2e.md) | Parallelize e2e tests in GitHub Actions via matrix strategy |
| [preset-tooling.md](preset-tooling.md) | Schema versioning, validation, and authoring shortcuts |

### Quality and Correctness

| File | Summary |
|------|---------|
| [processing-parity.md](processing-parity.md) | Per-feature algorithm verification, grain size bug fix, raw processing improvements |

### Documentation

| File | Summary |
|------|---------|
| [documentation-initiative.md](documentation-initiative.md) | Umbrella epic: mdbook site, rustdoc retrofit, auto-gen CLI/preset ref, algorithm explanations, tutorials, how-tos |

### Color and Formats

| File | Summary |
|------|---------|
| [color-management.md](color-management.md) | Wide gamut, ICC profiles, per-camera color matrices |
| [heic-support.md](heic-support.md) | HEIC/HEIF format decoding for Apple devices |
| [ecosystem-interop.md](ecosystem-interop.md) | XMP/costyle/pp3 import/export and sidecar files |

### Platform and UI

| File | Summary |
|------|---------|
| [platform-and-distribution.md](platform-and-distribution.md) | REST API, GPU, WASM, preset marketplace |
| [ui.md](ui.md) | Desktop and web UI, histogram, before/after, undo/redo |
| [advanced-research.md](advanced-research.md) | AI editing, HDR merge, panorama, focus stacking |
