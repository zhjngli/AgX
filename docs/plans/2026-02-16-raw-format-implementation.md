# Raw Format Support Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Decode raw photo files (CR2, CR3, NEF, ARW, RAF, DNG, etc.) via LibRaw FFI, and provide a unified `decode()` function that auto-detects format from file extension.

**Architecture:** Thin FFI wrapper around LibRaw's C API (~8 functions). LibRaw handles the full raw processing pipeline (demosaic + color conversion → 16-bit sRGB). We convert the output to our linear sRGB f32 format, same as standard image decoding. Raw support is gated behind a `raw` cargo feature flag. A RAII wrapper ensures LibRaw resources are cleaned up.

**Tech Stack:** Rust 2021, LibRaw (system C library, linked dynamically), no new Rust crate dependencies

---

## Context

This plan implements the design in `docs/plans/2026-02-16-raw-format-support-design.md`. The existing `decode_standard()` in `crates/oxiraw/src/decode/mod.rs` handles JPEG/PNG/TIFF via the `image` crate. We're adding a parallel `decode_raw()` path for raw formats, and a unified `decode()` entry point.

**Prerequisite:** LibRaw must be installed on the build machine before starting:
```bash
# macOS
brew install libraw

# Ubuntu/Debian
sudo apt install libraw-dev

# Fedora
sudo dnf install LibRaw-devel
```

Verify installation: `ls /opt/homebrew/lib/libraw.dylib` (macOS) or `ldconfig -p | grep libraw` (Linux).

## Critical Files

| File | Purpose |
|------|---------|
| `crates/oxiraw/build.rs` | Create: conditional LibRaw linking |
| `crates/oxiraw/src/decode/raw.rs` | Create: FFI bindings + decode_raw() + RAII wrapper |
| `crates/oxiraw/src/decode/mod.rs` | Modify: add unified decode() + format detection + re-export raw module |
| `crates/oxiraw/Cargo.toml` | Modify: add `raw` feature flag |
| `crates/oxiraw-cli/Cargo.toml` | Modify: enable `raw` feature |
| `crates/oxiraw-cli/src/main.rs` | Modify: use unified decode() instead of decode_standard() |
| `crates/oxiraw/src/lib.rs` | Modify: re-export decode function |

---

## Phase 1: Build System + FFI Bindings

### Task 1.1: Install LibRaw and add feature flag + build.rs

**Files:**
- Modify: `crates/oxiraw/Cargo.toml`
- Create: `crates/oxiraw/build.rs`

**Step 1: Install LibRaw**

Run: `brew install libraw`

Verify: `ls /opt/homebrew/lib/libraw.dylib` should exist.

**Step 2: Add feature flag to Cargo.toml**

Add to `crates/oxiraw/Cargo.toml` after the `[dependencies]` section:

```toml
[features]
default = []
raw = []
```

**Step 3: Create build.rs**

Create `crates/oxiraw/build.rs`:

```rust
fn main() {
    #[cfg(feature = "raw")]
    {
        // Link to LibRaw system library
        println!("cargo:rustc-link-lib=raw");

        // On macOS with Homebrew, add the library search path
        if cfg!(target_os = "macos") {
            if let Ok(output) = std::process::Command::new("brew")
                .args(["--prefix", "libraw"])
                .output()
            {
                if output.status.success() {
                    let prefix = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    println!("cargo:rustc-link-search=native={prefix}/lib");
                }
            }
        }
    }
}
```

**Step 4: Verify it compiles**

Run: `cargo build -p oxiraw`
Expected: compiles without LibRaw (feature not enabled)

Run: `cargo build -p oxiraw --features raw`
Expected: compiles and links against LibRaw

**Step 5: Stage**

`git add crates/oxiraw/Cargo.toml crates/oxiraw/build.rs`

---

### Task 1.2: FFI bindings and RAII wrapper

**Files:**
- Create: `crates/oxiraw/src/decode/raw.rs`
- Modify: `crates/oxiraw/src/decode/mod.rs`

**Step 1: Create raw.rs with FFI declarations and RAII wrapper**

Create `crates/oxiraw/src/decode/raw.rs`:

