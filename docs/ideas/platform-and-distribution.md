# Platform and Distribution

**Category:** Performance
**Status:** Backlog

## Problem / Opportunity

oxiraw is currently a Rust library + CLI. Expanding to a REST API, GPU acceleration, WASM compilation, and batch processing would make it usable in web apps, mobile backends, high-performance workflows, and automated pipelines. A preset marketplace would build community around sharing looks.

## Key Considerations

- **REST API**: Expose oxiraw as an HTTP service. Accept image + preset (or inline params), return processed image. Key considerations: multipart upload, JSON body for inline adjustments, streaming response for large images, authentication, rate limiting, job queuing for heavy processing. Could serve as backend for web UI, mobile app, or marketplace
- **Preset marketplace / registry**: Platform for sharing and discovering community presets. Needs curation, versioning, preview thumbnails, and a distribution format
- **GPU acceleration**: Use wgpu or compute shaders for real-time rendering. Most beneficial for interactive editing (immediate feedback on parameter changes). Per-pixel adjustments map naturally to GPU compute; neighborhood operations need more thought
- **WASM compilation**: Run the core engine in the browser for web-based editing. The `image` and `palette` crates support WASM. LibRaw FFI does not — raw decoding would need a server-side component or a WASM-compatible raw decoder
- **Batch processing**: Process entire folders with rayon/tokio parallelism. Apply the same preset to hundreds of images. Progress reporting, error handling per-file, and output naming conventions needed
- **Thumbnail/preview pipeline**: Fast low-res preview during editing, full-res on export. Downscale the original, render adjustments on the thumbnail, re-render at full resolution on export

## Related

- [Pluggable Pipeline](pluggable-pipeline.md) — stage caching enables interactive preview performance
- [UI](ui.md) — REST API and WASM enable web and desktop UIs
