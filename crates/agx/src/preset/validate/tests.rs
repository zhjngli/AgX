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
    use super::super::semantic::{check_schema, find_unknown_fields};
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

    #[test]
    fn unknown_field_in_known_table_is_detected_via_walker() {
        // unknown_field.toml has [lut] amount = 0.8 — structural pass skips it,
        // semantic walker must catch it.
        let toml_str = fixture("unknown_field.toml");
        let diags = find_unknown_fields(&toml_str);
        let unknown_errors: Vec<_> = diags
            .iter()
            .filter(|d| d.code == DiagnosticCode::UnknownField)
            .collect();
        assert_eq!(
            unknown_errors.len(),
            1,
            "expected exactly one UnknownField diagnostic, got: {:?}",
            diags
        );
        assert_eq!(unknown_errors[0].location.field, "lut.amount");
        // The fixture has `amount = 0.8` on line 6
        assert_eq!(
            unknown_errors[0].location.line, 6,
            "line number should point at the amount field"
        );
    }

    #[test]
    fn unknown_field_in_optional_struct_table_is_detected() {
        // hsl is Option<PartialHslChannels> — rendered as anyOf: [null, struct] in schema.
        // The walker must resolve through anyOf and catch the unknown channel name.
        let toml_str = r#"
[metadata]
name = "Test"

[hsl]
weird_channel = { hue = 0 }
"#;
        let diags = find_unknown_fields(toml_str);
        let unknown_tables: Vec<_> = diags
            .iter()
            .filter(|d| {
                d.code == DiagnosticCode::UnknownTable && d.location.field == "hsl.weird_channel"
            })
            .collect();
        assert_eq!(
            unknown_tables.len(),
            1,
            "expected unknown nested table in hsl to be detected via walker, got: {:?}",
            diags
        );
        // Message must be clean — no "anyOf" in it
        assert!(
            !unknown_tables[0].message.contains("anyOf"),
            "message should not mention anyOf, got: {}",
            unknown_tables[0].message
        );
        assert!(
            unknown_tables[0].message.contains("weird_channel"),
            "message should mention the unknown field name, got: {}",
            unknown_tables[0].message
        );
    }

    #[test]
    fn unknown_field_in_deeply_nested_table_uses_correct_parent_and_line() {
        // `hsl.red.weird_red` is a depth-3 unknown. The walker must report
        // `[hsl.red]` as the parent section (not `[hsl]`), and find_position_by_path
        // must match `[hsl.red]` in the source so the line number is not the
        // fallback (1, 1).
        let toml_str = fixture("unknown_deep_nested.toml");
        let diags = find_unknown_fields(&toml_str);

        let target: Vec<_> = diags
            .iter()
            .filter(|d| d.location.field == "hsl.red.weird_red")
            .collect();
        assert_eq!(
            target.len(),
            1,
            "expected exactly one diagnostic for hsl.red.weird_red, got: {:?}",
            diags
        );

        let diag = target[0];
        // Parent should be `[hsl.red]`, not `[hsl]`
        assert!(
            diag.message.contains("[hsl.red]"),
            "message should reference parent [hsl.red], got: {}",
            diag.message,
        );
        assert!(
            !diag.message.contains("in section `[hsl]`"),
            "message should NOT reference root [hsl] for a depth-3 field, got: {}",
            diag.message,
        );

        // Line number should point at the `weird_red` line (line 7 in the fixture),
        // not the fallback line 1.
        assert!(
            diag.location.line >= 6,
            "expected line >= 6 (weird_red is on line 7 in the fixture), got: {}",
            diag.location.line,
        );
    }

    #[test]
    fn no_duplicate_diagnostics_for_nested_unknowns() {
        // check_schema must NOT produce UnknownField/UnknownTable diagnostics —
        // those belong exclusively to find_unknown_fields.
        let toml_str = r#"
[tone]
exposre = 0.5
"#;
        let schema_diags = check_schema(toml_str);
        let unknown_from_schema: Vec<_> = schema_diags
            .iter()
            .filter(|d| {
                d.code == DiagnosticCode::UnknownField || d.code == DiagnosticCode::UnknownTable
            })
            .collect();
        assert!(
            unknown_from_schema.is_empty(),
            "check_schema must not produce unknown-field diagnostics; that's find_unknown_fields' job. Got: {:?}",
            unknown_from_schema
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
    fn structural_pass_does_not_recurse_into_known_tables() {
        // The unknown_field.toml fixture has [lut] amount = 0.8 — an unknown field
        // nested inside a known top-level table. The structural pass handles ONLY
        // top-level unknowns, so it must produce zero diagnostics here. The nested
        // unknown is detected by semantic::find_unknown_fields instead.
        let toml_str = fixture("unknown_field.toml");
        let diags = detect_unknown_fields(&toml_str);
        assert_eq!(
            diags,
            vec![],
            "structural pass must not recurse into known tables; got: {:?}",
            diags
        );
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

    #[test]
    fn extends_malformed_target_is_detected() {
        let path = fixture_path("extends_malformed/parent.toml");
        let diags = check_filesystem(&path);
        assert!(
            diags
                .iter()
                .any(|d| d.code == DiagnosticCode::ExtendsNotFound
                    && d.message.contains("unparseable")),
            "expected ExtendsNotFound with 'unparseable' message; got: {:?}",
            diags
        );
    }

    #[test]
    fn extends_malformed_via_intermediate_includes_via_suffix() {
        // child.toml → parent.toml → malformed.toml (child is the entry file)
        // The diagnostic is attributed to parent.toml's `extends` field, so the
        // message must include "(via parent.toml)" to disambiguate.
        let path = fixture_path("extends_malformed/child.toml");
        let diags = check_filesystem(&path);
        let malformed_diag = diags.iter().find(|d| {
            d.code == DiagnosticCode::ExtendsNotFound && d.message.contains("unparseable")
        });
        assert!(
            malformed_diag.is_some(),
            "expected ExtendsNotFound with 'unparseable' message; got: {:?}",
            diags
        );
        let msg = &malformed_diag.unwrap().message;
        assert!(
            msg.contains("(via parent.toml)"),
            "message for a chain where the malformed extends is in an ancestor file \
             must contain '(via parent.toml)' to disambiguate, got: {}",
            msg
        );
    }
}

mod top_level_api {
    use super::super::*;
    use crate::preset::Preset;
    use std::path::Path;

    fn fixture_path(name: &str) -> std::path::PathBuf {
        Path::new("src/preset/validate/tests/fixtures").join(name)
    }

    #[test]
    fn validate_clean_preset_returns_ok_status() {
        let path = fixture_path("clean.toml");
        let report = Preset::validate(&path);
        assert_eq!(report.status, FileStatus::Ok);
        assert!(report.diagnostics.is_empty());
    }

    #[test]
    fn validate_unknown_table_returns_error_status() {
        let path = fixture_path("unknown_table.toml");
        let report = Preset::validate(&path);
        assert_eq!(report.status, FileStatus::Error);
        assert!(report
            .diagnostics
            .iter()
            .any(|d| d.code == DiagnosticCode::UnknownTable));
    }

    #[test]
    fn validate_combines_all_three_passes() {
        // Use a fixture with multiple kinds of issues. For now, this test
        // confirms the API runs all passes; multi-issue fixture can be added
        // later if a real combined-failure case emerges.
        let path = fixture_path("out_of_range.toml");
        let report = Preset::validate(&path);
        assert_eq!(report.status, FileStatus::Error);
        assert!(report
            .diagnostics
            .iter()
            .any(|d| d.code == DiagnosticCode::OutOfRange));
    }

    #[test]
    fn validate_real_world_preset_with_full_features_returns_ok() {
        let path = fixture_path("real_world_clean.toml");
        let report = Preset::validate(&path);
        assert_eq!(
            report.status,
            FileStatus::Ok,
            "real-world preset using HSL, grain, vignette, detail, noise_reduction, \
             color_grading, dehaze should validate clean. Diagnostics: {:?}",
            report.diagnostics
        );
    }

    #[test]
    fn syntax_error_in_toml_is_reported_as_error() {
        let path = fixture_path("syntax_error.toml");
        let report = Preset::validate(&path);
        assert_eq!(
            report.status,
            FileStatus::Error,
            "syntax error must be Error, not Ok. Diagnostics: {:?}",
            report.diagnostics
        );
        assert_eq!(
            report.diagnostics.len(),
            1,
            "expected exactly one diagnostic for syntax error"
        );
        assert_eq!(report.diagnostics[0].code, DiagnosticCode::SyntaxError);
        assert!(
            report.diagnostics[0]
                .message
                .to_lowercase()
                .contains("syntax")
                || report.diagnostics[0]
                    .message
                    .to_lowercase()
                    .contains("toml")
                || report.diagnostics[0]
                    .message
                    .to_lowercase()
                    .contains("invalid")
                || report.diagnostics[0]
                    .message
                    .to_lowercase()
                    .contains("expected"),
            "message should describe the syntax error, got: {}",
            report.diagnostics[0].message,
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
