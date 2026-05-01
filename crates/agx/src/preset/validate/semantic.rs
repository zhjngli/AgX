//! Semantic pass: validate parsed TOML against the schemars-derived JSON Schema.
//!
//! Catches type mismatches, missing required fields, and out-of-range numeric
//! values. Each diagnostic is enriched with a line number from the structural
//! pass via the field path.

// The public function and helpers are used via the test module now; future tasks
// will wire them into the validate/apply command paths.
#![allow(dead_code)]

use super::diagnostic::{Diagnostic, DiagnosticCode, Location, Severity};
use crate::preset::PresetRaw;

/// Validate a TOML preset source against the preset JSON Schema.
///
/// Returns one [`Diagnostic`] per schema violation. Type mismatches, missing
/// required fields, and out-of-range numeric values are reported with code
/// [`DiagnosticCode::TypeMismatch`], [`DiagnosticCode::MissingRequired`], and
/// [`DiagnosticCode::OutOfRange`] respectively.
pub fn check_schema(toml_str: &str) -> Vec<Diagnostic> {
    // Parse TOML to serde_json::Value (jsonschema validates against this type)
    let toml_value: toml::Value = match toml::from_str(toml_str) {
        Ok(v) => v,
        Err(_) => return vec![], // Parse failure surfaces elsewhere
    };
    let json_value = match toml_to_json(&toml_value) {
        Ok(v) => v,
        Err(_) => return vec![],
    };

    // Generate the JSON Schema from PresetRaw via schemars
    let schema = schemars::schema_for!(PresetRaw);
    let schema_json =
        serde_json::to_value(&schema).expect("schemars schema is always serializable");

    // Validate using the jsonschema crate
    let validator = match jsonschema::validator_for(&schema_json) {
        Ok(v) => v,
        Err(_) => return vec![],
    };

    let mut diagnostics = Vec::new();
    for error in validator.iter_errors(&json_value) {
        let (code, message) = classify_error(&error);
        let field = error
            .instance_path
            .as_str()
            .trim_start_matches('/')
            .replace('/', ".");

        // Line number is enriched via the structural pass helper.
        let (line, column) = super::structural::find_position_by_path(toml_str, &field);

        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code,
            message,
            location: Location {
                line,
                column,
                field,
            },
        });
    }
    diagnostics
}

/// Convert a `toml::Value` to a `serde_json::Value` for jsonschema consumption.
fn toml_to_json(value: &toml::Value) -> Result<serde_json::Value, ()> {
    let json_str = serde_json::to_string(value).map_err(|_| ())?;
    serde_json::from_str(&json_str).map_err(|_| ())
}

/// Map a `jsonschema::ValidationError` to our `DiagnosticCode` and a
/// human-readable message.
fn classify_error(error: &jsonschema::ValidationError) -> (DiagnosticCode, String) {
    use jsonschema::error::ValidationErrorKind;
    match &error.kind {
        ValidationErrorKind::Type { .. } => (
            DiagnosticCode::TypeMismatch,
            format!(
                "type mismatch at `{}`: {}",
                error.instance_path.as_str(),
                error
            ),
        ),
        ValidationErrorKind::Required { .. } => (
            DiagnosticCode::MissingRequired,
            format!(
                "missing required field at `{}`: {}",
                error.instance_path.as_str(),
                error
            ),
        ),
        ValidationErrorKind::Maximum { limit } | ValidationErrorKind::Minimum { limit } => (
            DiagnosticCode::OutOfRange,
            format!(
                "`{}` value {} outside allowed range (limit: {})",
                error.instance_path.as_str(),
                error.instance,
                limit
            ),
        ),
        ValidationErrorKind::ExclusiveMaximum { limit }
        | ValidationErrorKind::ExclusiveMinimum { limit } => (
            DiagnosticCode::OutOfRange,
            format!(
                "`{}` value {} outside allowed range (exclusive limit: {})",
                error.instance_path.as_str(),
                error.instance,
                limit
            ),
        ),
        // TODO: jsonschema's ValidationErrorKind has additional variants
        // (Enum, Pattern, UniqueItems, AdditionalProperties, etc.) that don't
        // currently map cleanly to one of our DiagnosticCodes. They fall back to
        // TypeMismatch, which is semantically imprecise but honest in the message
        // body. Add finer-grained codes when these constraint kinds become relevant
        // to the preset schema.
        _ => (
            DiagnosticCode::TypeMismatch,
            format!(
                "schema violation at `{}`: {}",
                error.instance_path.as_str(),
                error
            ),
        ),
    }
}
