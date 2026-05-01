//! Human-readable formatter for ValidationReport.

use agx::preset::validate::{FileReport, FileStatus, Severity, ValidationReport};

/// Format a [`ValidationReport`] for display in a terminal.
///
/// Pass `quiet = true` to omit "ok" lines for clean files.
pub fn format_human(report: &ValidationReport, quiet: bool) -> String {
    let mut out = String::new();

    for file in &report.files {
        if file.status == FileStatus::Ok && quiet {
            continue;
        }
        out.push_str(&format_file(file));
        out.push('\n');
    }

    // Summary line
    let summary = &report.summary;
    if summary.errors == 0 {
        out.push_str(&format!(
            "{} file{} checked, all ok\n",
            summary.total,
            if summary.total == 1 { "" } else { "s" }
        ));
    } else {
        out.push_str(&format!(
            "{} file{} checked, {} with error{}\n",
            summary.total,
            if summary.total == 1 { "" } else { "s" },
            summary.errors,
            if summary.errors == 1 { "" } else { "s" }
        ));
    }

    out
}

fn format_file(file: &FileReport) -> String {
    let mut out = String::new();
    if file.status == FileStatus::Ok {
        out.push_str(&format!("{}: ok\n", file.path));
        return out;
    }

    out.push_str(&format!(
        "{}: {} problem{}\n",
        file.path,
        file.diagnostics.len(),
        if file.diagnostics.len() == 1 { "" } else { "s" }
    ));

    for diag in &file.diagnostics {
        let severity_label = match diag.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
        };
        out.push_str(&format!(
            "  {}: {} (line {})\n",
            severity_label, diag.message, diag.location.line
        ));
    }

    out
}
