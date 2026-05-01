# Render IO Buffer Reduction Design

## Problem

The decode and encode paths each allocate one f32 image buffer that is functionally redundant. At 26MP, each redundant buffer is ~300MB of resident memory.

**Decode** (`crates/agx/src/decode/mod.rs`, `decode_standard`):

1. `img.into_rgb32f()` produces an sRGB f32 buffer (~300MB).
2. `Rgb32FImage::from_fn(...)` allocates a brand-new f32 buffer (~300MB) and reads from the sRGB buffer to fill it with linear values.

Both buffers are alive simultaneously until the function returns. The sRGB buffer is then dropped — but only after the new linear buffer is fully populated.

**Encode** (`crates/agx/src/encode/mod.rs`, `encode_to_file_with_options`):

1. The caller passes in the linear buffer (~300MB).
2. `linear_to_srgb_dynamic(linear)` allocates an sRGB f32 buffer (~300MB) via `Rgb32FImage::from_fn` and wraps it as `DynamicImage::ImageRgb32F`.
3. `dynamic.to_rgb8()` allocates an `RgbImage` u8 buffer (~75MB) and quantizes the f32 values into it.

The intermediate f32 sRGB buffer (step 2) exists only to feed `to_rgb8()`. Going straight from linear f32 to u8 in a single pass collapses the two conversions and removes the f32 intermediate.

## Approach

Two surgical changes, one per module. No algorithmic change. No reordering of math operations beyond fusing two element-wise conversions into one.

### Decode: convert in-place

Replace the `from_fn` allocation with an in-place mutation of the sRGB buffer:

```rust
let mut buf = img.into_rgb32f();
for px in buf.pixels_mut() {
    let lin: LinSrgb<f32> = Srgb::new(px.0[0], px.0[1], px.0[2]).into_linear();
    px.0 = [lin.red, lin.green, lin.blue];
}
Ok(buf)
```

The same `Srgb::into_linear()` call runs against the same input values in the same order. Output is byte-identical to the current implementation.

### Encode: linear → u8 sRGB single pass

Add a new helper that produces an `RgbImage` directly from the linear input:

```rust
fn linear_to_srgb_rgb8(linear: &Rgb32FImage) -> RgbImage {
    let (w, h) = linear.dimensions();
    RgbImage::from_fn(w, h, |x, y| {
        let p = linear.get_pixel(x, y);
        let s: Srgb<f32> = LinSrgb::new(p.0[0], p.0[1], p.0[2]).into_encoding();
        Rgb([
            quantize_u8(s.red),
            quantize_u8(s.green),
            quantize_u8(s.blue),
        ])
    })
}
```

`quantize_u8` must reproduce exactly the rounding/clamping that `image::DynamicImage::to_rgb8()` performs on `ImageRgb32F` input. Verification is mechanical: render a representative sample of f32 values through both the current path (linear → `linear_to_srgb_dynamic` → `to_rgb8`) and the new helper, and assert byte-for-byte equality on the resulting `RgbImage`. This check runs as a unit test in `encode/mod.rs`.

The existing `linear_to_srgb_dynamic` is removed once the new helper replaces both call sites.

## Memory expectation

At 26MP:

| Path | Current peak (f32 + u8) | After change | Saved |
|------|-------------------------|--------------|-------|
| Decode `decode_standard` | ~600MB (two f32 buffers) | ~300MB (one f32 buffer) | ~300MB |
| Encode `encode_to_file_with_options` | ~675MB (linear f32 + sRGB f32 + u8) | ~375MB (linear f32 + u8) | ~300MB |

Numbers exclude format-specific encoder working memory and the encoded byte buffer, which are unaffected.

The exact figures depend on the `image` crate's allocation strategy — measurement below is ground truth.

## Verification

1. `./scripts/verify.sh` — fmt, clippy, unit, architecture, doc-links.
2. **Unit test for encode quantization parity.** New test in `encode/mod.rs` builds a small `Rgb32FImage` covering edge values (0.0, near 0.5, 1.0, slightly out of range, NaN if applicable) and asserts `linear_to_srgb_rgb8(&img)` produces the same bytes as the current `dynamic.to_rgb8()` path. This test must pass before the old path is removed.
3. `./scripts/e2e.sh` — full golden matrix must remain byte-identical. Decode and encode are universal stages, so any byte drift would surface across the matrix.
4. **Memory measurement** using `/usr/bin/time -l` on macOS (or `/usr/bin/time -v` on Linux) on a 26MP fixture:

   ```
   /usr/bin/time -l ./target/release/agx apply \
       --preset crates/agx-e2e/fixtures/looks/portra_400.toml \
       crates/agx-e2e/fixtures/raw/sunset_river.raf \
       /tmp/out.jpg
   ```

   Capture `maximum resident set size` before the change (on `main`) and after. Run separately against a JPEG input (`temple_blossoms.jpg`) to exercise the standard decode path. Record both numbers in this design doc once measured. Linux reports kilobytes; macOS reports bytes.

## Files

| File | Change |
|------|--------|
| `crates/agx/src/decode/mod.rs` | Convert sRGB→linear in-place inside `decode_standard` |
| `crates/agx/src/encode/mod.rs` | Add `linear_to_srgb_rgb8`, route encoders through it, remove `linear_to_srgb_dynamic` |
| `docs/backlog/performance.md` | Check off "Decode buffer reduction" and "Encode buffer reduction" |

## Scope

- **In scope.** Decode in-place conversion, encode single-pass conversion, parity unit test, memory measurement, backlog checkoff.
- **Out of scope.** Raw decode path (`decode/raw.rs`) — it goes through LibRaw with its own allocation flow, separate analysis. Algorithmic changes. Format-specific encoder optimizations. Dehaze guided filter buffer drops (separate design and PR landing in parallel). Persistent memory profiling harness.

## Risks

- **Encode quantization parity.** The biggest risk is that `linear_to_srgb_rgb8` rounds differently than `DynamicImage::to_rgb8()` on edge values, breaking goldens. The unit test above runs first, against representative inputs, and the new path is not wired into encoders until parity is proven. If parity cannot be reproduced via the public `palette` and `image` APIs, the design retreats: keep the f32 sRGB intermediate but consume it lazily via a streaming converter. This adds complexity and would prompt a separate brainstorming session.
- **Decode in-place borrow checker.** `pixels_mut()` returns `&mut Rgb<f32>`, and the conversion reads three channels and writes three channels of the same pixel — no aliasing. This compiles cleanly.
- **Behavior under raw decode.** This change does not touch `decode/raw.rs`. RAW images take a different path that already produces linear f32 directly (no intermediate sRGB buffer to drop). Verification against a RAW fixture is included in the e2e matrix, so any unexpected interaction would surface.

## Related

- Backlog: [docs/backlog/performance.md](../backlog/performance.md) — "Decode buffer reduction", "Encode buffer reduction"
- Sibling effort: dehaze guided filter buffer drops — separate design and PR landing in parallel.
