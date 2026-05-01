//! Tests for the diagnostic types — locks the JSON serialization shape that
//! `agx validate --format=json` consumers depend on.

use super::*;
use serde_json::json;

#[test]
fn diagnostic_serializes_to_documented_shape() {
    let diag = Diagnostic {
        severity: Severity::Error,
        code: DiagnosticCode::UnknownTable,
        message: "unknown table `tone_curves` (did you mean `tone_curve`?)".to_string(),
        location: Location {
            line: 12,
            column: 1,
            field: "tone_curves".to_string(),
        },
    };

    let actual = serde_json::to_value(&diag).unwrap();
    let expected = json!({
        "severity": "error",
        "code": "unknown-table",
        "message": "unknown table `tone_curves` (did you mean `tone_curve`?)",
        "location": {
            "line": 12,
            "column": 1,
            "field": "tone_curves"
        }
    });
    assert_eq!(actual, expected);
}

#[test]
fn validation_report_serializes_to_documented_shape() {
    let report = ValidationReport::from_files(vec![
        FileReport::new(
            "looks/broken.toml",
            vec![Diagnostic {
                severity: Severity::Error,
                code: DiagnosticCode::OutOfRange,
                message: "`tone.exposure` value 99.0 outside range [-5.0, 5.0]".to_string(),
                location: Location {
                    line: 5,
                    column: 1,
                    field: "tone.exposure".to_string(),
                },
            }],
        ),
        FileReport::new("looks/clean.toml", vec![]),
    ]);

    let actual = serde_json::to_value(&report).unwrap();
    let expected = json!({
        "files": [
            {
                "path": "looks/broken.toml",
                "status": "error",
                "diagnostics": [{
                    "severity": "error",
                    "code": "out-of-range",
                    "message": "`tone.exposure` value 99.0 outside range [-5.0, 5.0]",
                    "location": {"line": 5, "column": 1, "field": "tone.exposure"}
                }]
            },
            {
                "path": "looks/clean.toml",
                "status": "ok",
                "diagnostics": []
            }
        ],
        "summary": {"total": 2, "ok": 1, "errors": 1}
    });
    assert_eq!(actual, expected);
}

#[test]
fn file_report_status_derived_from_diagnostics() {
    let with_error = FileReport::new(
        "x.toml",
        vec![Diagnostic {
            severity: Severity::Error,
            code: DiagnosticCode::UnknownField,
            message: "x".to_string(),
            location: Location {
                line: 1,
                column: 1,
                field: "x".to_string(),
            },
        }],
    );
    assert_eq!(with_error.status, FileStatus::Error);

    let with_warning_only = FileReport::new(
        "y.toml",
        vec![Diagnostic {
            severity: Severity::Warning,
            code: DiagnosticCode::UnknownField,
            message: "y".to_string(),
            location: Location {
                line: 1,
                column: 1,
                field: "y".to_string(),
            },
        }],
    );
    assert_eq!(with_warning_only.status, FileStatus::Ok);

    let empty = FileReport::new("z.toml", vec![]);
    assert_eq!(empty.status, FileStatus::Ok);
}

#[test]
fn report_has_errors_reflects_summary() {
    let with_error = ValidationReport::from_files(vec![FileReport::new(
        "x.toml",
        vec![Diagnostic {
            severity: Severity::Error,
            code: DiagnosticCode::UnknownField,
            message: "x".to_string(),
            location: Location {
                line: 1,
                column: 1,
                field: "x".to_string(),
            },
        }],
    )]);
    assert!(with_error.has_errors());

    let clean = ValidationReport::from_files(vec![FileReport::new("x.toml", vec![])]);
    assert!(!clean.has_errors());
}

mod semantic_pass {
    use super::super::semantic::check_schema;
    use super::super::*;
    use std::path::Path;

    fn fixture(name: &str) -> String {
        let path = Path::new("src/preset/validate/tests/fixtures").join(name);
        std::fs::read_to_string(&path).unwrap()
    }

    #[test]
    fn clean_preset_passes_semantic_check() {
        let toml_str = fixture("clean.toml");
        let diags = check_schema(&toml_str);
        assert_eq!(diags, vec![]);
    }

    #[test]
    fn type_mismatch_is_detected() {
        let toml_str = fixture("type_mismatch.toml");
        let diags = check_schema(&toml_str);

        assert!(!diags.is_empty(), "expected at least one diagnostic");
        let type_errors: Vec<_> = diags
            .iter()
            .filter(|d| d.code == DiagnosticCode::TypeMismatch)
            .collect();
        assert_eq!(type_errors.len(), 1);
        assert_eq!(type_errors[0].location.field, "tone.exposure");
    }

    #[test]
    fn out_of_range_is_detected() {
        let toml_str = fixture("out_of_range.toml");
        let diags = check_schema(&toml_str);

        let range_errors: Vec<_> = diags
            .iter()
            .filter(|d| d.code == DiagnosticCode::OutOfRange)
            .collect();
        assert_eq!(
            range_errors.len(),
            1,
            "expected exactly one out-of-range diagnostic"
        );
        assert_eq!(range_errors[0].location.field, "tone.exposure");
        assert!(
            range_errors[0].message.contains("99"),
            "message should mention the offending value, got: {}",
            range_errors[0].message,
        );
    }
}

mod structural_pass {
    use super::super::structural::detect_unknown_fields;
    use super::super::*;
    use std::path::Path;

