use serde_json::Value;
use std::path::PathBuf;
use std::process::Command;

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/durable-gates")
}
fn cli(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_spacetop"))
        .args(args)
        .arg("-w")
        .arg(fixture())
        .output()
        .unwrap()
}
#[test]
fn export_projects_current_authority_without_sessions_and_preserves_invalid_rows() {
    let before = std::fs::read(fixture().join("invalid.md")).unwrap();
    let output = cli(&["export", "--json"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    let rows = json["entities"].as_array().unwrap();
    let row = |id: &str| rows.iter().find(|row| row["id"] == id).unwrap();
    assert_eq!(row("pending")["gate_readiness"], "awaiting-captain");
    assert_eq!(
        row("advance")["gate_readiness"],
        "approved-awaiting-advance"
    );
    assert_eq!(row("merge")["gate_readiness"], "approved-awaiting-merge");
    assert_eq!(row("consumed")["gate_readiness"], "consumed");
    assert_eq!(row("superseded")["gate_readiness"], "superseded");
    assert!(row("historical")["gate_readiness"].is_null());
    assert_eq!(row("invalid")["gate_readiness"], "invalid");
    assert!(!row("invalid")["gates"]["diagnostics"]
        .as_array()
        .unwrap()
        .is_empty());
    assert_eq!(std::fs::read(fixture().join("invalid.md")).unwrap(), before);
}
#[test]
fn readiness_filter_does_not_queue_spent_or_historical_approval() {
    let output = cli(&[
        "list",
        "--gate-readiness",
        "approved-awaiting-merge",
        "--json",
    ]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let rows: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(rows.as_array().unwrap().len(), 1);
    assert_eq!(rows[0]["id"], "merge");
    assert!(!cli(&["list", "--gate-readiness", "made-up"])
        .status
        .success());
}

#[test]
fn archived_export_retains_gate_records_without_active_readiness() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::copy(fixture().join("README.md"), dir.path().join("README.md")).unwrap();
    std::fs::create_dir(dir.path().join("_archive")).unwrap();
    let path = dir.path().join("_archive/merge.md");
    std::fs::copy(fixture().join("merge.md"), &path).unwrap();
    let before = std::fs::read(&path).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_spacetop"))
        .args(["export", "--json", "-w"])
        .arg(dir.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["archived_entities"][0]["gates"]["kind"], "valid");
    assert_eq!(
        json["archived_entities"][0]["gates"]["document"]["records"][0]["attempts"][0]
            ["application"]["state"],
        "pending"
    );
    assert!(json["archived_entities"][0]["gate_readiness"].is_null());
    assert_eq!(std::fs::read(path).unwrap(), before);
}