```rust
//! Raw format decoding via LibRaw FFI.
//!
//! This module provides thin FFI bindings to LibRaw's C API and a safe
//! `decode_raw()` function that converts raw photo files to linear sRGB f32.

use std::ffi::{c_char, c_int, c_uint, CString};
use std::path::Path;

use image::{Rgb, Rgb32FImage};
use palette::{LinSrgb, Srgb};

use crate::error::{OxirawError, Result};

// --- FFI declarations ---

/// Opaque LibRaw processor handle.
#[repr(C)]
struct libraw_data_t {
    _opaque: [u8; 0],
}

/// Processed image output from LibRaw.
#[repr(C)]
struct libraw_processed_image_t {
    /// Image format type (0 = bitmap).
    _type: c_uint,
    /// Image height in pixels.
    height: u16,
    /// Image width in pixels.
    width: u16,
    /// Number of color channels (3 for RGB).
    colors: u16,
    /// Bits per channel (8 or 16).
    bits: u16,
    /// Size of the data array in bytes.
    data_size: c_uint,
    /// Pixel data (variable length, we access via pointer).
    data: [u8; 1],
}

extern "C" {
    fn libraw_init(flags: c_uint) -> *mut libraw_data_t;
    fn libraw_open_file(data: *mut libraw_data_t, fname: *const c_char) -> c_int;
    fn libraw_unpack(data: *mut libraw_data_t) -> c_int;
    fn libraw_dcraw_process(data: *mut libraw_data_t) -> c_int;
    fn libraw_dcraw_make_mem_image(
        data: *mut libraw_data_t,
        errc: *mut c_int,
    ) -> *mut libraw_processed_image_t;
    fn libraw_dcraw_clear_mem(img: *mut libraw_processed_image_t);
    fn libraw_recycle(data: *mut libraw_data_t);
    fn libraw_close(data: *mut libraw_data_t);
    fn libraw_strerror(err: c_int) -> *const c_char;
}

// LibRaw output_params_t is embedded inside libraw_data_t. We need to set
// fields on it before calling dcraw_process. The offset varies by LibRaw
// version, so we use the C API's setter approach instead — but LibRaw's C API
// doesn't have setters for output params. We access them via a known offset.
//
// Alternative: use libraw_set_output_bps etc. if available, or just accept
// LibRaw's defaults (8-bit sRGB with camera WB).
//
// For simplicity in this first version, we accept LibRaw's defaults:
// - 8-bit output (we'll convert to f32 anyway)
// - sRGB color space
// - Camera white balance if available, auto otherwise
// - Auto brightness

// --- Error helper ---

fn libraw_error_msg(err: c_int) -> String {
    unsafe {
        let ptr = libraw_strerror(err);
        if ptr.is_null() {
            return format!("LibRaw error code {err}");
        }
        std::ffi::CStr::from_ptr(ptr)
            .to_string_lossy()
            .into_owned()
    }
}

fn check_libraw(err: c_int) -> Result<()> {
    if err == 0 {
        Ok(())
    } else {
        Err(OxirawError::Decode(format!("LibRaw: {}", libraw_error_msg(err))))
    }
}

// --- RAII wrapper ---

/// RAII wrapper for a LibRaw processor. Calls `libraw_close` on drop.
struct LibRawProcessor {
    ptr: *mut libraw_data_t,
}

impl LibRawProcessor {
    fn new() -> Result<Self> {
        let ptr = unsafe { libraw_init(0) };
        if ptr.is_null() {
            return Err(OxirawError::Decode("LibRaw: failed to initialize".into()));
        }
        Ok(Self { ptr })
    }

    fn open_file(&self, path: &Path) -> Result<()> {
        let c_path = CString::new(
            path.to_str()
                .ok_or_else(|| OxirawError::Decode("invalid file path encoding".into()))?,
        )
        .map_err(|_| OxirawError::Decode("file path contains null byte".into()))?;
        check_libraw(unsafe { libraw_open_file(self.ptr, c_path.as_ptr()) })
    }

    fn unpack(&self) -> Result<()> {
        check_libraw(unsafe { libraw_unpack(self.ptr) })
    }

    fn process(&self) -> Result<()> {
        check_libraw(unsafe { libraw_dcraw_process(self.ptr) })
    }

    fn make_mem_image(&self) -> Result<ProcessedImage> {
        let mut errc: c_int = 0;
        let ptr = unsafe { libraw_dcraw_make_mem_image(self.ptr, &mut errc) };
        if ptr.is_null() {
            return Err(OxirawError::Decode(format!(
                "LibRaw: failed to create memory image: {}",
                libraw_error_msg(errc)
            )));
        }
        Ok(ProcessedImage { ptr })
    }
}

impl Drop for LibRawProcessor {
    fn drop(&mut self) {
        unsafe {
            libraw_recycle(self.ptr);
            libraw_close(self.ptr);
        }
    }
}

/// RAII wrapper for a LibRaw processed image. Calls `libraw_dcraw_clear_mem` on drop.
struct ProcessedImage {
    ptr: *mut libraw_processed_image_t,
}

impl ProcessedImage {
    fn width(&self) -> u32 {
        unsafe { (*self.ptr).width as u32 }
    }

    fn height(&self) -> u32 {
        unsafe { (*self.ptr).height as u32 }
    }

    fn colors(&self) -> u16 {
        unsafe { (*self.ptr).colors }
    }

    fn bits(&self) -> u16 {
        unsafe { (*self.ptr).bits }
    }

    fn data_size(&self) -> usize {
        unsafe { (*self.ptr).data_size as usize }
    }

    /// Get a slice of the raw pixel data bytes.
    fn data(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts((*self.ptr).data.as_ptr(), self.data_size()) }
    }
}

impl Drop for ProcessedImage {
    fn drop(&mut self) {
        unsafe {
            libraw_dcraw_clear_mem(self.ptr);
        }
    }
}

// --- Public API ---

/// Decode a raw photo file into linear sRGB f32 using LibRaw.
///
/// LibRaw handles the full processing pipeline: file parsing, unpacking,
/// demosaicing, color conversion, and white balance. The output is sRGB
/// which we convert to linear sRGB f32 for the oxiraw engine.
///
/// # Supported formats
///
/// CR2, CR3, NEF, NRW, ARW, SRF, SR2, RAF, DNG, RW2, ORF, PEF, SRW,
/// and many more — anything LibRaw supports (~1000 camera models).
pub fn decode_raw(path: &Path) -> Result<Rgb32FImage> {
    let processor = LibRawProcessor::new()?;
    processor.open_file(path)?;
    processor.unpack()?;
    processor.process()?;

    let img = processor.make_mem_image()?;

    let width = img.width();
    let height = img.height();
    let colors = img.colors();
    let bits = img.bits();

    if colors != 3 {
        return Err(OxirawError::Decode(format!(
            "LibRaw: expected 3 color channels, got {colors}"
        )));
    }

    let data = img.data();

    let linear = match bits {
        8 => {
            // 8-bit sRGB: convert each byte to f32 sRGB then to linear
            Rgb32FImage::from_fn(width, height, |x, y| {
                let idx = ((y * width + x) * 3) as usize;
                let sr = data[idx] as f32 / 255.0;
                let sg = data[idx + 1] as f32 / 255.0;
                let sb = data[idx + 2] as f32 / 255.0;
                let lin: LinSrgb<f32> = Srgb::new(sr, sg, sb).into_linear();
                Rgb([lin.red, lin.green, lin.blue])
            })
        }
        16 => {
            // 16-bit sRGB: read u16 pairs, convert to f32 sRGB then to linear
            Rgb32FImage::from_fn(width, height, |x, y| {
                let idx = ((y * width + x) * 3) as usize * 2;
                let sr = u16::from_ne_bytes([data[idx], data[idx + 1]]) as f32 / 65535.0;
                let sg = u16::from_ne_bytes([data[idx + 2], data[idx + 3]]) as f32 / 65535.0;
                let sb = u16::from_ne_bytes([data[idx + 4], data[idx + 5]]) as f32 / 65535.0;
                let lin: LinSrgb<f32> = Srgb::new(sr, sg, sb).into_linear();
                Rgb([lin.red, lin.green, lin.blue])
            })
        }
        _ => {
            return Err(OxirawError::Decode(format!(
                "LibRaw: unsupported bit depth {bits}"
            )));
        }
    };

    Ok(linear)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn libraw_processor_init_and_drop() {
        // Just verify we can create and drop a processor without crashing
        let processor = LibRawProcessor::new().unwrap();
        drop(processor);
    }

    #[test]
    fn decode_raw_nonexistent_file_returns_error() {
        let result = decode_raw(Path::new("/nonexistent/photo.cr2"));
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("LibRaw"),
            "Error should mention LibRaw: {err_msg}"
        );
    }
}
```

