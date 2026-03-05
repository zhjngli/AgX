# Harness Engineering Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Apply harness engineering principles to oxiraw — create a metadata module to resolve coupling, a navigable documentation map, structural tests for module layering, and an architecture evolution process.

**Architecture:** Refactor decode↔encode coupling into a clean `metadata` module. Then build: thin `CLAUDE.md` → `ARCHITECTURE.md` map → per-module READMEs → `tests/architecture.rs` → `docs/contributing/evolving-architecture.md`.

**Tech Stack:** Rust standard library (structural test + metadata module), Markdown (all documentation).

**Design doc:** `docs/plans/2026-03-04-harness-engineering-design.md`

---

### Task 1: Create the metadata module

Extract `ImageMetadata` and all extraction functions from `encode` into a new `metadata` module. Change `decode/raw.rs` to return raw bytes. This eliminates the decode↔encode coupling.

**Files:**
- Create: `crates/oxiraw/src/metadata.rs`
- Modify: `crates/oxiraw/src/encode/mod.rs` — remove `ImageMetadata`, `extract_metadata*` functions
- Modify: `crates/oxiraw/src/decode/raw.rs` — change `extract_raw_metadata` return type
- Modify: `crates/oxiraw/src/lib.rs` — add `pub mod metadata`, update re-exports
- Modify: `crates/oxiraw-cli/src/main.rs` — update import paths

**Step 1: Create `crates/oxiraw/src/metadata.rs`**

Move `ImageMetadata` struct, `extract_metadata` orchestrator, `extract_metadata_jpeg`, `extract_metadata_png`, and `extract_metadata_raw_tiff` from `encode/mod.rs` to this new file. The orchestrator calls `crate::decode::is_raw_extension` and `crate::decode::raw::extract_raw_metadata` for raw file strategies.

