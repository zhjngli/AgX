# HEIC/HEIF Support Design

## Problem

iPhone has been the most popular consumer camera for years, and since iOS 11 the default capture format is HEIC (HEIF container, HEVC codec). AgX currently rejects `.heic` / `.heif` files — they go down the standard-format path, the `image` crate refuses them, and the user gets a decode error. The only workaround is to convert to JPEG first (lossy and tedious).

Adding HEIC decode unblocks the single largest user-reach gap in the project. Backlog entry: [`docs/backlog/heic-support.md`](../backlog/heic-support.md).

## Scope

In scope:

- Decoding HEIC and HEIF files into the engine's linear sRGB f32 buffer.
- EXIF metadata extraction, fed into the existing metadata module so capture info survives to the output JPEG/PNG/TIFF.
- 8-bit and 10-bit sources from iPhones (and any other camera writing HEIC).
- Source-color-space handling for the three common NCLX matrices on iPhone output: sRGB / BT.709, Display P3, BT.2020.

Out of scope (logged in [`docs/backlog/heic-support.md`](../backlog/heic-support.md) for future work):

- HEIF encoding (HEIC or AVIF output).
- Auxiliary images inside the HEIF container (depth maps, burst frames, alternate exposures). Initial decode reads only the primary image.
- XMP handling. Selective namespace preservation needs its own design because naive passthrough produces misleading output (stale Live Photo links, false content-credentials signatures, misleading edit-history XMP).
- ICC profile parsing. Wide-gamut working-space changes are tracked under [`docs/backlog/color-management.md`](../backlog/color-management.md); this design gamut-maps to linear sRGB at decode and notes the limitation in user-facing docs.

## Approach

HEIC support lands as a third decode path in the `decode` module, alongside `decode_standard` (image crate, JPEG/PNG/TIFF) and `decode_raw` (LibRaw FFI). All three paths return the same `Rgb32FImage` in linear sRGB — the engine and every downstream stage are unchanged.

Routing happens in `decode/mod.rs`. A new `HEIC_EXTENSIONS = &["heic", "heif"]` constant and `is_heic_extension` helper mirror the raw equivalents. `decode()` checks extension and routes to the HEIC path when the `heic` feature is enabled, returning a friendly error when it is not.

The HEIC path itself is a single new file, `crates/agx/src/decode/heic.rs`, that wraps libheif's C API. libheif is the de-facto reference HEIF implementation (used by ImageMagick, GIMP, darktable, RawTherapee). Its public C API is rich enough that no helper C file is needed; this differs from the LibRaw path, which compiles `libraw_meta.c` because LibRaw doesn't expose certain fields directly.

## Components

```
crates/agx/src/decode/
  mod.rs              extended: HEIC_EXTENSIONS, is_heic_extension(), routing
  heic.rs             new: libheif FFI, RAII wrappers, decode_heic, extract_heic_metadata
  raw.rs              unchanged
  orientation.rs      unchanged (libheif applies HEIC orientation internally)
  libraw_meta.c       unchanged
  README.md           extended: HEIC entry, "Building from source" note
```

Inside `heic.rs`:

- `extern "C"` declarations for the libheif functions we call: context lifecycle (`heif_context_alloc`, `heif_context_read_from_file`, `heif_context_free`), image handle (`heif_context_get_primary_image_handle`, `heif_image_handle_release`), decoding (`heif_decode_image`, `heif_image_release`), pixel access (`heif_image_get_plane_readonly`), color profile inspection (`heif_image_handle_get_color_profile_type`, `heif_image_handle_get_nclx_color_profile`), bit-depth inspection (`heif_image_handle_get_luma_bits_per_pixel`), metadata (`heif_image_handle_get_list_of_metadata_block_IDs`, `heif_image_handle_get_metadata_size`, `heif_image_handle_get_metadata`).
- RAII wrappers — `HeifContext`, `HeifImageHandle`, `HeifImage` — each holding the relevant opaque pointer and calling the matching release function in `Drop`. Same pattern as `LibRawProcessor` and `ProcessedImage`.
- `pub fn decode_heic(path: &Path) -> Result<Rgb32FImage>` — opens the context, gets the primary image handle, inspects bit depth and color profile, requests RGB decode in the appropriate chroma layout, reads the pixel plane, converts to linear sRGB f32, returns the buffer.
- `pub fn extract_heic_metadata(path: &Path) -> Option<Vec<u8>>` — opens the context, enumerates EXIF metadata blocks via `get_list_of_metadata_block_IDs` filtered by type, returns the bytes with libheif's 4-byte TIFF-offset prefix stripped.

## Data flow

`decode_heic(path)`:

