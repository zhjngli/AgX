# Platform and Distribution

Expanding AgX beyond a library + CLI to serve web apps, mobile backends, and interactive editing workflows.

## Sub-tasks

- [ ] **REST API** — expose AgX as an HTTP service: accept image + preset (or inline params), return processed image. Considerations: multipart upload, streaming response, job queuing for heavy processing
- [ ] **GPU acceleration (wgpu)** — use compute shaders for real-time rendering. Per-pixel adjustments map naturally to GPU; neighborhood operations need more thought. Most beneficial for interactive editing
- [ ] **WASM compilation** — run the core engine in the browser. The `image` and `palette` crates support WASM, but LibRaw FFI does not — raw decoding would need a server-side component
- [ ] **Thumbnail/preview pipeline** — fast low-res preview during editing, full-res on export. Downscale original, render on thumbnail, re-render at full res on export
- [ ] **Preset marketplace/registry** — platform for sharing and discovering community presets. Needs curation, versioning, preview thumbnails, and a distribution format

## Considerations

- REST API could serve as the backend for both a web UI and a mobile app.
- GPU acceleration and WASM are independent efforts — GPU targets native performance, WASM targets browser reach.
- The thumbnail/preview pipeline is a prerequisite for interactive editing regardless of platform.

## Related

- [Pluggable Pipeline](pluggable-pipeline.md) — stage caching enables interactive preview performance
- [UI](ui.md) — REST API and WASM enable web and desktop UIs
- [Performance](performance.md) — GPU and preview pipeline are performance-adjacent