```rust
//! Image metadata extraction.
//!
//! Extracts EXIF and ICC profile data from source image files for preservation
//! during processing. This module bridges decode and encode — it reads metadata
//! from input files (using format-specific strategies) and provides it to encode
//! for injection into output files.

use crate::error::Result;

/// Extracted metadata from an input image (EXIF, ICC profile).
#[derive(Debug, Clone)]
pub struct ImageMetadata {
    /// Raw EXIF bytes.
    pub exif: Option<Vec<u8>>,
    /// Raw ICC profile bytes.
    pub icc_profile: Option<Vec<u8>>,
}

/// Extract metadata (EXIF, ICC profile) from an input image file.
///
/// Extraction strategy (best-effort, cascading):
/// 1. `img-parts` for JPEG — lossless byte-level copy
/// 2. `img-parts` for PNG — lossless byte-level copy
/// 3. `kamadak-exif` for TIFF-based raw files (behind `raw` feature)
/// 4. LibRaw parsed fields for non-TIFF raw files (behind `raw` feature)
/// 5. Return None — no metadata extracted
///
/// Returns `None` for unsupported formats or if the file can't be read.
/// This is best-effort — metadata extraction failure should never block processing.
pub fn extract_metadata(path: &std::path::Path) -> Option<ImageMetadata> {
    let bytes = std::fs::read(path).ok()?;

    // Strategy 1: Try img-parts for JPEG
    if let Some(meta) = extract_metadata_jpeg(&bytes) {
        return Some(meta);
    }

    // Strategy 2: Try img-parts for PNG
    if let Some(meta) = extract_metadata_png(&bytes) {
        return Some(meta);
    }

    // Strategy 3: Try kamadak-exif for TIFF-based raw files (CR2, NEF, DNG, ARW, PEF, ORF)
    #[cfg(feature = "raw")]
    {
        if crate::decode::is_raw_extension(path) {
            if let Some(meta) = extract_metadata_raw_tiff(path) {
                return Some(meta);
            }
        }
    }

    // Strategy 4: Try LibRaw parsed fields for non-TIFF raw files (RAF, RW2, CR3, etc.)
    #[cfg(feature = "raw")]
    {
        if crate::decode::is_raw_extension(path) {
            if let Some(exif_bytes) = crate::decode::raw::extract_raw_metadata(path) {
                return Some(ImageMetadata {
                    exif: Some(exif_bytes),
                    icc_profile: None,
                });
            }
        }
    }

    None
}

fn extract_metadata_jpeg(bytes: &[u8]) -> Option<ImageMetadata> {
    use img_parts::{ImageEXIF, ImageICC};

    let jpeg = img_parts::jpeg::Jpeg::from_bytes(bytes.to_vec().into()).ok()?;
    let exif = jpeg.exif().map(|b| b.to_vec());
    let icc = jpeg.icc_profile().map(|b| b.to_vec());
    if exif.is_some() || icc.is_some() {
        return Some(ImageMetadata {
            exif,
            icc_profile: icc,
        });
    }
    None
}

fn extract_metadata_png(bytes: &[u8]) -> Option<ImageMetadata> {
    use img_parts::{ImageEXIF, ImageICC};

    let png = img_parts::png::Png::from_bytes(bytes.to_vec().into()).ok()?;
    let exif = png.exif().map(|b| b.to_vec());
    let icc = png.icc_profile().map(|b| b.to_vec());
    if exif.is_some() || icc.is_some() {
        return Some(ImageMetadata {
            exif,
            icc_profile: icc,
        });
    }
    None
}

/// Extract EXIF from a TIFF-based raw file using kamadak-exif.
///
/// Works for: CR2, NEF, DNG, ARW, PEF, ORF (TIFF-container raw formats).
/// Returns raw EXIF bytes suitable for injection into output files.
#[cfg(feature = "raw")]
fn extract_metadata_raw_tiff(path: &std::path::Path) -> Option<ImageMetadata> {
    let file = std::fs::File::open(path).ok()?;
    let mut reader = std::io::BufReader::new(file);
    let exif = exif::Reader::new().read_from_container(&mut reader).ok()?;
    let raw_buf = exif.buf();
    if raw_buf.is_empty() {
        return None;
    }
    // kamadak-exif returns raw EXIF bytes (TIFF header + IFDs).
    // For injection into JPEG via img-parts, we need "Exif\0\0" prefix.
    let exif_bytes = if raw_buf.starts_with(b"Exif\0\0") {
        raw_buf.to_vec()
    } else {
        let mut prefixed = b"Exif\0\0".to_vec();
        prefixed.extend_from_slice(raw_buf);
        prefixed
    };
    Some(ImageMetadata {
        exif: Some(exif_bytes),
        icc_profile: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_metadata_nonexistent_file_returns_none() {
        let meta = extract_metadata(std::path::Path::new("/nonexistent/file.jpg"));
        assert!(meta.is_none());
    }

    #[test]
    fn extract_metadata_from_jpeg_with_no_exif() {
        let temp_path = std::env::temp_dir().join("oxiraw_meta_test_no_exif.jpg");
        let img: image::ImageBuffer<image::Rgb<u8>, Vec<u8>> =
            image::ImageBuffer::from_pixel(4, 4, image::Rgb([128u8, 128, 128]));
        img.save(&temp_path).unwrap();

        let meta = extract_metadata(&temp_path);
        if let Some(m) = meta {
            assert!(m.exif.is_none() || !m.exif.as_ref().unwrap().is_empty());
        }

        let _ = std::fs::remove_file(&temp_path);
    }

    #[test]
    fn extract_metadata_from_png() {
        let temp_path = std::env::temp_dir().join("oxiraw_meta_test.png");
        let img: image::ImageBuffer<image::Rgb<u8>, Vec<u8>> =
            image::ImageBuffer::from_pixel(4, 4, image::Rgb([128u8, 128, 128]));
        img.save(&temp_path).unwrap();

        // Should not crash
        let _meta = extract_metadata(&temp_path);
        let _ = std::fs::remove_file(&temp_path);
    }
}

#[cfg(all(test, feature = "raw"))]
mod raw_metadata_tests {
    use super::*;

    #[test]
    fn extract_metadata_raw_tiff_nonexistent_returns_none() {
        let meta = extract_metadata_raw_tiff(std::path::Path::new("/nonexistent/photo.cr2"));
        assert!(meta.is_none());
    }

    #[test]
    fn extract_metadata_raw_tiff_non_tiff_file_returns_none() {
        let temp_path = std::env::temp_dir().join("oxiraw_meta_not_tiff_raw.jpg");
        let img: image::ImageBuffer<image::Rgb<u8>, Vec<u8>> =
            image::ImageBuffer::from_pixel(4, 4, image::Rgb([128u8, 128, 128]));
        img.save(&temp_path).unwrap();

        let _meta = extract_metadata_raw_tiff(&temp_path);
        let _ = std::fs::remove_file(&temp_path);
    }

    #[test]
    fn extract_metadata_falls_through_to_none_for_unknown() {
        let temp_path = std::env::temp_dir().join("oxiraw_meta_unknown.bmp");
        let img: image::ImageBuffer<image::Rgb<u8>, Vec<u8>> =
            image::ImageBuffer::from_pixel(4, 4, image::Rgb([128u8, 128, 128]));
        img.save(&temp_path).unwrap();
        let meta = extract_metadata(&temp_path);
        assert!(meta.is_none());
        let _ = std::fs::remove_file(&temp_path);
    }
}
```