1. `HeifContext::new()` → `heif_context_alloc()`. Then `read_from_file(path)`.
2. Get the primary image handle. Errors on this step typically mean the file is not a valid HEIF.
3. Inspect source:
   - `heif_image_handle_get_luma_bits_per_pixel` → 8 or 10.
   - `heif_image_handle_get_color_profile_type` → `nclx` (matrix coefficients), `prof` (ICC profile), or `unknown`.
   - If NCLX: read coefficients, classify as sRGB/BT.709, Display P3, or BT.2020.
   - If ICC or unknown: log a stderr warning, treat as sRGB.
4. Request decode in interleaved RGB:
   - 8-bit source: `chroma = heif_chroma_interleaved_RGB`.
   - 10-bit source: `chroma = heif_chroma_interleaved_RRGGBB_LE` (16-bit container holding 10-bit values, little-endian).
   - libheif applies the file's irot/imir orientation transformations during decode by default. We do not pass `ignore_transformations`.
5. Read pixel plane via `heif_image_get_plane_readonly(img, channel=interleaved)` returning a `*const u8` and the row stride.
6. Convert to linear sRGB f32, parallelizing row work with rayon (matches the existing per-pixel loop pattern in `decode_standard`):
   - Normalize integer pixel values to `[0, 1]` floats (`/255.0` for 8-bit, `/1023.0` for 10-bit).
   - If the source is Display P3 or BT.2020, apply the 3x3 matrix to convert into sRGB primaries (still gamma-encoded). Matrices come from the `palette` crate (already a workspace dependency).
   - Apply sRGB gamma → linear via `palette::Srgb::into_linear`.
7. Return the `Rgb32FImage`.

`extract_heic_metadata(path)`:

1. Same context and handle setup. Errors return `None` (matches `extract_raw_metadata` behavior).
2. `heif_image_handle_get_list_of_metadata_block_IDs(handle, type="Exif", ...)`.
3. For each ID, read its size and bytes.
4. EXIF blocks in HEIF are stored with a leading 4-byte TIFF-header-offset prefix (per ISO/IEC 23008-12). Strip it if present so callers receive a standard EXIF buffer compatible with `little_exif` and `kamadak-exif`.
5. Return the bytes. The metadata module wraps them like any other EXIF buffer and writes them to the output file.

The engine, pipeline, presets, LUTs, and encode side are not modified.

## Build & feature flag

A new `heic = []` feature, opt-in, matches the existing `raw = []` feature. Neither is in `default`.

`build.rs` extends with a `#[cfg(feature = "heic")]` block paralleling the existing `raw` block:

```rust
#[cfg(feature = "heic")]
{
    println!("cargo:rustc-link-lib=heif");

    if cfg!(target_os = "macos") {
        if let Ok(output) = std::process::Command::new("brew")
            .args(["--prefix", "libheif"])
            .output()
        {
            if output.status.success() {
                let prefix = String::from_utf8_lossy(&output.stdout).trim().to_string();
                println!("cargo:rustc-link-search=native={prefix}/lib");
            }
        }
    }
}
```

No `cc::Build` step — there is no helper C file.

On macOS, `brew install libheif` provides the library and headers. On Linux, `apt install libheif-dev` (or distro equivalent) does the same. The decode README's "Building from source" section gains a HEIC entry with these install steps. Behavior when the system library is missing is a normal linker error — same outcome as a missing LibRaw install.

Downstream crates do not introduce their own `heic` feature. Instead, they hardcode `heic` in the feature list of their `agx-photo` dependency, matching how `raw` is wired today:

- `agx-cli/Cargo.toml`: change `agx = { ..., features = ["raw", "validate"] }` to `agx = { ..., features = ["raw", "validate", "heic"] }`. The shipped CLI binary always has HEIC support enabled, just like it always has raw enabled.
- `agx-e2e/Cargo.toml`: change the dev-dep `agx = { ..., features = ["raw"] }` to `agx = { ..., features = ["raw", "heic"] }`.

A library consumer who wants to disable HEIC at the call-site level can depend on `agx-photo` without the `heic` feature and skip libheif entirely.

## Error handling

All errors return `AgxError::Decode(String)` with a `"libheif: <message>"` prefix for runtime errors. The mapping:

| Scenario                                            | Source                                                  | Behavior                                                                                              |
| --------------------------------------------------- | ------------------------------------------------------- | ----------------------------------------------------------------------------------------------------- |
| Feature off, `.heic`/`.heif` extension              | `decode()` route                                        | `Err("heic format support requires the 'heic' feature flag")`                                         |
| File missing / unreadable                           | `heif_context_read_from_file` returns error             | `Err("libheif: <libheif's error string>")`                                                            |
| File is not a valid HEIF                            | same                                                    | same                                                                                                  |
| No primary image                                    | `heif_context_get_primary_image_handle` fails           | `Err("libheif: file has no primary image")`                                                           |
| Unsupported codec (e.g., AV1 with no decoder)       | `heif_decode_image` fails                               | `Err("libheif: codec '...' not available — install the required decoder backend")`                    |
| Unsupported bit depth (not 8 or 10)                 | post-decode check                                       | `Err("libheif: unsupported bit depth {bits}")`                                                        |
| ICC profile present, not NCLX                       | profile-type inspection                                 | Not an error. `eprintln!` warning, treat as sRGB, continue.                                           |
| Unknown / unhandled NCLX matrix                     | NCLX inspection                                         | Same — warn, treat as sRGB, continue.                                                                 |

