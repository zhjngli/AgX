//! Semantic pass: validate parsed TOML against the schemars-derived JSON Schema.
//!
//! Catches type mismatches, missing required fields, and out-of-range numeric
//! values. Each diagnostic is enriched with a line number from the structural
//! pass via the field path.

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

    // Generate the JSON Schema from PresetRaw via schemars.
    // Inject `additionalProperties: false` into every object sub-schema so that
    // unknown nested fields are caught by the semantic pass (without this, the
    // schemars-derived schema permits arbitrary extra properties by default).
    let schema = schemars::schema_for!(PresetRaw);
    let mut schema_json =
        serde_json::to_value(&schema).expect("schemars schema is always serializable");
    inject_additional_properties_false(&mut schema_json);

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

/// Recursively walk a JSON Schema value and inject `additionalProperties: false`
/// into every object schema. Without this, schemars-derived schemas allow
/// arbitrary extra fields, so unknown nested fields would pass validation.
fn inject_additional_properties_false(schema: &mut serde_json::Value) {
    match schema {
        serde_json::Value::Object(map) => {
            // If this looks like an object schema, set additionalProperties: false
            let is_object = map.get("type").and_then(|v| v.as_str()) == Some("object")
                || map.contains_key("properties");
            if is_object && !map.contains_key("additionalProperties") {
                map.insert(
                    "additionalProperties".to_string(),
                    serde_json::Value::Bool(false),
                );
            }
            // Recurse into all values
            for v in map.values_mut() {
                inject_additional_properties_false(v);
            }
        }
        serde_json::Value::Array(arr) => {
            for v in arr.iter_mut() {
                inject_additional_properties_false(v);
            }
        }
        _ => {}
    }
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
        ValidationErrorKind::AdditionalProperties { .. } => (
            DiagnosticCode::UnknownField,
            format!(
                "unknown field at `{}`: not in the preset schema",
                error.instance_path.as_str()
            ),
        ),
        // TODO: jsonschema's ValidationErrorKind has additional variants
        // (Enum, Pattern, UniqueItems, etc.) that don't currently map cleanly to
        // one of our DiagnosticCodes. They fall back to TypeMismatch, which is
        // semantically imprecise but honest in the message body. Add finer-grained
        // codes when these constraint kinds become relevant to the preset schema.
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