**Step 2: Modify `crates/oxiraw/src/decode/raw.rs`**

Change `extract_raw_metadata` to return `Option<Vec<u8>>` instead of `Option<ImageMetadata>`:

- Remove `use crate::encode::ImageMetadata;`
- Change `pub fn extract_raw_metadata(path: &Path) -> Option<ImageMetadata>` to `pub fn extract_raw_metadata(path: &Path) -> Option<Vec<u8>>`
- Change `construct_exif_from_fields` to return `Option<Vec<u8>>`
- In `construct_exif_from_fields`, change the final return from `Some(ImageMetadata { exif: Some(exif_bytes), icc_profile: None })` to `Some(exif_bytes)`

**Step 3: Modify `crates/oxiraw/src/encode/mod.rs`**

- Remove `ImageMetadata` struct definition
- Remove `extract_metadata`, `extract_metadata_jpeg`, `extract_metadata_png`, `extract_metadata_raw_tiff` functions
- Remove the `raw_metadata_tests` test module
- Remove tests that test extraction (keep tests that test encoding/injection)
- Change all references to `ImageMetadata` to `crate::metadata::ImageMetadata`
- The `inject_metadata` and `inject_metadata_tiff` functions stay, using `crate::metadata::ImageMetadata`
- `encode_to_file_with_options` parameter type changes to `Option<&crate::metadata::ImageMetadata>`
- Remove `use crate::decode::is_raw_extension` (no longer needed in encode)

**Step 4: Modify `crates/oxiraw/src/lib.rs`**

- Add `pub mod metadata;`
- Change `pub use encode::{EncodeOptions, ImageMetadata, OutputFormat};` to `pub use encode::{EncodeOptions, OutputFormat};`
- Add `pub use metadata::ImageMetadata;`

**Step 5: Modify `crates/oxiraw-cli/src/main.rs`**

- Change `oxiraw::encode::extract_metadata(input)` to `oxiraw::metadata::extract_metadata(input)` (two occurrences in `run_apply` and `run_edit`)

**Step 6: Run tests to verify**

Run: `cargo test -p oxiraw`

Expected: All tests pass. The metadata extraction tests now live in `metadata.rs`. The encode tests that tested injection still pass. No cross-module coupling between decode and encode.

Run: `cargo test -p oxiraw-cli`

Expected: CLI tests (if any) pass.

**Step 7: Commit**

