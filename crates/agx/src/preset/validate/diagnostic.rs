//! Diagnostic types for preset validation.
//!
//! These types are stable and serialize to a stable JSON shape — they are part
//! of the documented `--format=json` contract for `agx validate`.

use serde::{Deserialize, Serialize};

/// Severity of a single diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// A correctness error. `agx validate` exits non-zero if any diagnostic has this severity.
    Error,
    /// A warning. Surfaced in output but does not affect exit code.
    Warning,
}

/// Stable diagnostic code — used by `--format=json` consumers to filter or suppress
/// specific kinds of issues. Codes are kebab-case strings to avoid quoting issues
/// in shell pipelines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DiagnosticCode {
    /// An unrecognized TOML table was found in the preset file.
    UnknownTable,
    /// An unrecognized field was found within a known TOML table.
    UnknownField,
    /// A field value has the wrong type (e.g. string where a number is expected).
    TypeMismatch,
    /// A required field is absent from the preset file.
    MissingRequired,
    /// A field value is outside its documented valid range.
    OutOfRange,
    /// The LUT file referenced by `[lut].path` does not exist on disk.
    LutNotFound,
    /// The preset referenced by `[metadata].extends` does not exist on disk.
    ExtendsNotFound,
    /// The `extends` chain forms a cycle.
    ExtendsCycle,
    /// The preset file could not be read (e.g., file missing or permission denied).
    FileNotReadable,
    /// The preset file is not valid TOML (syntax error).
    SyntaxError,
}

/// Location of a diagnostic within a source file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Location {
    /// 1-based line number.
    pub line: usize,
    /// 1-based column number.
    pub column: usize,
    /// TOML field path that triggered the diagnostic, e.g. `"tone.exposure"` or `"lut.path"`.
    pub field: String,
}

/// A single validation diagnostic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    /// Severity level of this diagnostic.
    pub severity: Severity,
    /// Stable code identifying the kind of issue.
    pub code: DiagnosticCode,
    /// Human-readable description of the issue.
    pub message: String,
    /// Location within the source file where the issue was found.
    pub location: Location,
}

/// Validation result for a single preset file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileReport {
    /// Path to the validated preset file as given on the command line.
    /// No path normalization is applied — the value appears verbatim in
    /// the JSON output.
    pub path: String,
    /// Overall status — derived from the diagnostics.
    pub status: FileStatus,
    /// All diagnostics found during validation.
    pub diagnostics: Vec<Diagnostic>,
}

/// Overall pass/fail status for a single preset file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileStatus {
    /// No errors found.
    Ok,
    /// At least one diagnostic with [`Severity::Error`].
    Error,
}

/// Top-level validation report for one or more preset files.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationReport {
    /// Per-file validation results.
    pub files: Vec<FileReport>,
    /// Aggregate counts across all files.
    pub summary: Summary,
}

/// Aggregate counts for a [`ValidationReport`].
///
/// Constructed via [`ValidationReport::from_files`]. Direct construction is
/// possible because all fields are public, but `total == ok + errors` is an
/// invariant `from_files` maintains; constructing inconsistent values will
/// cause [`ValidationReport::has_errors`] to lie.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Summary {
    /// Total number of files validated.
    pub total: usize,
    /// Number of files with no errors.
    pub ok: usize,
    /// Number of files with at least one error.
    pub errors: usize,
}

impl ValidationReport {
    /// Build a report from a list of per-file reports. Computes summary fields.
    pub fn from_files(files: Vec<FileReport>) -> Self {
        let total = files.len();
        let errors = files
            .iter()
            .filter(|f| f.status == FileStatus::Error)
            .count();
        let ok = total - errors;
        Self {
            files,
            summary: Summary { total, ok, errors },
        }
    }

    /// True if any file has [`FileStatus::Error`].
    pub fn has_errors(&self) -> bool {
        self.summary.errors > 0
    }
}

impl FileReport {
    /// Build a report from a path + diagnostics. Status is derived: Error if any
    /// diagnostic has Severity::Error, otherwise Ok.
    pub fn new(path: impl Into<String>, diagnostics: Vec<Diagnostic>) -> Self {
        let status = if diagnostics.iter().any(|d| d.severity == Severity::Error) {
            FileStatus::Error
        } else {
            FileStatus::Ok
        };
        Self {
            path: path.into(),
            status,
            diagnostics,
        }
    }
}