**Step 2: Add raw module to decode/mod.rs**

Add at the top of `crates/oxiraw/src/decode/mod.rs`, before the `use` statements:

```rust
#[cfg(feature = "raw")]
pub mod raw;
```

**Step 3: Verify it compiles**

Run: `cargo build -p oxiraw --features raw`
Expected: PASS — compiles and links against LibRaw

Run: `cargo test -p oxiraw --features raw decode`
Expected: PASS — both new tests pass (init/drop, nonexistent file error)

**Step 4: Stage**

`git add crates/oxiraw/src/decode/raw.rs crates/oxiraw/src/decode/mod.rs`

---

## Phase 2: Unified Decode + Format Detection

### Task 2.1: Format detection and unified decode()

**Files:**
- Modify: `crates/oxiraw/src/decode/mod.rs`

**Step 1: Write failing tests**

Add to the `mod tests` block in `decode/mod.rs`:

```rust
#[test]
fn is_raw_extension_detects_common_formats() {
    assert!(is_raw_extension(std::path::Path::new("photo.cr2")));
    assert!(is_raw_extension(std::path::Path::new("photo.CR2")));
    assert!(is_raw_extension(std::path::Path::new("photo.nef")));
    assert!(is_raw_extension(std::path::Path::new("photo.arw")));
    assert!(is_raw_extension(std::path::Path::new("photo.raf")));
    assert!(is_raw_extension(std::path::Path::new("photo.dng")));
    assert!(is_raw_extension(std::path::Path::new("photo.cr3")));
    assert!(is_raw_extension(std::path::Path::new("photo.rw2")));
}

#[test]
fn is_raw_extension_rejects_standard_formats() {
    assert!(!is_raw_extension(std::path::Path::new("photo.jpg")));
    assert!(!is_raw_extension(std::path::Path::new("photo.png")));
    assert!(!is_raw_extension(std::path::Path::new("photo.tiff")));
    assert!(!is_raw_extension(std::path::Path::new("photo.bmp")));
}

#[test]
fn decode_routes_jpg_to_standard() {
    let temp_path = std::env::temp_dir().join("oxiraw_test_unified.png");
    let img: ImageBuffer<Rgb<u8>, Vec<u8>> =
        ImageBuffer::from_pixel(2, 2, Rgb([128, 128, 128]));
    img.save(&temp_path).unwrap();

    let result = decode(&temp_path);
    assert!(result.is_ok());

    let _ = std::fs::remove_file(&temp_path);
}

#[test]
fn decode_nonexistent_raw_file_returns_error() {
    let result = decode(std::path::Path::new("/nonexistent/photo.cr2"));
    assert!(result.is_err());
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p oxiraw --features raw decode::tests`
Expected: FAIL — `is_raw_extension` and `decode` not defined

