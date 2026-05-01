//! Structural pass: parse TOML to a position-preserving representation and
//! detect top-level tables/fields that are not in the known preset schema.
//!
//! This pass handles ONLY top-level unknown tables/keys. Nested unknown fields
//! (inside a known table like `[hsl]`, `[grain]`, etc.) are detected by the
//! semantic pass via a custom JSON walk — see `semantic::find_unknown_fields`.
//!
//! This pass is shared between `agx validate` (where unknown fields are errors)
//! and `agx apply`'s warning path (where they're surfaced as warnings).

use super::diagnostic::{Diagnostic, DiagnosticCode, Location, Severity};
use std::collections::HashSet;

/// Detect unknown TOP-LEVEL tables and fields in a preset TOML source.
///
/// Returns one [`Diagnostic`] per unknown top-level table or field, with line
/// numbers derived from the source. Does NOT recurse into known tables — nested
/// unknown fields are the responsibility of `semantic::find_unknown_fields`.
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

/// Find the (line, column) for a dotted field path like "tone.exposure" or "lut.path".
///
/// Used by the semantic pass to enrich jsonschema errors with source positions.
pub(super) fn find_position_by_path(source: &str, path: &str) -> (usize, usize) {
    let parts: Vec<&str> = path.split('.').collect();
    match parts.as_slice() {
        [single] => find_key_position(source, single, None),
        [parent, child] => find_key_position(source, child, Some(parent)),
        _ => {
            // For depth ≥ 3, build the full dotted parent path so we match
            // headings like `[hsl.red]` or `[color_grading.shadows]` in the TOML
            // source. Using only the immediate parent segment (e.g. "red") would
            // never match `[hsl.red]` and cause the position to fall back to (1, 1).
            let full_parent = parts[..parts.len() - 1].join(".");
            let child = parts[parts.len() - 1];
            find_key_position(source, child, Some(&full_parent))
        }
    }
}

/// Compute (line, column) for a top-level key in the document.
fn position_for_key(doc: &toml_edit::DocumentMut, key: &str) -> (usize, usize) {
    let source = doc.to_string();
    find_key_position(&source, key, None)
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

        // Check the full dotted heading form `[parent.key]` BEFORE the parent-state
        // reset below. This handles cases like `[hsl.bogus]` (depth-2) and
        // `[hsl.red.weird_table]` (depth-3 with a dotted parent path), where the
        // line starts with `[` and would otherwise trigger the bail-out before the
        // heading match runs.
        if let Some(p) = parent {
            let dotted_heading = format!("[{}.{}]", p, key);
            let dotted_heading_with_dot = format!("[{}.{}.", p, key);
            let dotted_array = format!("[[{}.{}]]", p, key);
            if trimmed.starts_with(dotted_heading.as_str())
                || trimmed.starts_with(dotted_heading_with_dot.as_str())
                || trimmed.starts_with(dotted_array.as_str())
            {
                let column = line.len() - line.trim_start().len() + 1;
                return (line_num, column);
            }
        }

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
