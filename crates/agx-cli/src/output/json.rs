//! JSON formatter for ValidationReport.

use agx::preset::validate::ValidationReport;

/// Serialize a [`ValidationReport`] as pretty-printed JSON.
pub fn format_json(report: &ValidationReport) -> String {
    let mut s = serde_json::to_string_pretty(report).expect("ValidationReport always serializes");
    s.push('\n');
    s
}