**Step 3: Write implementation**

Add to `crates/oxiraw/src/decode/mod.rs`, after the `raw` module declaration and before `decode_standard`:

```rust
/// Known raw file extensions supported via LibRaw.
const RAW_EXTENSIONS: &[&str] = &[
    "cr2", "cr3", "crw", "nef", "nrw", "arw", "srf", "sr2", "raf", "dng", "rw2", "orf", "pef",
    "srw", "x3f", "3fr", "fff", "iiq", "rwl", "mrw", "mdc", "dcr", "raw", "kdc", "erf", "mef",
    "mos",
];

/// Check if a file path has a known raw format extension.
pub fn is_raw_extension(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| RAW_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()))
}

/// Decode any supported image file into linear sRGB f32.
///
/// Auto-detects format from file extension:
/// - Standard formats (JPEG, PNG, TIFF, BMP, WebP): decoded via the `image` crate
/// - Raw formats (CR2, CR3, NEF, ARW, RAF, DNG, etc.): decoded via LibRaw (requires `raw` feature)
pub fn decode(path: &std::path::Path) -> Result<Rgb32FImage> {
    if is_raw_extension(path) {
        #[cfg(feature = "raw")]
        {
            return raw::decode_raw(path);
        }
        #[cfg(not(feature = "raw"))]
        {
            return Err(OxirawError::Decode(
                "raw format support requires the 'raw' feature flag".into(),
            ));
        }
    }
    decode_standard(path)
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo test -p oxiraw --features raw decode::tests`
Expected: PASS (all 6 decode tests — 2 existing + 4 new)

