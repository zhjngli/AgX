# Raw Format Support Design

**Date**: 2026-02-16
**Status**: Approved

## Overview

Add support for decoding raw photo files (CR2, CR3, NEF, ARW, RAF, DNG, RW2, ORF, PEF, etc.) using LibRaw via FFI. Raw files are processed by LibRaw into sRGB and fed into the existing oxiraw pipeline. A unified `decode()` function auto-detects format from file extension.

This is the first phase of raw support. Future phases will add color-space-aware processing (working in ProPhoto RGB or Adobe RGB internally) and eventually a pluggable pipeline with direct access to raw sensor data for custom demosaicing.

## Background: Raw Files and Demosaicing

Camera sensors are monochrome — each pixel measures light intensity through a color filter (Color Filter Array / CFA). The most common pattern is Bayer (2x2 RGGB repeat). Fujifilm X-Trans uses a 6x6 pseudo-random pattern.

**Demosaicing** reconstructs a full RGB image from the single-channel-per-pixel mosaic. Different CFA patterns require different algorithms:

- **Bayer (most cameras):** AHD, PPG, AMaZE, VNG. Well-understood, many implementations.
- **X-Trans (Fujifilm):** Markesteijn algorithm. Requires entirely separate code path from Bayer algorithms.

LibRaw handles demosaicing for both Bayer and X-Trans sensors using algorithms inherited from dcraw (AHD, PPG). These are good enough for most use cases. Higher-quality algorithms (AMaZE, Markesteijn 3-pass) exist in darktable/RawTherapee but are out of scope.

## Design

### System Dependency

LibRaw must be installed on the build machine:
- macOS: `brew install libraw`
- Linux: `apt install libraw-dev` / `dnf install LibRaw-devel`
- Windows: download from libraw.org

We link dynamically. The dependency is gated behind a cargo feature flag so users who don't need raw support don't need LibRaw installed.

### Feature Flag

```toml
# crates/oxiraw/Cargo.toml
[features]
default = []
raw = []

# crates/oxiraw-cli/Cargo.toml
[dependencies]
oxiraw = { path = "../oxiraw", features = ["raw"] }
```

All raw-related code is behind `#[cfg(feature = "raw")]`. The library compiles and works without LibRaw for standard formats.

### Module Structure

```
crates/oxiraw/src/
├── decode/
│   ├── mod.rs       # unified decode() + format detection + existing decode_standard()
│   └── raw.rs       # LibRaw FFI bindings + decode_raw() (behind "raw" feature)
```

### LibRaw FFI Bindings

`decode/raw.rs` contains thin FFI declarations for the LibRaw C API functions we need:

```rust
extern "C" {
    fn libraw_init(flags: c_uint) -> *mut libraw_data_t;
    fn libraw_open_file(data: *mut libraw_data_t, fname: *const c_char) -> c_int;
    fn libraw_unpack(data: *mut libraw_data_t) -> c_int;
    fn libraw_dcraw_process(data: *mut libraw_data_t) -> c_int;
    fn libraw_dcraw_make_mem_image(data: *mut libraw_data_t, errc: *mut c_int) -> *mut libraw_processed_image_t;
    fn libraw_dcraw_clear_mem(img: *mut libraw_processed_image_t);
    fn libraw_recycle(data: *mut libraw_data_t);
    fn libraw_close(data: *mut libraw_data_t);
    fn libraw_strerror(err: c_int) -> *const c_char;
}
```

Plus the necessary struct definitions (`libraw_data_t` is opaque, `libraw_processed_image_t` has width/height/colors/bits/data fields).

We also need a `build.rs` to link against libraw:

```rust
// crates/oxiraw/build.rs
#[cfg(feature = "raw")]
fn main() {
    println!("cargo:rustc-link-lib=raw");
}

#[cfg(not(feature = "raw"))]
fn main() {}
```

### decode_raw()

```rust
/// Decode a raw photo file into linear sRGB f32 using LibRaw.
pub fn decode_raw(path: &Path) -> Result<Rgb32FImage>
```

The function:
1. Initializes a LibRaw processor (`libraw_init`)
2. Opens the file (`libraw_open_file`)
3. Unpacks sensor data (`libraw_unpack`)
4. Configures output: 16-bit, sRGB color space, auto white balance
5. Processes (demosaic + color conversion) (`libraw_dcraw_process`)
6. Extracts the processed image to memory (`libraw_dcraw_make_mem_image`)
7. Converts 16-bit sRGB pixels to our linear sRGB f32 format (sRGB gamma → linear via palette crate, same as `decode_standard`)
8. Cleans up LibRaw resources (`libraw_dcraw_clear_mem`, `libraw_recycle`, `libraw_close`)

