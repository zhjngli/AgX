//! `agx validate` subcommand.

use crate::output::{format_human, format_json};
use crate::OutputFormat;
use agx::preset::validate::{FileReport, ValidationReport};
use std::path::PathBuf;

/// Entry point for `agx validate`. Returns the process exit code (0, 1, or 2).
///
/// - 0: all files clean
/// - 1: at least one file has errors
/// - 2: no files given
pub fn run_validate(paths: &[PathBuf], quiet: bool, format: OutputFormat) -> i32 {
    if paths.is_empty() {
        eprintln!("error: no preset files given");
        return 2;
    }

    let file_reports: Vec<FileReport> = paths
        .iter()
        .map(|p| agx::preset::Preset::validate(p))
        .collect();

    let report = ValidationReport::from_files(file_reports);

    let output = match format {
        OutputFormat::Human => format_human(&report, quiet),
        OutputFormat::Json => format_json(&report),
    };
    print!("{}", output);

    if report.has_errors() {
        1
    } else {
        0
    }
}