Also run without the raw feature to verify standard path still works:
Run: `cargo test -p oxiraw decode::tests`
Expected: PASS (the raw-specific test `decode_nonexistent_raw_file_returns_error` should still pass since it goes through the feature gate)

**Step 5: Stage**

`git add crates/oxiraw/src/decode/mod.rs`

---

## Phase 3: CLI Integration

### Task 3.1: Update CLI to use unified decode()

**Files:**
- Modify: `crates/oxiraw-cli/Cargo.toml`
- Modify: `crates/oxiraw-cli/src/main.rs`

**Step 1: Enable raw feature in CLI**

In `crates/oxiraw-cli/Cargo.toml`, change:

```toml
oxiraw = { path = "../oxiraw" }
```

to:

```toml
oxiraw = { path = "../oxiraw", features = ["raw"] }
```

**Step 2: Update CLI to use decode()**

In `crates/oxiraw-cli/src/main.rs`, replace both occurrences of `decode_standard` with `decode`:

In `run_apply`:
```rust
let linear = oxiraw::decode::decode(input)?;
```

In `run_edit`:
```rust
let linear = oxiraw::decode::decode(input)?;
```

**Step 3: Verify CLI compiles and works**

Run: `cargo build -p oxiraw-cli`
Expected: compiles, links against LibRaw

Run: `cargo run -p oxiraw-cli -- edit --help`
Expected: shows help (unchanged)

Run existing CLI tests:
Run: `cargo test -p oxiraw-cli`
Expected: PASS (all 5 existing tests still pass — they use PNG files which route through decode_standard)

**Step 4: Stage**

`git add crates/oxiraw-cli/Cargo.toml crates/oxiraw-cli/src/main.rs`

---

### Task 3.2: Add re-export and update lib.rs

**Files:**
- Modify: `crates/oxiraw/src/lib.rs`

**Step 1: Add decode re-export**

The `decode` module is already public. Users can call `oxiraw::decode::decode()`. No changes to `lib.rs` needed unless we want a top-level re-export.

Add a convenience re-export to `crates/oxiraw/src/lib.rs`:

```rust
pub use decode::decode;
```

**Step 2: Verify**

Run: `cargo test --workspace`
Expected: all tests pass

**Step 3: Stage**

`git add crates/oxiraw/src/lib.rs`

---

## Phase 4: End-to-End Testing with a Real Raw File

### Task 4.1: Download a sample DNG and add integration test

**Files:**
- Modify: `crates/oxiraw-cli/tests/integration.rs`

**Step 1: Find and download a small sample DNG**

We need a small raw file for testing. Options:
- Use `libraw_init` + check that processing works on a known test file
- Download a tiny DNG from a test suite

For the integration test, create a test that downloads or uses a DNG. Since we don't want large binaries in the repo, we'll create a test that only runs when a raw file is available:

Add to `crates/oxiraw-cli/tests/integration.rs`:

```rust
/// Test that the CLI can process a raw file.
/// This test is ignored by default since it requires a sample raw file.
/// To run: place a .dng file at /tmp/oxiraw_test_sample.dng and run:
///   cargo test -p oxiraw-cli -- --ignored cli_edit_raw_file
#[test]
#[ignore]
fn cli_edit_raw_file() {
    let input = std::path::PathBuf::from("/tmp/oxiraw_test_sample.dng");
    if !input.exists() {
        eprintln!("Skipping: no sample raw file at {}", input.display());
        return;
    }

    let output = std::env::temp_dir().join("oxiraw_cli_raw_out.jpg");

    let status = cli_bin()
        .args([
            "edit",
            "-i", input.to_str().unwrap(),
            "-o", output.to_str().unwrap(),
            "--exposure", "0.5",
        ])
        .status()
        .expect("failed to run CLI");

    assert!(status.success(), "CLI should process raw file successfully");
    assert!(output.exists(), "Output file should exist");

    // Verify the output is a valid image with reasonable dimensions
    let out_img = image::open(&output).unwrap();
    assert!(out_img.width() > 0);
    assert!(out_img.height() > 0);

    let _ = std::fs::remove_file(&output);
}
```