Error handling: LibRaw returns integer error codes. We map them to `OxirawError::Decode` with human-readable messages via `libraw_strerror`.

Resource safety: Use a RAII wrapper struct that calls `libraw_close` on drop, ensuring cleanup even on error paths.

### Unified decode()

```rust
/// Decode any supported image file into linear sRGB f32.
///
/// Auto-detects format from file extension:
/// - Standard formats (JPEG, PNG, TIFF, BMP, WebP): decoded via the `image` crate
/// - Raw formats (CR2, CR3, NEF, ARW, RAF, DNG, etc.): decoded via LibRaw (requires `raw` feature)
pub fn decode(path: &Path) -> Result<Rgb32FImage> {
    if is_raw_extension(path) {
        #[cfg(feature = "raw")]
        return raw::decode_raw(path);
        #[cfg(not(feature = "raw"))]
        return Err(OxirawError::Decode(
            "raw format support requires the 'raw' feature flag".into(),
        ));
    }
    decode_standard(path)
}
```

Format detection uses a set of known raw extensions: `.cr2`, `.cr3`, `.nef`, `.nrw`, `.arw`, `.srf`, `.sr2`, `.raf`, `.dng`, `.rw2`, `.orf`, `.pef`, `.srw`, `.x3f`, `.3fr`, `.fff`, `.iiq`, `.rwl`, `.mrw`, `.mdc`, `.dcr`, `.raw`.

### CLI Changes

Replace `decode_standard()` calls with `decode()` in both `run_apply` and `run_edit`. No new flags — raw files just work when passed as `--input`.

### Error Handling

Reuse the existing `Decode` error variant:

```rust
OxirawError::Decode("LibRaw: unsupported file format".into())
OxirawError::Decode("LibRaw: file not found".into())
OxirawError::Decode("LibRaw: out of memory during processing".into())
```

### Testing

- Unit test: `decode_raw` on a small sample DNG file (we can include a tiny DNG in `example/` or generate one)
- Unit test: `decode()` auto-detection routes `.jpg` to standard and `.cr2` to raw
- Unit test: `decode()` with `raw` feature disabled returns appropriate error for raw extensions
- Integration test: CLI processes a raw file end-to-end
- Integration test: CLI apply preset to a raw file

Note: testing raw decode requires either a sample raw file in the repo or downloading one during tests. A small DNG file (~100KB) is the most portable option since DNG is an open standard.

## Scope

**In scope:**
- LibRaw FFI bindings (thin, ~8 functions)
- `decode_raw()` function with RAII resource management
- Unified `decode()` with format auto-detection
- `raw` cargo feature flag
- `build.rs` for LibRaw linking
- CLI updated to use unified decode
- Tests with a sample DNG file

**Out of scope (future):**
- Custom demosaicing algorithms (AMaZE, Markesteijn)
- Direct access to raw sensor data (pre-demosaic)
- Color space selection (Adobe RGB, ProPhoto RGB working space)
- Camera-specific color profiles
- Raw metadata exposure (EXIF, camera settings)
- White balance override from raw metadata

## Key Decisions

| Decision | Rationale |
|----------|-----------|
| LibRaw via FFI (not pure Rust) | 1000+ camera support immediately. Pure Rust options (rawloader, rawler) are stalled or don't demosaic. |
| Thin FFI wrapper, not existing crate | Existing wrappers (libraw-sys, rsraw) are either abandoned or add unwanted abstractions. Only ~8 C functions needed. |
| Feature flag for raw support | LibRaw is a system dependency. Users who only need JPEG/PNG/TIFF shouldn't need it installed. |
| Dynamic linking | Avoids bundling LibRaw source. Standard practice for system libraries. |
| 16-bit sRGB output from LibRaw | Maximum quality from LibRaw before converting to our f32 pipeline. 8-bit would lose tonal range. |
| Format detection by extension | No reliable magic-byte detection across all raw formats. Extension-based is standard practice (darktable, RawTherapee, Lightroom all do this). |
| RAII wrapper for LibRaw lifecycle | Ensures cleanup on all code paths including panics. |