```bash
git add crates/oxiraw/src/metadata.rs crates/oxiraw/src/encode/mod.rs \
    crates/oxiraw/src/decode/raw.rs crates/oxiraw/src/lib.rs \
    crates/oxiraw-cli/src/main.rs
git commit -m "refactor: extract metadata module to resolve decode↔encode coupling

Move ImageMetadata type and all extraction functions from encode to a new
metadata module. Change decode/raw.rs::extract_raw_metadata to return raw
EXIF bytes (Vec<u8>) instead of ImageMetadata, eliminating its dependency
on encode. Clean one-way dependency flow: decode ← metadata → encode."
```

---

### Task 2: Create the structural test

Now that the module layering is clean, build the test that enforces it.

**Files:**
- Create: `crates/oxiraw/tests/architecture.rs`

**Step 1: Write the structural test**

```rust
//! Structural tests enforcing module dependency layering.
//!
//! These tests scan source files for `use crate::` imports and assert that
//! no forbidden cross-module dependencies exist. See ARCHITECTURE.md for
//! the full dependency graph and rationale.
//!
//! If a test fails, read ARCHITECTURE.md "When a Structural Test Fails" before
//! making changes.

use std::fs;
use std::path::{Path, PathBuf};

/// Collect all `.rs` files under a directory, recursively.
fn collect_rs_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                files.extend(collect_rs_files(&path));
            } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                files.push(path);
            }
        }
    }
    files
}

/// Check that no `.rs` file in `module_dir` contains any of the forbidden import patterns.
/// Returns a list of violations as (file_path, line_number, line_content).
fn check_forbidden_imports(
    module_dir: &Path,
    forbidden_modules: &[&str],
) -> Vec<(PathBuf, usize, String)> {
    let mut violations = Vec::new();
    let files = collect_rs_files(module_dir);

    for file in files {
        let content = match fs::read_to_string(&file) {
            Ok(c) => c,
            Err(_) => continue,
        };
        for (line_num, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            // Skip comments
            if trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with('*')
            {
                continue;
            }
            for forbidden in forbidden_modules {
                let pattern = format!("crate::{forbidden}");
                if trimmed.contains(&pattern) {
                    violations.push((file.clone(), line_num + 1, trimmed.to_string()));
                }
            }
        }
    }
    violations
}

fn src_dir() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir.join("src")
}

fn format_violations(violations: &[(PathBuf, usize, String)]) -> String {
    violations
        .iter()
        .map(|(file, line, content)| format!("  {}:{}: {}", file.display(), line, content))
        .collect::<Vec<_>>()
        .join("\n")
}

/// adjust/ must not import from any other oxiraw module.
/// It is pure f32 math with no knowledge of images, files, or presets.
#[test]
fn adjust_has_no_forbidden_imports() {
    let violations = check_forbidden_imports(
        &src_dir().join("adjust"),
        &["engine", "decode", "encode", "preset", "lut", "metadata"],
    );
    assert!(
        violations.is_empty(),
        "adjust/ has forbidden imports:\n{}",
        format_violations(&violations)
    );
}

/// lut/ must not import from engine, decode, encode, preset, or metadata.
/// It is self-contained LUT parsing and interpolation.
#[test]
fn lut_has_no_forbidden_imports() {
    let violations = check_forbidden_imports(
        &src_dir().join("lut"),
        &["engine", "decode", "encode", "preset", "metadata"],
    );
    assert!(
        violations.is_empty(),
        "lut/ has forbidden imports:\n{}",
        format_violations(&violations)
    );
}

/// decode/ must not import from engine, encode, preset, adjust, lut, or metadata.
/// It only produces image buffers and exposes format detection utilities.
#[test]
fn decode_has_no_forbidden_imports() {
    let violations = check_forbidden_imports(
        &src_dir().join("decode"),
        &["engine", "encode", "preset", "adjust", "lut", "metadata"],
    );
    assert!(
        violations.is_empty(),
        "decode/ has forbidden imports:\n{}",
        format_violations(&violations)
    );
}

/// metadata.rs must not import from engine, preset, adjust, lut, or encode.
/// It may import from decode (for is_raw_extension and raw::extract_raw_metadata).
#[test]
fn metadata_has_no_forbidden_imports() {
    let metadata_path = src_dir().join("metadata.rs");
    let content = fs::read_to_string(&metadata_path).expect("metadata.rs should exist");
    let forbidden = &["engine", "preset", "adjust", "lut", "encode"];
    let mut violations = Vec::new();

    for (line_num, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with('*') {
            continue;
        }
        for f in forbidden {
            let pattern = format!("crate::{f}");
            if trimmed.contains(&pattern) {
                violations.push((metadata_path.clone(), line_num + 1, trimmed.to_string()));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "metadata.rs has forbidden imports:\n{}",
        format_violations(&violations)
    );
}

/// encode/ must not import from engine, preset, adjust, lut, or decode.
/// It may import from metadata (for ImageMetadata type).
#[test]
fn encode_has_no_forbidden_imports() {
    let violations = check_forbidden_imports(
        &src_dir().join("encode"),
        &["engine", "preset", "adjust", "lut", "decode"],
    );
    assert!(
        violations.is_empty(),
        "encode/ has forbidden imports:\n{}",
        format_violations(&violations)
    );
}

/// preset/ must not import from decode, encode, or metadata.
/// It may import from engine (for Parameters) and lut (for Lut3D).
#[test]
fn preset_has_no_forbidden_imports() {
    let violations = check_forbidden_imports(
        &src_dir().join("preset"),
        &["decode", "encode", "metadata"],
    );
    assert!(
        violations.is_empty(),
        "preset/ has forbidden imports:\n{}",
        format_violations(&violations)
    );
}
```

