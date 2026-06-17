# UI

Desktop and web graphical interface for AgX.

## Sub-tasks

- [ ] **Evaluate UI frameworks** — egui (immediate mode, pure Rust), iced (Elm-inspired), or Tauri (web frontend + Rust backend)
- [ ] **Real-time histogram** — live luminance and per-channel RGB histogram that updates as parameters change
- [ ] **Before/after comparison** — side-by-side or split-view comparison between original and edited
- [ ] **Undo/redo** — parameter state history stack. Since the engine always re-renders from original, undo is just "restore previous parameter set" — `Vec<Parameters>` with a cursor
- [ ] **Web UI via WASM** — browser-based editing with no installation

## Considerations

- Interactive performance requires fast re-rendering — the thumbnail/preview pipeline and stage caching are prerequisites.
- Undo/redo is architecturally simple because the engine always re-renders from the original image (no mutable image state to manage).
- Framework choice depends on whether we prioritize native performance (egui/iced) or web reach (Tauri/WASM).

## Related

- [Platform and Distribution](platform-and-distribution.md) — WASM and the REST API enable web and desktop UI backends
- [Pluggable Pipeline](pluggable-pipeline.md) — stage caching for interactive editing performance
- [Local Adjustments](local-adjustments.md) — brushes and masks need UI for painting/drawing
