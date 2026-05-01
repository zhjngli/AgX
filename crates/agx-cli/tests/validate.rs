//! Integration tests for `agx validate` subcommand.

use std::process::Command;

fn cli_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_agx"))
}

fn fixture_path(name: &str) -> std::path::PathBuf {
    let mut path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop();
    path.push("agx/src/preset/validate/tests/fixtures");
    path.push(name);
    path
}

#[test]
fn validate_clean_exits_zero() {
    let output = cli_bin()
        .arg("validate")
        .arg(fixture_path("clean.toml"))
        .output()
        .expect("failed to run agx");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ok"));
}

#[test]
fn validate_broken_exits_one() {
    let output = cli_bin()
        .arg("validate")
        .arg(fixture_path("unknown_table.toml"))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("unknown table"));
    assert!(stdout.contains("tone_curves"));
}

#[test]
fn validate_quiet_skips_ok_lines() {
    let output = cli_bin()
        .arg("validate")
        .arg("--quiet")
        .arg(fixture_path("clean.toml"))
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains(": ok"));
    assert!(stdout.contains("all ok"));
}

#[test]
fn validate_json_output_is_valid_json() {
    let output = cli_bin()
        .arg("validate")
        .arg("--format=json")
        .arg(fixture_path("unknown_table.toml"))
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("output should be valid JSON");
    assert_eq!(parsed["summary"]["errors"], 1);
    assert_eq!(parsed["files"][0]["status"], "error");
    assert_eq!(
        parsed["files"][0]["diagnostics"][0]["code"],
        "unknown-table"
    );
}

#[test]
fn validate_multiple_files_aggregates_summary() {
    let output = cli_bin()
        .arg("validate")
        .arg(fixture_path("clean.toml"))
        .arg(fixture_path("unknown_table.toml"))
        .output()
        .unwrap();
    // One file clean, one broken → exit code 1
    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("clean.toml"));
    assert!(stdout.contains("unknown_table.toml"));
    // Summary should reflect both files
    assert!(stdout.contains("2 files checked"));
}

#[test]
fn validate_quiet_still_shows_errors_for_broken_files() {
    let output = cli_bin()
        .arg("validate")
        .arg("--quiet")
        .arg(fixture_path("clean.toml"))
        .arg(fixture_path("unknown_table.toml"))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Clean file's "ok" line is suppressed
    assert!(!stdout.contains("clean.toml: ok"));
    // Broken file's diagnostics still appear
    assert!(stdout.contains("unknown_table.toml"));
    assert!(stdout.contains("unknown table"));
}
