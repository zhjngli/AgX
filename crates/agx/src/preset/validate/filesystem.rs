//! Filesystem pass: check LUT file existence and `extends` chain validity.
//!
//! Unlike the structural and semantic passes (which only need the TOML
//! source), this pass needs the file path to resolve relative paths.

use super::diagnostic::{Diagnostic, DiagnosticCode, Location, Severity};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Run filesystem checks on a preset file.
pub fn check_filesystem(preset_path: &Path) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    let toml_str = match std::fs::read_to_string(preset_path) {
        Ok(s) => s,
        Err(_) => return diagnostics,
    };

    let raw: crate::preset::PresetRaw = match toml::from_str(&toml_str) {
        Ok(r) => r,
        Err(_) => return diagnostics, // Parse errors handled elsewhere
    };

    let base_dir = preset_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();

    // LUT existence
    if let Some(lut_path) = raw.lut.path.as_ref() {
        let resolved = base_dir.join(lut_path);
        if !resolved.exists() {
            let (line, column) = super::structural::find_position_by_path(&toml_str, "lut.path");
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: DiagnosticCode::LutNotFound,
                message: format!(
                    "LUT file not found: `{}` (resolved to {})",
                    lut_path,
                    resolved.display()
                ),
                location: Location {
                    line,
                    column,
                    field: "lut.path".to_string(),
                },
            });
        }
    }

    // Extends chain
    diagnostics.extend(check_extends_chain(preset_path, &base_dir, &toml_str));

    diagnostics
}

fn check_extends_chain(start_path: &Path, start_dir: &Path, start_toml: &str) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let mut visited: HashSet<PathBuf> = HashSet::new();
    let canonical = start_path
        .canonicalize()
        .unwrap_or_else(|_| start_path.to_path_buf());
    visited.insert(canonical);

    let mut current_dir = start_dir.to_path_buf();
    let mut current_toml = start_toml.to_string();

    while let Ok(raw) = toml::from_str::<crate::preset::PresetRaw>(&current_toml) {
        let extends = match raw.metadata.extends.as_ref() {
            Some(e) => e.clone(),
            None => break, // End of chain
        };

        let next_path = current_dir.join(&extends);
        if !next_path.exists() {
            let (line, column) =
                super::structural::find_position_by_path(&current_toml, "metadata.extends");
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: DiagnosticCode::ExtendsNotFound,
                message: format!(
                    "extends references missing file: `{}` (resolved to {})",
                    extends,
                    next_path.display()
                ),
                location: Location {
                    line,
                    column,
                    field: "metadata.extends".to_string(),
                },
            });
            break;
        }

        let canonical = next_path
            .canonicalize()
            .unwrap_or_else(|_| next_path.clone());
        if !visited.insert(canonical) {
            let (line, column) =
                super::structural::find_position_by_path(&current_toml, "metadata.extends");
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: DiagnosticCode::ExtendsCycle,
                message: format!(
                    "extends chain has a cycle: `{}` already visited",
                    next_path.display()
                ),
                location: Location {
                    line,
                    column,
                    field: "metadata.extends".to_string(),
                },
            });
            break;
        }

        // Move to the next file in the chain
        current_toml = match std::fs::read_to_string(&next_path) {
            Ok(s) => s,
            Err(_) => break,
        };
        current_dir = next_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
    }

    diagnostics
}
