//! Preset validation: strict, image-free correctness checks for preset files.
//!
//! See `docs/plans/2026-04-30-agx-validate-design.md` for design context.
//!
//! # Public API
//!
//! - [`Diagnostic`], [`DiagnosticCode`], [`Location`], [`Severity`] — diagnostic types
//! - [`ValidationReport`] — per-file or per-batch validation result
//! - [`Preset::validate`] — run all three passes and return a [`FileReport`]
//! - [`detect_unknown_fields`] — structural pass (top-level only), re-exported for the CLI apply path
//! - [`find_unknown_fields`] — semantic pass (nested unknowns), re-exported for the CLI apply path
//!
//! Pass responsibilities (non-overlapping):
//! - `structural::detect_unknown_fields` — unknown TOP-LEVEL tables/keys only, with line numbers
//! - `semantic::find_unknown_fields` — unknown NESTED fields (inside known tables), via custom walk
//! - `semantic::check_schema` — type/required/range checks via jsonschema (no unknown-field detection)
//! - `filesystem::check_filesystem` — LUT existence and extends chain

mod diagnostic;
pub(crate) mod filesystem;
pub(crate) mod semantic;
pub(crate) mod structural;

pub use diagnostic::{
    Diagnostic, DiagnosticCode, FileReport, FileStatus, Location, Severity, Summary,
    ValidationReport,
};
pub use semantic::find_unknown_fields;
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

        // Early TOML parse check — short-circuit on syntax errors so they don't
        // silently pass through (each pass returns vec![] on parse failure).
        if let Err(parse_err) = toml::from_str::<toml::Value>(&toml_str) {
            let (line, column) = parse_err
                .span()
                .map(|s| {
                    // toml::de::Error spans are byte ranges; convert to 1-based line/col.
                    let prefix = &toml_str[..s.start];
                    let line = prefix.matches('\n').count() + 1;
                    let last_newline = prefix.rfind('\n').map(|i| i + 1).unwrap_or(0);
                    let column = s.start - last_newline + 1;
                    (line, column)
                })
                .unwrap_or((1, 1));

            return FileReport::new(
                path.to_string_lossy(),
                vec![Diagnostic {
                    severity: Severity::Error,
                    code: DiagnosticCode::SyntaxError,
                    message: format!("TOML syntax error: {}", parse_err.message()),
                    location: Location {
                        line,
                        column,
                        field: String::new(),
                    },
                }],
            );
        }

        let mut diagnostics = Vec::new();
        diagnostics.extend(structural::detect_unknown_fields(&toml_str));
        diagnostics.extend(semantic::check_schema(&toml_str));
        diagnostics.extend(semantic::find_unknown_fields(&toml_str));
        diagnostics.extend(filesystem::check_filesystem(path));

        FileReport::new(path.to_string_lossy(), diagnostics)
    }
}

#[cfg(test)]
mod tests;
