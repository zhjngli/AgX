# Platform and Distribution

Expand AgX from a library + CLI into a platform for its preset language: serve looks as a service, and build a place to share and discover them. This is the headline expansion of the project's vision — the network-effects layer around the portable preset language.

## Sub-tasks

- [ ] **REST API** — expose AgX as an HTTP service: accept image + preset (or inline params), return the processed image. The hosted renderer behind any web UI, mobile backend, or marketplace preview. Considerations: multipart upload, streaming response, job queuing for heavy renders.
- [ ] **Preset marketplace / registry** — a platform for sharing and discovering community presets, the network-effects play around the language. Needs curation, preview thumbnails, a distribution format, and — because authors must trust presets not to break — preset schema versioning (see [Preset Tooling](preset-tooling.md)).

## Considerations

- The REST API re-exposes engine quality as a customer-facing concern: it is the hosted renderer, so it is where own-engine improvements (performance, parity) become visible again.
- A marketplace distributes the portable preset language; its reach is widened by the export/portability work in [Ecosystem Interop](ecosystem-interop.md).
- GPU acceleration is tracked under [Performance](performance.md) (the compute-shader path shipped as P7; GPU-as-default is P8) — not duplicated here.

## Parked

- **WASM compilation** — run the core engine in the browser for install-free editing. The `image` and `palette` crates support WASM, but LibRaw FFI does not, so raw decoding would need a server-side component. UI-adjacent; revisit if a browser editing surface is pursued.
- **Thumbnail / preview pipeline** — fast low-res preview during editing, full-res on export. A prerequisite for interactive editing regardless of platform; revisit alongside UI work.

## Related

- [Preset Tooling](preset-tooling.md) — schema versioning is a marketplace prerequisite.
- [Ecosystem Interop](ecosystem-interop.md) — portability widens where marketplace presets can be used.
- [Pluggable Pipeline](pluggable-pipeline.md) — stage caching enables interactive/preview performance behind the API.
- [Performance](performance.md) — GPU and the preview pipeline are performance-adjacent.
- [UI](ui.md) — REST API and WASM enable web and desktop UIs.
