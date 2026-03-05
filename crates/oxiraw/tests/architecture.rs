//! Structural tests that enforce module layering rules.
//!
//! These tests scan Rust source files for `use crate::` imports and verify that
//! no module imports from a forbidden peer module. This prevents architectural
//! regressions such as circular dependencies or upward dependencies from
//! low-level modules into higher-level ones.

use std::fs;
use std::path::{Path, PathBuf};

/// Recursively collect all `.rs` files under `dir`.
fn collect_rs_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if !dir.is_dir() {
        return files;
    }
    for entry in fs::read_dir(dir).expect("failed to read directory") {
        let entry = entry.expect("failed to read directory entry");
        let path = entry.path();
        if path.is_dir() {
            files.extend(collect_rs_files(&path));
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            files.push(path);
        }
    }
    files
}

/// Represents a single violation: a forbidden import found in a source file.
struct Violation {
    file: PathBuf,
    line_number: usize,
    line: String,
}

impl std::fmt::Display for Violation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "  {}:{}: {}",
            self.file.display(),
            self.line_number,
            self.line.trim()
        )
    }
}

/// Returns true if a trimmed line is inside a comment (line comment or block comment prefix).
fn is_comment_line(trimmed: &str) -> bool {
    trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with('*')
}

/// Scan all `.rs` files in `dir` for forbidden `use crate::{module}` imports.
///
/// Returns a list of violations. Each forbidden module is checked as a
/// `use crate::{module}` prefix (catching both `use crate::module;` and
/// `use crate::module::Something`).
fn check_forbidden_imports(files: &[PathBuf], forbidden_modules: &[&str]) -> Vec<Violation> {
    let mut violations = Vec::new();

    // Build the patterns we look for, e.g. "use crate::engine"
    let patterns: Vec<String> = forbidden_modules
        .iter()
        .map(|m| format!("use crate::{m}"))
        .collect();

    for file in files {
        let contents = fs::read_to_string(file).expect("failed to read source file");
        for (i, line) in contents.lines().enumerate() {
            let trimmed = line.trim();

            // Skip comment lines
            if is_comment_line(trimmed) {
                continue;
            }

            // Check each forbidden pattern
            for pattern in &patterns {
                if trimmed.contains(pattern.as_str()) {
                    violations.push(Violation {
                        file: file.clone(),
                        line_number: i + 1,
                        line: line.to_string(),
                    });
                }
            }
        }
    }

    violations
}

/// Format violations into a clear assertion message.
fn format_violations(module_name: &str, forbidden: &[&str], violations: &[Violation]) -> String {
    let mut msg = format!(
        "\n`{module_name}` module has forbidden imports (must not import from: {forbidden:?}):\n"
    );
    for v in violations {
        msg.push_str(&format!("{v}\n"));
    }
    msg
}

/// Return the path to the `src/` directory of the oxiraw crate.
fn src_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src")
}

// ---------------------------------------------------------------------------
// Per-module tests
// ---------------------------------------------------------------------------

/// The `adjust` module contains pure image adjustment functions. It must not
/// depend on engine (which orchestrates adjustments), decode/encode (I/O),
/// preset (serialization), lut (LUT application), or metadata (EXIF handling).
#[test]
fn adjust_must_not_import_engine_decode_encode_preset_lut_metadata() {
    let dir = src_dir().join("adjust");
    let files = collect_rs_files(&dir);
    let forbidden = &["engine", "decode", "encode", "preset", "lut", "metadata"];
    let violations = check_forbidden_imports(&files, forbidden);
    assert!(
        violations.is_empty(),
        "{}",
        format_violations("adjust", forbidden, &violations)
    );
}

/// The `lut` module handles LUT loading and application. It must not depend on
/// engine, decode/encode, preset, or metadata — it is a leaf utility module.
#[test]
fn lut_must_not_import_engine_decode_encode_preset_metadata() {
    let dir = src_dir().join("lut");
    let files = collect_rs_files(&dir);
    let forbidden = &["engine", "decode", "encode", "preset", "metadata"];
    let violations = check_forbidden_imports(&files, forbidden);
    assert!(
        violations.is_empty(),
        "{}",
        format_violations("lut", forbidden, &violations)
    );
}

/// The `decode` module reads image files into in-memory buffers. It must not
/// depend on engine, encode, preset, adjust, lut, or metadata — it only
/// produces raw pixel data and uses `error` for error types.
#[test]
fn decode_must_not_import_engine_encode_preset_adjust_lut_metadata() {
    let dir = src_dir().join("decode");
    let files = collect_rs_files(&dir);
    let forbidden = &["engine", "encode", "preset", "adjust", "lut", "metadata"];
    let violations = check_forbidden_imports(&files, forbidden);
    assert!(
        violations.is_empty(),
        "{}",
        format_violations("decode", forbidden, &violations)
    );
}

/// The `metadata` module (a single file, not a directory) handles EXIF and
/// image metadata. It must not depend on engine, preset, adjust, lut, or
/// encode — it is a standalone data-structure module.
#[test]
fn metadata_must_not_import_engine_preset_adjust_lut_encode() {
    let file = src_dir().join("metadata.rs");
    assert!(file.exists(), "metadata.rs not found at {}", file.display());
    let files = vec![file];
    let forbidden = &["engine", "preset", "adjust", "lut", "encode"];
    let violations = check_forbidden_imports(&files, forbidden);
    assert!(
        violations.is_empty(),
        "{}",
        format_violations("metadata", forbidden, &violations)
    );
}

/// The `encode` module writes in-memory images to output formats. It must not
/// depend on engine, preset, adjust, lut, or decode — keeping encode and
/// decode independent prevents circular I/O coupling.
#[test]
fn encode_must_not_import_engine_preset_adjust_lut_decode() {
    let dir = src_dir().join("encode");
    let files = collect_rs_files(&dir);
    let forbidden = &["engine", "preset", "adjust", "lut", "decode"];
    let violations = check_forbidden_imports(&files, forbidden);
    assert!(
        violations.is_empty(),
        "{}",
        format_violations("encode", forbidden, &violations)
    );
}

/// The `preset` module handles serialization and deserialization of parameter
/// presets. It may depend on `engine` (for `Parameters`) but must not depend on
/// decode, encode, or metadata — presets are a pure data-mapping layer.
#[test]
fn preset_must_not_import_decode_encode_metadata() {
    let dir = src_dir().join("preset");
    let files = collect_rs_files(&dir);
    let forbidden = &["decode", "encode", "metadata"];
    let violations = check_forbidden_imports(&files, forbidden);
    assert!(
        violations.is_empty(),
        "{}",
        format_violations("preset", forbidden, &violations)
    );
}
