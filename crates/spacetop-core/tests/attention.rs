use spacetop_core::{domain::*, index::WorkflowIndex, query::*, sources::WorkflowSources};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/attention")
}
fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &to.join(entry.file_name()));
        } else {
            fs::copy(entry.path(), to.join(entry.file_name())).unwrap();
        }
    }
}
fn git(root: &Path, args: &[&str]) {
    assert!(Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .unwrap()
        .status
        .success());
}
fn attention(index: &WorkflowIndex) -> Vec<(String, AttentionReason)> {
    index
        .query(EntityQuery {
            field_filters: vec![FieldFilter::NeedsAttention],
            ..Default::default()
        })
        .iter()
        .map(|e| (e.id.clone(), WorkflowIndex::attention_reason(e).unwrap()))
        .collect()
}

#[test]
fn fixture_queries_every_reason_and_verifies_real_evidence_without_runtime_inference() {
    use AttentionReason::*;
    let temp = tempfile::tempdir().unwrap();
    copy_tree(&fixture(), temp.path());
    git(temp.path(), &["init", "-q"]);
    git(temp.path(), &["add", "."]);
    git(
        temp.path(),
        &[
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.test",
            "commit",
            "-qm",
            "fixture",
        ],
    );
    let root = fs::canonicalize(temp.path()).unwrap();
    let mut index = WorkflowIndex::load(&root, &root).unwrap();
    let expected = [
        ("3k", ReviewRequested),
        ("advance", AdvancePending),
        ("feedback", RecordedFollowUp),
        ("held", RecordedBlocker),
        ("invalid", InvalidInformation),
        ("invalid-ungated", InvalidInformation),
        ("merge", MergePending),
        ("missing-artifact", ReviewRequested),
        ("missing-room", ReviewRequested),
        ("preparation", PreparationNeeded),
        ("unverified", MissingOrUnverifiedInformation),
        ("withdrawn", ReplacementNeeded),
    ];
    let baseline = attention(&index);
    assert_eq!(
        baseline,
        expected.map(|(id, reason)| (id.to_string(), reason))
    );
    assert_eq!(index.active_parse_errors().len(), 1);
    for timestamp in [0, 1_900_000_000] {
        for activity in [
            EntityActivity::Idle {
                updated_unix: Some(timestamp),
            },
            EntityActivity::Running {
                handler: ActivityHandler::Worker,
                runtime: AgentRuntime::Codex,
                session_id: "session".into(),
                updated_unix: timestamp,
            },
            EntityActivity::HumanGate {
                runtime: AgentRuntime::ClaudeCode,
                session_id: "session".into(),
                updated_unix: timestamp,
            },
        ] {
            for errors in [vec![], vec!["activity scan unavailable".into()]] {
                index.replace_session_scan_report(SessionScanReport {
                    workflow_dir: temp.path().into(),
                    repo_root: temp.path().into(),
                    scanned_roots: vec![],
                    errors,
                    attributions: index
                        .query(EntityQuery::default())
                        .iter()
                        .map(|e| EntityActivityAttribution {
                            entity_id: e.id.clone(),
                            activity: activity.clone(),
                        })
                        .collect(),
                });
                assert_eq!(attention(&index), baseline);
            }
        }
    }
    let rows = index.query(EntityQuery {
        text: Some("3k".into()),
        status: Some("validation".into()),
        sort: EntitySort::Status,
        field_filters: vec![FieldFilter::NeedsAttention],
        ..Default::default()
    });
    assert_eq!(rows.len(), 1);
    let entity = &rows[0];
    let record = &entity.gates.document().unwrap().records[0];
    let attempt = &record.attempts[0];
    let room = spacetop_core::gate_room::load(index.definition(), entity, record, attempt);
    assert!(room.verified(), "{:?}", room.diagnostics);
    assert_eq!(
        spacetop_core::gate_room::read_item(
            index.definition(),
            entity,
            record,
            attempt,
            "artifact:gate-review"
        )
        .unwrap(),
        b"Exact candidate review.\n"
    );
    for id in ["missing-room", "missing-artifact"] {
        let entity = index
            .query(EntityQuery {
                text: Some(id.into()),
                ..Default::default()
            })
            .into_iter()
            .find(|e| e.id == id)
            .unwrap();
        let record = &entity.gates.document().unwrap().records[0];
        let attempt = &record.attempts[0];
        let room = spacetop_core::gate_room::load(index.definition(), &entity, record, attempt);
        assert_eq!(room.verified(), id == "missing-artifact");
        assert!(spacetop_core::gate_room::read_item(
            index.definition(),
            &entity,
            record,
            attempt,
            "artifact:gate-review"
        )
        .is_err());
    }
    index.replace_archive(WorkflowSources::load_archive(
        temp.path(),
        index.definition(),
    ));
    assert!(index
        .query(EntityQuery {
            scope: QueryScope::Archived,
            field_filters: vec![FieldFilter::NeedsAttention],
            ..Default::default()
        })
        .is_empty());
    assert_eq!(attention(&index), baseline);
}