    fn fixture(name: &str) -> String {
        let path = Path::new("src/preset/validate/tests/fixtures").join(name);
        std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("failed to read fixture {:?}: {}", path, e))
    }

    #[test]
    fn clean_preset_has_no_unknown_fields() {
        let toml_str = fixture("clean.toml");
        let diags = detect_unknown_fields(&toml_str);
        assert_eq!(
            diags,
            vec![],
            "clean preset should have no unknown-field diagnostics"
        );
    }

    #[test]
    fn unknown_table_is_detected_with_line_number() {
        let toml_str = fixture("unknown_table.toml");
        let diags = detect_unknown_fields(&toml_str);

        assert_eq!(diags.len(), 1, "expected exactly one diagnostic");
        let diag = &diags[0];
        assert_eq!(diag.code, DiagnosticCode::UnknownTable);
        assert_eq!(diag.severity, Severity::Error);
        assert_eq!(diag.location.field, "tone_curves");
        assert!(
            diag.message.contains("tone_curves"),
            "message should reference the unknown table name, got: {}",
            diag.message,
        );
        // The fixture has `[tone_curves]` on line 7
        assert_eq!(
            diag.location.line, 7,
            "line number should point at the [tone_curves] heading"
        );
    }

    #[test]
    fn unknown_field_is_detected_with_line_number_and_path() {
        let toml_str = fixture("unknown_field.toml");
        let diags = detect_unknown_fields(&toml_str);

        assert_eq!(diags.len(), 1);
        let diag = &diags[0];
        assert_eq!(diag.code, DiagnosticCode::UnknownField);
        assert_eq!(diag.location.field, "lut.amount");
        // The fixture has `amount = 0.8` on line 6
        assert_eq!(diag.location.line, 6);
    }

    #[test]
    fn unknown_array_of_tables_is_classified_as_table_with_line_number() {
        let toml_str = fixture("unknown_array_of_tables.toml");
        let diags = detect_unknown_fields(&toml_str);

        assert_eq!(
            diags.len(),
            1,
            "expected exactly one diagnostic for the unknown [[arr]]"
        );
        let diag = &diags[0];
        assert_eq!(
            diag.code,
            DiagnosticCode::UnknownTable,
            "[[arr]] should be classified as a table"
        );
        assert_eq!(diag.severity, Severity::Error);
        assert_eq!(diag.location.field, "unknown_array");
        assert!(
            diag.message.contains("unknown_array"),
            "message should reference the array name, got: {}",
            diag.message,
        );
        // The fixture has `[[unknown_array]]` on line 4
        assert_eq!(
            diag.location.line, 4,
            "line number should point at the [[unknown_array]] heading"
        );
    }
}

mod filesystem_pass {
    use super::super::filesystem::check_filesystem;
    use super::super::*;
    use std::path::Path;

    fn fixture_path(name: &str) -> std::path::PathBuf {
        Path::new("src/preset/validate/tests/fixtures").join(name)
    }

    #[test]
    fn clean_preset_with_no_lut_or_extends_passes() {
        // The clean.toml fixture has neither a LUT nor an extends.
        let path = fixture_path("clean.toml");
        let diags = check_filesystem(&path);
        assert_eq!(diags, vec![]);
    }

    #[test]
    fn missing_lut_is_detected() {
        let path = fixture_path("missing_lut.toml");
        let diags = check_filesystem(&path);

        let lut_errors: Vec<_> = diags
            .iter()
            .filter(|d| d.code == DiagnosticCode::LutNotFound)
            .collect();
        assert_eq!(lut_errors.len(), 1);
        assert_eq!(lut_errors[0].location.field, "lut.path");
        assert!(
            lut_errors[0].message.contains("nonexistent/portra.cube"),
            "message should reference the missing path, got: {}",
            lut_errors[0].message,
        );
    }

    #[test]
    fn extends_cycle_is_detected() {
        let path = fixture_path("extends_cycle/a.toml");
        let diags = check_filesystem(&path);

        let cycle_errors: Vec<_> = diags
            .iter()
            .filter(|d| d.code == DiagnosticCode::ExtendsCycle)
            .collect();
        assert!(
            !cycle_errors.is_empty(),
            "expected at least one cycle diagnostic"
        );
    }

    #[test]
    fn extends_missing_file_is_detected() {
        let path = fixture_path("extends_missing.toml");
        let diags = check_filesystem(&path);

        let missing_errors: Vec<_> = diags
            .iter()
            .filter(|d| d.code == DiagnosticCode::ExtendsNotFound)
            .collect();
        assert_eq!(
            missing_errors.len(),
            1,
            "expected exactly one ExtendsNotFound diagnostic"
        );
        assert_eq!(missing_errors[0].location.field, "metadata.extends");
        assert!(
            missing_errors[0].message.contains("nonexistent_base.toml"),
            "message should reference the missing file, got: {}",
            missing_errors[0].message,
        );
    }
}

mod missing_required {
    use super::*;

    #[test]
    fn missing_required_diagnostic_code_is_reserved_for_future_use() {
        // Currently no preset fields are marked required by the schemars-derived
        // schema (every field has `#[serde(default)]`), so this code is reserved
        // for future schema changes that may introduce required fields.
        // If/when a required field is added, add a fixture and test here.
        let _ = DiagnosticCode::MissingRequired;
    }
}