**Step 2: Run the test to verify it passes**

Run: `cargo test -p oxiraw --test architecture`

Expected: All 6 tests PASS against the refactored codebase.

**Step 3: Commit**

```bash
git add crates/oxiraw/tests/architecture.rs
git commit -m "test: add structural test for module dependency layering"
```

---

### Task 3: Create ARCHITECTURE.md

**Files:**
- Create: `ARCHITECTURE.md`

**Step 1: Write ARCHITECTURE.md**

Content should include:
- Module dependency graph (ASCII diagram from the design doc, updated for the metadata module)
- Dependency rules table (from the design doc)
- Negative constraints
- Core invariants (always-re-render-from-original, declarative presets, sRGB-only, fixed render order)
- Links to per-module READMEs
- Links to design docs in `docs/plans/`
- Structural test failure protocol (from the design doc)

Keep it concise. The design doc at `docs/plans/2026-03-04-harness-engineering-design.md` has all the content — adapt it for ARCHITECTURE.md format.

**Step 2: Commit**

```bash
git add ARCHITECTURE.md
git commit -m "docs: add ARCHITECTURE.md — module dependency map and layering rules"
```

---

### Task 4: Revise CLAUDE.md

**Files:**
- Modify: `CLAUDE.md`

**Step 1: Rewrite CLAUDE.md**

Slim it down. Key sections:
- Project purpose (1 sentence)
- Workspace layout (two crates)
- Architecture pointer → `ARCHITECTURE.md`
- Conventions (Rust 2021, thiserror, serde, test location, structural tests)
- Definition of Done checklist (from the design doc)
- Key docs links (ARCHITECTURE.md, docs/plans/, docs/ideas/, docs/contributing/)

**Step 2: Commit**

```bash
git add CLAUDE.md
git commit -m "docs: revise CLAUDE.md — thin entry point with Definition of Done"
```

---

### Task 5: Create per-module READMEs

**Files:**
- Create: `crates/oxiraw/src/adjust/README.md`
- Create: `crates/oxiraw/src/lut/README.md`
- Create: `crates/oxiraw/src/preset/README.md`
- Create: `crates/oxiraw/src/engine/README.md`
- Create: `crates/oxiraw/src/decode/README.md`
- Create: `crates/oxiraw/src/encode/README.md`
- Create: `crates/oxiraw/src/metadata/README.md` (placed alongside metadata.rs)
- Create: `crates/oxiraw-cli/README.md`

