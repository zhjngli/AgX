//! Structural pass: parse TOML to a position-preserving representation and
//! detect fields/tables that are not in the known preset schema.
//!
//! This pass is shared between `agx validate` (where unknown fields are errors)
//! and `agx apply`'s warning path (where they're surfaced as warnings).

// The public function and helpers are used via the test module now; future tasks
// will wire them into the validate/apply command paths.
#![allow(dead_code)]

use super::diagnostic::{Diagnostic, DiagnosticCode, Location, Severity};
use std::collections::HashSet;

/// Detect unknown fields and tables in a preset TOML source.
///
/// Returns one [`Diagnostic`] per unknown table or field, with line numbers
/// derived from the source.
///
/// # Severity
///
/// All returned diagnostics use [`Severity::Error`]. The caller decides whether
/// to surface them as errors (validate path) or warnings (apply path).
pub fn detect_unknown_fields(toml_str: &str) -> Vec<Diagnostic> {
    let doc: toml_edit::DocumentMut = match toml_str.parse() {
        Ok(d) => d,
        // If TOML doesn't parse at all, the structural pass produces no diagnostics —
        // the caller's serde parse will surface a TypeMismatch via the semantic pass.
        Err(_) => return vec![],
    };

    let known_top_level = known_top_level_tables();
    let mut diagnostics = Vec::new();

    for (key, item) in doc.iter() {
        if !known_top_level.contains(key) {
            // Unknown top-level table or field
            let (line, column) = position_for_key(&doc, key);
            let code = if item.is_table() || item.is_inline_table() || item.is_array_of_tables() {
                DiagnosticCode::UnknownTable
            } else {
                DiagnosticCode::UnknownField
            };
            let kind = if item.is_table() || item.is_array_of_tables() {
                "table"
            } else {
                "field"
            };
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code,
                message: format!("unknown {} `{}`", kind, key),
                location: Location {
                    line,
                    column,
                    field: key.to_string(),
                },
            });
            continue;
        }

        // Known top-level table — recurse to check its fields
        if let Some(table) = item.as_table() {
            let known_fields = known_fields_for(key);
            for (sub_key, sub_item) in table.iter() {
                if !known_fields.contains(sub_key) {
                    let (line, column) = position_for_subkey(&doc, key, sub_key);
                    let code = if sub_item.is_table()
                        || sub_item.is_inline_table()
                        || sub_item.is_array_of_tables()
                    {
                        DiagnosticCode::UnknownTable
                    } else {
                        DiagnosticCode::UnknownField
                    };
                    let kind = if sub_item.is_table() || sub_item.is_array_of_tables() {
                        "table"
                    } else {
                        "field"
                    };
                    diagnostics.push(Diagnostic {
                        severity: Severity::Error,
                        code,
                        message: format!("unknown {} `{}` in section `[{}]`", kind, sub_key, key),
                        location: Location {
                            line,
                            column,
                            field: format!("{}.{}", key, sub_key),
                        },
                    });
                }
            }
        }
    }

    diagnostics
}

/// The set of top-level tables/keys recognized by the preset schema.
fn known_top_level_tables() -> HashSet<&'static str> {
    [
        "metadata",
        "tone",
        "white_balance",
        "lut",
        "hsl",
        "vignette",
        "color_grading",
        "tone_curve",
        "detail",
        "dehaze",
        "noise_reduction",
        "grain",
    ]
    .into_iter()
    .collect()
}

/// The set of known fields for a given top-level table.
fn known_fields_for(table: &str) -> HashSet<&'static str> {
    let fields: &[&str] = match table {
        "metadata" => &["name", "version", "author", "extends"],
        "tone" => &[
            "exposure",
            "contrast",
            "highlights",
            "shadows",
            "whites",
            "blacks",
        ],
        "white_balance" => &["temperature", "tint"],
        "lut" => &["path"],
        // Other tables: their fields come from the engine structs (HslChannels,
        // VignetteParams, etc.). For sub-table awareness, the semantic pass
        // (Task 5) catches these via the JSON Schema. The structural pass only
        // covers the flat fields directly on PresetRaw.
        _ => &[],
    };
    fields.iter().copied().collect()
}

/// Compute (line, column) for a top-level key in the document.
fn position_for_key(doc: &toml_edit::DocumentMut, key: &str) -> (usize, usize) {
    let source = doc.to_string();
    find_key_position(&source, key, None)
}

/// Compute (line, column) for a sub-key within a parent table.
fn position_for_subkey(doc: &toml_edit::DocumentMut, parent: &str, key: &str) -> (usize, usize) {
    let source = doc.to_string();
    find_key_position(&source, key, Some(parent))
}

/// Scan the source for a key, optionally constrained to be after a parent
/// `[parent]` heading. Returns 1-based line and column of the first match.
///
/// This is a pragmatic implementation; toml_edit does not expose source spans
/// in its public API (as of 0.22). If a future version exposes spans, replace
/// this scan with the official API.
fn find_key_position(source: &str, key: &str, parent: Option<&str>) -> (usize, usize) {
    let mut in_parent = parent.is_none();
    let parent_heading = parent.map(|p| format!("[{}]", p));

    for (idx, line) in source.lines().enumerate() {
        let line_num = idx + 1;
        let trimmed = line.trim_start();

        if let Some(ref heading) = parent_heading {
            if trimmed.starts_with(heading.as_str()) {
                in_parent = true;
                continue;
            }
            if trimmed.starts_with('[') && in_parent {
                // We left the parent table without finding the key — shouldn't happen
                // if the doc parsed correctly, but bail out at file end with a fallback.
                in_parent = false;
            }
        }

        if !in_parent {
            continue;
        }

        // Match either `[key]` (table heading) or `key = ...` (field)
        let heading_match = trimmed.starts_with(&format!("[{}]", key))
            || trimmed.starts_with(&format!("[{}.", key))
            || trimmed.starts_with(&format!("[[{}]]", key));
        let field_match =
            trimmed.starts_with(&format!("{} =", key)) || trimmed.starts_with(&format!("{}=", key));

        if heading_match || field_match {
            let column = line.len() - line.trim_start().len() + 1;
            return (line_num, column);
        }
    }
    // Fallback: line 1 if we somehow can't find it
    (1, 1)
}
