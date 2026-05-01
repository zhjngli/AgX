//! Preset validation: strict, image-free correctness checks for preset files.
//!
//! See `docs/plans/2026-04-30-agx-validate-design.md` for design context.
//!
//! # Public API
//!
//! - [`Diagnostic`], [`DiagnosticCode`], [`Location`], [`Severity`] — diagnostic types
//! - [`ValidationReport`] — per-file or per-batch validation result
//!
//! Future tasks (not yet implemented) will add:
//! - `structural::detect_unknown_fields` — unknown-field detection with line numbers
//! - `semantic::check_schema` — type/required/range checks via jsonschema
//! - `filesystem::check_filesystem` — LUT existence and extends chain
//! - `Preset::validate` — public top-level API tying all three passes together

mod diagnostic;
pub(crate) mod structural;

pub use diagnostic::{
    Diagnostic, DiagnosticCode, FileReport, FileStatus, Location, Severity, Summary,
    ValidationReport,
};

#[cfg(test)]
mod tests;
