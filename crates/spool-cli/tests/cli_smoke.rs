use spool_format::SpoolFile;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use uuid::Uuid;

fn spool_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_spool"))
}

fn minimal_fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../spec/examples/minimal.spool")
}

#[test]
fn validate_minimal_spool_success() {
    let output = Command::new(spool_bin())
        .arg("validate")
        .arg(minimal_fixture())
        .output()
        .expect("failed to run spool validate");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("File is valid"));
}

#[test]
fn info_minimal_spool_contains_basics() {
    let output = Command::new(spool_bin())
        .arg("info")
        .arg(minimal_fixture())
        .output()
        .expect("failed to run spool info");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Session Information"));
    assert!(stdout.contains("Agent:      test"));
    assert!(stdout.contains("Entries:    1"));
}

#[test]
fn export_minimal_spool_writes_output() {
    let temp_dir = std::env::temp_dir().join(format!("spool-cli-test-{}", Uuid::new_v4()));
    fs::create_dir_all(&temp_dir).expect("failed to create temp dir");
    let output_path = temp_dir.join("exported.spool");

    let output = Command::new(spool_bin())
        .arg("export")
        .arg(minimal_fixture())
        .arg("--output")
        .arg(&output_path)
        .arg("--trim")
        .arg("0-1")
        .output()
        .expect("failed to run spool export");

    assert!(output.status.success());
    assert!(output_path.exists());

    let file = SpoolFile::from_path(&output_path).expect("failed to parse exported spool");
    assert_eq!(file.entries.len(), 1);
    assert_eq!(file.session.agent, "test");
}