Also add a library-level test in `crates/oxiraw/src/decode/mod.rs`:

```rust
/// Test decode() with a real raw file. Ignored by default.
#[test]
#[ignore]
fn decode_real_raw_file() {
    let path = std::path::Path::new("/tmp/oxiraw_test_sample.dng");
    if !path.exists() {
        eprintln!("Skipping: no sample raw file at {}", path.display());
        return;
    }

    let result = decode(path);
    assert!(result.is_ok(), "Failed to decode raw file: {:?}", result.err());

    let img = result.unwrap();
    assert!(img.width() > 0);
    assert!(img.height() > 0);

    // Verify pixels are in a reasonable range (linear sRGB, mostly 0-1)
    let pixel = img.get_pixel(img.width() / 2, img.height() / 2);
    for i in 0..3 {
        assert!(
            pixel.0[i] >= 0.0 && pixel.0[i] <= 2.0,
            "Pixel channel {} out of expected range: {}",
            i,
            pixel.0[i]
        );
    }
}
```

**Step 2: Run the non-ignored tests**

Run: `cargo test --workspace`
Expected: all tests pass (ignored tests are skipped)

**Step 3: Manual verification with a real raw file**

Download a sample DNG (or use one of your own raw files):

```bash
# If you have a raw file:
cp /path/to/your/photo.dng /tmp/oxiraw_test_sample.dng

# Process it
cargo run -p oxiraw-cli -- edit \
  -i /tmp/oxiraw_test_sample.dng \
  -o /tmp/raw_test_output.jpg \
  --exposure 0.5

# Verify the output
open /tmp/raw_test_output.jpg
```

Also run the ignored tests:

```bash
cargo test --workspace -- --ignored
```

**Step 4: Stage**

`git add crates/oxiraw-cli/tests/integration.rs crates/oxiraw/src/decode/mod.rs`

---

## Phase 5: Documentation

### Task 5.1: Update READMEs and docs

**Files:**
- Modify: `README.md`
- Modify: `example/README.md`

**Step 1: Update README.md**

Add "Raw format support" to the features list:

```markdown
- **Raw format support**: decode CR2, CR3, NEF, ARW, RAF, DNG, and 1000+ camera formats via LibRaw
```

Add a raw section to Quick Start:

```bash
# Process a raw file
cargo run -p oxiraw-cli -- edit \
  -i photo.dng \
  -o edited.jpg \
  --exposure 0.5 --contrast 15
```

Add a "Building with Raw Support" section:

```markdown
## Building with Raw Support

Raw format decoding requires [LibRaw](https://www.libraw.org/) installed on your system:

\`\`\`bash
# macOS
brew install libraw

# Ubuntu/Debian
sudo apt install libraw-dev
\`\`\`

The CLI enables raw support by default. To use the library without raw support (no LibRaw dependency):

\`\`\`rust
// Cargo.toml — no "raw" feature, only standard formats
oxiraw = "0.1"
\`\`\`
```

**Step 2: Stage**

`git add README.md example/README.md`

---

## Phase 6: Final Verification

### Task 6.1: Full test suite + cargo fmt

Run: `cargo fmt --all`
Run: `cargo test --workspace`
Expected: all tests pass

Run: `cargo test --workspace -- --ignored` (if sample raw file is available)
Expected: raw decode tests pass

---

## Summary

| Phase | Tasks | Tests Added | Key Deliverable |
|-------|-------|-------------|-----------------|
| 1 | 1.1–1.2 | 2 | LibRaw FFI bindings, RAII wrapper, build.rs |
| 2 | 2.1 | 4 | Unified decode() with format auto-detection |
| 3 | 3.1–3.2 | 0 | CLI uses unified decode, raw feature enabled |
| 4 | 4.1 | 2 (ignored) | End-to-end raw file tests |
| 5 | 5.1 | 0 | README updates |
| 6 | 6.1 | 0 | Final verification |
