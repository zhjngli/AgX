//! Semantic pass: validate parsed TOML against the schemars-derived JSON Schema.
//!
//! Responsibilities:
//! - `check_schema`: type mismatches, missing required fields, and out-of-range
//!   numeric values via the jsonschema crate. Does NOT detect unknown fields —
//!   that avoids the cryptic "anyOf schema violation" messages produced when
//!   `additionalProperties: false` is combined with `Option<Struct>` fields.
//! - `find_unknown_fields`: walks the input JSON value against the schema's
//!   `properties` map at each nesting level, emitting clean "unknown field X in
//!   section \[Y\]" diagnostics for any key not in the schema. Bypasses jsonschema
//!   to avoid the anyOf complication described above.
//!
//! Top-level unknowns (unknown tables/keys at the root of the preset) are handled
//! exclusively by the structural pass (`structural::detect_unknown_fields`).

use super::diagnostic::{Diagnostic, DiagnosticCode, Location, Severity};
use crate::preset::PresetRaw;

/// Validate a TOML preset source against the preset JSON Schema.
///
/// Returns one [`Diagnostic`] per schema violation. Type mismatches, missing
/// required fields, and out-of-range numeric values are reported with code
/// [`DiagnosticCode::TypeMismatch`], [`DiagnosticCode::MissingRequired`], and
/// [`DiagnosticCode::OutOfRange`] respectively.
///
/// Does NOT detect unknown fields — use [`find_unknown_fields`] for that.
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
    // We do NOT inject `additionalProperties: false` — that causes cryptic
    // "anyOf schema violation" errors for Option<Struct> fields and duplicates
    // the unknown-field diagnostics that `find_unknown_fields` produces cleanly.
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

/// Walk the input TOML source against the schemars-derived JSON Schema and emit
/// a diagnostic for each unknown field or table at any nesting depth.
///
/// Top-level unknowns are skipped here (handled by the structural pass). Only
/// nested unknowns — fields inside a known table like `[grain]`, `[hsl]`, etc.
/// — are reported. This produces clean "unknown field X in section \[Y\]" messages
/// rather than the cryptic "anyOf schema violation" messages that
/// `additionalProperties: false` would cause for `Option<Struct>` fields.
pub fn find_unknown_fields(toml_str: &str) -> Vec<Diagnostic> {
    let toml_value: toml::Value = match toml::from_str(toml_str) {
        Ok(v) => v,
        Err(_) => return vec![],
    };
    let json_value = match toml_to_json(&toml_value) {
        Ok(v) => v,
        Err(_) => return vec![],
    };

    let schema = schemars::schema_for!(PresetRaw);
    let schema_json = serde_json::to_value(&schema).expect("schema serializes");

    let mut diagnostics = Vec::new();
    walk_unknown_fields(
        &json_value,
        &schema_json,
        &schema_json,
        "",
        toml_str,
        &mut diagnostics,
    );
    diagnostics
}

/// Recursively walk the JSON value against the schema, reporting unknown nested keys.
///
/// `path_prefix` tracks the dotted path to the current node (e.g. `"hsl"`,
/// `"color_grading.shadows"`). Top-level keys (`path_prefix.is_empty()`) are
/// silently skipped — the structural pass owns those.
fn walk_unknown_fields(
    value: &serde_json::Value,
    schema_node: &serde_json::Value,
    schema_root: &serde_json::Value,
    path_prefix: &str,
    toml_str: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Resolve $ref / anyOf wrappers down to a concrete object schema.
    let object_schema = match resolve_to_object_schema(schema_node, schema_root) {
        Some(s) => s,
        None => return,
    };

    let value_obj = match value.as_object() {
        Some(o) => o,
        None => return,
    };

    let properties = object_schema.get("properties").and_then(|p| p.as_object());

    for (field, field_value) in value_obj.iter() {
        let field_path = if path_prefix.is_empty() {
            field.clone()
        } else {
            format!("{}.{}", path_prefix, field)
        };

        let is_top_level = path_prefix.is_empty();

        match properties.and_then(|p| p.get(field)) {
            Some(field_schema) => {
                // Known field — recurse to detect deeper unknowns.
                walk_unknown_fields(
                    field_value,
                    field_schema,
                    schema_root,
                    &field_path,
                    toml_str,
                    diagnostics,
                );
            }
            None if !is_top_level => {
                // Unknown nested field — emit a diagnostic.
                let parent = path_prefix.split('.').next().unwrap_or("");
                let (line, column) =
                    super::structural::find_position_by_path(toml_str, &field_path);
                let is_table = field_value.is_object();
                let kind = if is_table { "table" } else { "field" };
                let code = if is_table {
                    DiagnosticCode::UnknownTable
                } else {
                    DiagnosticCode::UnknownField
                };
                diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    code,
                    message: format!("unknown {} `{}` in section `[{}]`", kind, field, parent),
                    location: Location {
                        line,
                        column,
                        field: field_path,
                    },
                });
            }
            None => {
                // Top-level unknown — handled by structural pass; skip.
            }
        }
    }
}

/// Resolve a schema node to its concrete object form by following `$ref` links,
/// unwrapping `anyOf: [null, struct]` (schemars representation of `Option<Struct>`),
/// and unwrapping `allOf: [{ "$ref": ... }]` (schemars representation of non-optional
/// struct fields with defaults).
fn resolve_to_object_schema<'a>(
    schema: &'a serde_json::Value,
    root: &'a serde_json::Value,
) -> Option<&'a serde_json::Map<String, serde_json::Value>> {
    let map = schema.as_object()?;

    // Follow $ref — e.g. "#/definitions/PartialHslChannels"
    if let Some(ref_val) = map.get("$ref").and_then(|r| r.as_str()) {
        if let Some(def_name) = ref_val.strip_prefix("#/definitions/") {
            let target = root.get("definitions").and_then(|d| d.get(def_name))?;
            return resolve_to_object_schema(target, root);
        }
        return None;
    }

    // Unwrap anyOf: [null, struct] — schemars representation of Option<Struct>
    if let Some(any_of) = map.get("anyOf").and_then(|a| a.as_array()) {
        for branch in any_of {
            let branch_map = match branch.as_object() {
                Some(m) => m,
                None => continue,
            };
            // Skip the null branch
            if branch_map.get("type").and_then(|t| t.as_str()) == Some("null") {
                continue;
            }
            return resolve_to_object_schema(branch, root);
        }
        return None;
    }

    // Unwrap allOf: [{ "$ref": "..." }] — schemars representation of non-optional struct
    // fields that have a default value (e.g. `lut: LutSection` with `#[serde(default)]`)
    if let Some(all_of) = map.get("allOf").and_then(|a| a.as_array()) {
        for branch in all_of {
            if let Some(resolved) = resolve_to_object_schema(branch, root) {
                return Some(resolved);
            }
        }
        return None;
    }

    // Direct object schema — has "type": "object" or "properties"
    if map.get("type").and_then(|t| t.as_str()) == Some("object") || map.contains_key("properties")
    {
        return Some(map);
    }

    None
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
