# UI

**Category:** UI
**Status:** Backlog

## Problem / Opportunity

oxiraw is CLI-only. A graphical interface — native desktop or web-based — would make it accessible to photographers who aren't comfortable with command-line tools. Supporting features like real-time histograms, before/after comparison, and undo/redo are essential for an interactive editing experience.

## Key Considerations

- **Native desktop UI**: Possible frameworks include egui (immediate mode, pure Rust), iced (Elm-inspired, pure Rust), or Tauri (web frontend + Rust backend). Trade-offs between rendering performance, widget richness, and development speed
- **Web UI**: Via WASM compilation of the core library. Enables browser-based editing with no installation. See [Platform and Distribution](platform-and-distribution.md) for WASM considerations
- **Real-time histogram**: Live luminance and per-channel RGB histogram display during editing. Must update interactively as parameters change — depends on fast preview rendering
- **Before/after comparison**: Side-by-side or split-view comparison between original and edited image. Requires rendering both versions simultaneously or caching the original display version
- **Undo/redo**: Parameter state history stack. Since the engine always re-renders from original, undo is just "restore previous parameter set" — no image state to manage. Could use a simple `Vec<Parameters>` with a cursor
- Interactive performance requires fast re-rendering — thumbnail/preview pipeline and stage caching help here

## Related

- [Platform and Distribution](platform-and-distribution.md) — WASM, REST API, and GPU enable UI backends
- [Pluggable Pipeline](pluggable-pipeline.md) — stage caching for interactive editing performance
- [Local Adjustments](local-adjustments.md) — brushes and masks need UI for painting/drawing
