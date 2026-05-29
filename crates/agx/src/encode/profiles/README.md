# Color profiles

This directory holds the ICC profile blobs the encoder embeds in output files.

## Current contents

- `srgb_v4.icc` — sRGB v4, lcms2-generated. The encoder unconditionally embeds these bytes in every JPEG / PNG / TIFF output. See `crates/agx/src/encode/icc.rs` for the consuming code.

## Why we generate the blob ourselves

Famous third-party sRGB v4 profiles (Elle Stone, ICC Consortium reference) carry licensing terms incompatible with AgX's MIT/Apache library posture — see [`docs/contributing/asset-licensing.md`](../../../../../docs/contributing/asset-licensing.md) for the full rationale and the rejected-options table.

The profile here is generated from sRGB primaries + parametric transfer curve via the `lcms2` Rust crate; the generator lives at `crates/agx-profile-gen/`. lcms2 is MIT-licensed, and the generated output inherits that license cleanly.

To regenerate (rarely needed — the bytes are deterministic):

```bash
cargo run -p agx-profile-gen
```

The committed `srgb_v4.icc` is the canonical artifact; regenerate only if the generator changes or a future ICC spec update warrants it.