The ICC and unknown-matrix cases are deliberately warnings rather than errors, matching the project's existing apply-time-warnings precedent (preset unknown fields, see `2026-04-30-agx-validate-design.md`). The render still produces an output; the user is told what was approximated.

`extract_heic_metadata` returns `Option<Vec<u8>>` and treats all error paths as `None`, matching the raw metadata extractor.

All FFI resources (contexts, image handles, decoded images) are wrapped in RAII types whose `Drop` impl calls the matching `_release` function. The `?` operator on a fallible step propagates after the wrapper is constructed, so partial failures release cleanly. If a wrapper constructor itself fails, the FFI didn't allocate anything to release.

The `heic` and `raw` features are independent. A user can enable either, both, or neither; each FFI block in `build.rs` is gated by its own `#[cfg(feature = "...")]`.

## Color space mapping

For this initial scope, libheif's reported NCLX matrix coefficients are interpreted as follows:

- BT.709 / sRGB (matrix coefficient 1) → treat pixels as sRGB primaries directly.
- Display P3 (matrix coefficient 12, primaries P3-D65) → apply `palette`'s `LinSrgb::from_color(LinP3<f32>::new(...))` or the equivalent 3x3 matrix on the linear values, after gamma decode.
- BT.2020 (matrix coefficient 9 or 10) → analogous matrix conversion via `palette`'s BT.2020 spaces.
- Anything else, including ICC profiles → warn and treat as sRGB.

This is a one-way mapping. Wide-gamut iPhone captures are gamut-compressed to sRGB at decode and the original gamut is not recoverable through the rest of the pipeline. The color-management backlog item ("Revisit HEIC wide-gamut preservation") tracks the eventual switch: when AgX's working space widens, this decode-time conversion is dropped and the source colors survive end-to-end.

The decode-side conversion is a 3x3 matrix per pixel on top of the gamma decode that all standard images already pay; cost is negligible.

## Public API additions

New public items in the `agx` crate:

- `decode::is_heic_extension(path: &Path) -> bool` — extension predicate. Always compiled (not feature-gated), matching `is_raw_extension`. Callers and the unified `decode()` router can check extension regardless of which decode backends are linked.
- `decode::heic::decode_heic(path: &Path) -> Result<Rgb32FImage>` — module-scoped behind `#[cfg(feature = "heic")]`. Mirrors `decode::raw::decode_raw`.
- `decode::heic::extract_heic_metadata(path: &Path) -> Option<Vec<u8>>` — module-scoped behind `#[cfg(feature = "heic")]`. Mirrors `decode::raw::extract_raw_metadata`.

The unified `decode::decode(path)` entry point automatically routes HEIC files when the feature is on, so most callers (including the CLI) don't need to know HEIC is a separate path.

## Testing

Unit tests in `decode/heic.rs`, all gated by `#[cfg(feature = "heic")]`:

- `heif_context_init_and_drop` — RAII smoke test for the context wrapper.
- `decode_heic_nonexistent_file_returns_error` — error path on a path that does not exist; verify the error message contains `"libheif"`.
- `extract_heic_metadata_nonexistent_returns_none` — error path on a path that does not exist.

Library smoke test in `crates/agx-e2e/tests/library_pipeline.rs`:

- One additional test: decode a small HEIC fixture, verify dimensions match expected, sample a few pixel positions for sanity.

End-to-end tests in `crates/agx-e2e/`:

- New fixture directory `fixtures/heic/`. Source: real iPhone HEIC captures contributed by the project owner, downscaled to ~1024px longest edge and re-encoded to HEIC. Initial coverage targets one landscape, one portrait, ideally one HDR-mode shot.
- Existing data-driven matrix extends: each HEIC fixture is rendered against the 11 curated color looks plus a noop, producing 12 goldens per fixture, the same as JPEG. New golden tree `fixtures/golden/heic/`.
- Golden tolerance: strict (`tolerance=2, max_diff_pct=0.0`), the same as JPEG. HEVC decode is integer fixed-point per the standard and should be byte-identical across libheif/libde265 versions and across platforms. If cross-platform diffs surface in CI we fall back to permissive (`tolerance=100, max_diff_pct=25.0`) like the raw path, but the expectation is strict.

CI workflow:

- Ubuntu jobs: add `sudo apt-get install -y libheif-dev` alongside the existing `libraw-dev` step.
- macOS jobs: add `brew install libheif` alongside the existing LibRaw install.
- No CI workflow flag changes needed beyond the system-package step: `agx-cli` and `agx-e2e` hardcode the `heic` feature on their `agx-photo` dependency, so every default `cargo build` and `cargo test` in those crates exercises the HEIC path.

Local developer experience:

- Once `agx-cli` and `agx-e2e` hardcode the `heic` feature on their `agx-photo` dependency, contributors need libheif installed on their machine to build the CLI or run e2e. This is the same situation as LibRaw today — `./scripts/verify.sh` and the test suites assume the system libraries are available.
- The `agx-photo` library itself can still be built without libheif by depending on it without the `heic` feature (the library-level feature flag remains opt-in for downstream consumers).
- `./scripts/e2e-quick.sh` includes a HEIC smoke case in its matrix.
- `decode/README.md` documents the install steps and the `--features heic` flag for direct library users.

## Documentation updates

HEIC is not algorithmic, so the explanation quadrant of the book stays untouched. The updates are structural and contributor-facing.

**Rustdoc (`///` doc comments).** The `agx-photo` crate enforces `deny(missing_docs)`, so every new `pub` item ships with doc comments:

- Module-level `//!` doc on `decode/heic.rs` describing the FFI wrapper and the linear-sRGB output contract.
- `///` comments on `is_heic_extension`, `heic::decode_heic`, `heic::extract_heic_metadata`, including supported file extensions, the feature-flag prerequisite, and the color-space gamut-mapping note.

**`crates/agx/src/decode/README.md`.** Extend the existing structure:

- "Public API" section: add the three new items.
- "Extension Guide" section: a third bullet for HEIC, explaining that adding a new HEIF variant is just adding the extension to `HEIC_EXTENSIONS` (libheif handles the codec).
- "Building from source" entry for libheif (the section already covers LibRaw).

**`ARCHITECTURE.md`.**

- Module-table row for `metadata` (line 61): extend the "May import from" cell to include `heic::extract_heic_metadata` alongside the existing `raw::extract_raw_metadata`.
- Negative-constraints bullet for `decode` (line 76): broaden "No metadata interpretation beyond what LibRaw provides" to also acknowledge libheif's EXIF blob, since the wording today is LibRaw-specific.
- Core invariants section: no change — output is still linear sRGB.

**Root `README.md`.**

- Features list near the top: add HEIC alongside the existing "Raw format support" bullet.
- The "EXIF orientation: automatic ... (JPEG, PNG, TIFF)" line: extend to mention HEIC.
- The `// Decode an image (auto-detects format: JPEG, PNG, TIFF, CR2, NEF, DNG, etc.)` comment in the library usage example: add HEIC to the format list.
- "System requirements" section: a libheif install block (brew on macOS, apt on Linux) paralleling the existing libraw block.

**Book `docs/book/src/install.md`.** Currently the book install page silently assumes the system libraries are already installed — the existing LibRaw prerequisite is documented only in the root README. Adding HEIC is a natural moment to fix that gap:

- New "System prerequisites" subsection in `install.md` that documents both libraw and libheif install steps (matching the README content, with a one-line callout that `cargo install agx-cli` will fail to link without them).

**Auto-generated reference (`agx-docgen`).** No manual change needed. The CLI reference is generated from `clap-markdown`; HEIC files are accepted by the same `apply` / `batch-apply` / `multi-apply` subcommands so help text doesn't change. The preset reference is unaffected. If the docgen run produces a drift, it's regenerated as part of the implementation.

**No book quadrant changes.** No tutorial, how-to, explanation, or reference-concepts page needs a new section. HEIC is a format unlock, not a new feature or concept.

## Verification before merge

1. `./scripts/verify.sh` passes on a machine with libheif installed. Because `agx-cli` and `agx-e2e` hardcode the `heic` feature on their `agx-photo` dependency, the HEIC code path is built and its unit tests run. `verify.sh` also runs `markdown-lint`, `book-linkcheck`, and `sibling-md-clean`, which catch any doc-update mistakes.
2. `cargo build -p agx-photo --no-default-features` passes — verifies the library still builds with no FFI features at all.
3. `./scripts/e2e-quick.sh` passes on a machine with libheif installed.
4. `./scripts/e2e.sh` passes in CI (full matrix) with libheif installed on each runner.
5. `ARCHITECTURE.md` updated per the Documentation updates section above.
6. `crates/agx/src/decode/README.md` updated with the new public API and build instructions.
7. Root `README.md` and `docs/book/src/install.md` updated per the Documentation updates section above.
8. `docs/backlog/heic-support.md` shipped sub-tasks checked off; deferred sub-tasks remain.
