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
