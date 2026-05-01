//! Preset validation: strict, image-free correctness checks for preset files.
//!
//! See `docs/plans/2026-04-30-agx-validate-design.md` for design context.
//!
//! # Public API
//!
//! - [`Diagnostic`], [`DiagnosticCode`], [`Location`], [`Severity`] — diagnostic types
//! - [`ValidationReport`] — per-file or per-batch validation result
//! - [`Preset::validate`] — run all three passes and return a [`FileReport`]
//! - [`detect_unknown_fields`] — structural pass, re-exported for the CLI apply path
//!
//! Implemented passes:
//! - `structural::detect_unknown_fields` — unknown-field detection with line numbers
//! - `semantic::check_schema` — type/required/range checks via jsonschema
//! - `filesystem::check_filesystem` — LUT existence and extends chain

mod diagnostic;
pub(crate) mod filesystem;
pub(crate) mod semantic;
pub(crate) mod structural;

pub use diagnostic::{
    Diagnostic, DiagnosticCode, FileReport, FileStatus, Location, Severity, Summary,
    ValidationReport,
};
pub use structural::detect_unknown_fields;

use crate::preset::Preset;
use std::path::Path;

impl Preset {
    /// Validate a preset file without rendering. Runs all three passes
    /// (structural, semantic, filesystem) and returns a per-file report.
    ///
    /// This API is image-free — no decode/render — and is intended for
    /// preset authors and preset-library CI. See
    /// `docs/plans/2026-04-30-agx-validate-design.md` for design context.
    pub fn validate(path: &Path) -> FileReport {
        let toml_str = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                return FileReport::new(
                    path.to_string_lossy(),
                    vec![Diagnostic {
                        severity: Severity::Error,
                        code: DiagnosticCode::FileNotReadable,
                        message: format!("failed to read file: {}", e),
                        location: Location {
                            line: 1,
                            column: 1,
                            field: String::new(),
                        },
                    }],
                );
            }
        };

        let mut diagnostics = Vec::new();
        diagnostics.extend(structural::detect_unknown_fields(&toml_str));
        diagnostics.extend(semantic::check_schema(&toml_str));
        diagnostics.extend(filesystem::check_filesystem(path));

        FileReport::new(path.to_string_lossy(), diagnostics)
    }
}

#[cfg(test)]
mod tests;