**Step 1: Write all 8 READMEs**

Each follows the template from the design doc: Purpose, Public API, Extension Guide, Does NOT, Key Decisions. Content for each module is derived from reading the current source code. Keep them short — a few paragraphs each.

Key content per module:

- **adjust**: Pure f32 math. Public functions: exposure_factor, apply_exposure, apply_white_balance, apply_contrast, apply_highlights, apply_shadows, apply_whites, apply_blacks, linear_to_srgb, srgb_to_linear. Extension: add function here, wire in engine, add to Parameters and preset.
- **lut**: Lut3D struct, from_cube_str/file, lookup. Extension: add new parser module for new formats.
- **preset**: Preset struct, from_toml/to_toml/load_from_file/save_to_file. Extension: add field to Parameters, add to ToneSection/WhiteBalanceSection, map in from_toml/to_toml.
- **engine**: Engine struct, new/render/params/set_params/apply_preset/lut/set_lut, Parameters struct. Extension: add field to Parameters, add adjustment call in render() at correct pipeline position.
- **decode**: decode(), decode_standard(), is_raw_extension(), raw::decode_raw(). Extension: standard formats handled by image crate automatically, raw formats by LibRaw.
- **encode**: encode_to_file(), encode_to_file_with_options(), resolve_output(), EncodeOptions, OutputFormat. Extension: add OutputFormat variant, add encoding branch, add metadata injection.
- **metadata**: ImageMetadata struct, extract_metadata(). Extension: add new extraction strategy.
- **oxiraw-cli**: apply and edit subcommands. Extension: add arg fields to Commands, pass through run_* functions.

**Step 2: Commit**

```bash
git add crates/oxiraw/src/adjust/README.md crates/oxiraw/src/lut/README.md \
    crates/oxiraw/src/preset/README.md crates/oxiraw/src/engine/README.md \
    crates/oxiraw/src/decode/README.md crates/oxiraw/src/encode/README.md \
    crates/oxiraw/src/metadata/README.md crates/oxiraw-cli/README.md
git commit -m "docs: add per-module READMEs with contracts and extension guides"
```

---

### Task 6: Create architecture evolution process doc

**Files:**
- Create: `docs/contributing/evolving-architecture.md`

**Step 1: Write the evolution process doc**

Content from the design doc:
- When to change (new feature needs forbidden dependency, module responsibility outgrown)
- When NOT to change (can restructure to avoid dependency, only needed in tests)
- Process: identify → design doc → approval → update ARCHITECTURE.md → update READMEs → update structural test → implement → verify
- Principles: prefer constraining over expanding, document coupling, small changes
- Agent guidance: read assertion, try restructuring first, surface conflict if needed

**Step 2: Commit**

```bash
git add docs/contributing/evolving-architecture.md
git commit -m "docs: add architecture evolution process"
```

---

### Task 7: Run all tests and verify

**Step 1: Run the full test suite**

Run: `cargo test -p oxiraw`

Expected: All existing tests PASS, plus the 6 new architecture tests PASS, plus the new metadata module tests PASS.

**Step 2: Run only the structural test**

Run: `cargo test -p oxiraw --test architecture`

Expected: 6 tests, all PASS.

**Step 3: Verify the documentation map is navigable**

Check that:
- `CLAUDE.md` links to `ARCHITECTURE.md`
- `ARCHITECTURE.md` links to all per-module READMEs
- `ARCHITECTURE.md` links to all design docs
- `ARCHITECTURE.md` links to `docs/contributing/evolving-architecture.md`
- Each module README follows the template consistently

**Step 4: Verify no decode↔encode coupling remains**

Run: `grep -r "crate::encode" crates/oxiraw/src/decode/`
Expected: No output (decode no longer imports from encode).

Run: `grep -r "crate::decode" crates/oxiraw/src/encode/`
Expected: No output (encode no longer imports from decode).
