use spacetop_core::{
    domain::*, gate_proof, gates, index::WorkflowIndex, parser::parse_work_item, query::*,
    sources::WorkflowSources,
};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/durable-gates")
}
fn stage(name: &str, initial: bool, gate: bool, terminal: bool) -> StageDefinition {
    StageDefinition {
        name: name.into(),
        initial,
        gate,
        terminal,
        fresh: false,
        feedback_to: None,
        concurrency: None,
        worktree: false,
    }
}
fn parse(gate: &str) -> Entity {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("e.md");
    fs::write(
        &path,
        format!("---\nid: test\ntitle: readable\nstatus: review\n{gate}---\nBody"),
    )
    .unwrap();
    parse_work_item(&path, &["review".into()], None).unwrap()
}
fn pending() -> String {
    let contents = fs::read_to_string(fixture().join("merge.md")).unwrap();
    contents[contents.find("gates:").unwrap()..contents.find("---\n\nFixture").unwrap()].into()
}
#[test]
fn strict_gate_parser_matrix_refuses_invalid_authority_and_retains_extensions_only_at_application()
{
    let good = pending();
    let malformed = vec![
        good.replace("version: 1", "version: 3"),
        good.replace("id: gate-review", "id: gate-review\n      extra: ignored"),
        good.replace("id: attempt-one", "id: attempt-one\n          state: closed"),
        good.replace("id: briefing-one\n", "id: briefing-one\n            digest: duplicate\n"),
        good.replace("briefing: briefing-one", "briefing: wrong-binding"),
        good.replace("type: Resolution", "type: Annotation"),
        good.replace("decision: approve", "decision: maybe"),
        good.replace("state: pending", "state: unknown"),
        good.replace("target-stage: done", "target-stage: ''"),
        good.replace("state: pending", "state: pending\n            state: consumed"),
        good.replace("decision: approve", "decision: revise\n            reason: fix"),
        good.replace("by: person:captain", "by: person:captain\n            conn: {quote: grant, source: chat}"),
        good.replace("by: person:captain", "by: agent:first-officer\n            conn: {quote: '', source: chat}"),
        good.replace("          application:\n            target-stage: done\n            state: pending", "          application: []"),
        good.replace("          application:\n            target-stage: done\n            state: pending", "          application: null"),
        good.replace("          application:\n            target-stage: done\n            state: pending", "          application: scalar"),
        good.replace("          resolution:", "          withdrawal: {by: agent:first-officer, at: '2026-10-01T00:00:00Z', reason: retry}\n          resolution:"),
        "gates: {version: 1, records: []}\n".into(),
        "gates: {version: 1, records: [{id: g, stage: review, attempts: []}]}\n".into(),
        "gates: {room-ref: prototype}\n".into(),
    ];
    for bad in malformed {
        assert!(
            matches!(parse(&bad).gates, GateData::Invalid { .. }),
            "{bad}"
        );
    }
    let extended = good
        .replace(
            "state: pending",
            "state: pending\n            z-extension: future\n            a-extension: [unknown]",
        )
        .replace(
            "id: attempt-one",
            "id: attempt-one\n          provider-evidence: {retired: anything}",
        );
    let entity = parse(&extended);
    let GateData::Valid { document, warnings } = &entity.gates else {
        panic!("{:?}", entity.gates);
    };
    assert_eq!(
        warnings
            .iter()
            .map(|w| w.field.as_str())
            .collect::<Vec<_>>(),
        ["a-extension", "z-extension"]
    );
    assert_eq!(warnings[0].path, "gates.records[0].attempts[0].application");
    assert_eq!(
        document.records[0].attempts[0]
            .application
            .as_ref()
            .unwrap()
            .state,
        GateApplicationState::Pending
    );
    assert!(matches!(parse("").gates, GateData::Absent));
    let fo = good.replace(
        "by: person:captain",
        "by: agent:first-officer\n            conn: {quote: grant, source: chat}",
    );
    assert!(matches!(parse(&fo).gates, GateData::Valid { .. }));
    let roundtrip: Entity = serde_json::from_value(serde_json::to_value(entity).unwrap()).unwrap();
    assert_eq!(roundtrip.gates.document().unwrap().records.len(), 1);
}
#[test]
fn duplicate_stage_attempt_briefing_resolution_and_pending_applications_are_refused() {
    let good = pending();
    let record = good[good.find("    - id:").unwrap()..].to_string();
    for duplicate in [
        record.clone(),
        record.replace("id: gate-review", "id: second-gate"),
        record
            .replace("id: gate-review", "id: second-gate")
            .replace("stage: review", "stage: other"),
    ] {
        assert!(matches!(
            parse(&(good.clone() + &duplicate)).gates,
            GateData::Invalid { .. }
        ));
    }
    let attempt = good[good.find("        - id:").unwrap()..].to_string();
    // Distinct attempt id, but duplicate briefing or resolution, then all IDs
    // distinct while two approvals are pending in the same logical gate.
    for duplicate in [
        attempt.clone(),
        attempt.replace("attempt-one", "attempt-two"),
        attempt
            .replace("attempt-one", "attempt-two")
            .replace("briefing-one", "briefing-two"),
        attempt
            .replace("attempt-one", "attempt-two")
            .replace("briefing-one", "briefing-two")
            .replace("resolution-one", "resolution-two"),
    ] {
        assert!(matches!(
            parse(&(good.clone() + &duplicate)).gates,
            GateData::Invalid { .. }
        ));
    }
}
#[test]
fn readiness_uses_only_last_attempt_in_current_gated_nonterminal_stage() {
    let stages = [
        stage("review", false, true, false),
        stage("implement", false, false, false),
        stage("done", false, true, true),
    ];
    let good = pending();
    for (yaml, want) in [
        (good.clone(), GateReadiness::ApprovedAwaitingMerge),
        (
            good.replace("target-stage: done", "target-stage: implement"),
            GateReadiness::ApprovedAwaitingAdvance,
        ),
        (
            good.replace("target-stage: done", "target-stage: review"),
            GateReadiness::Invalid,
        ),
        (
            good.replace("target-stage: done", "target-stage: missing"),
            GateReadiness::Invalid,
        ),
        (
            good.replace("state: pending", "state: consumed"),
            GateReadiness::Consumed,
        ),
        (
            good.replace("state: pending", "state: superseded"),
            GateReadiness::Superseded,
        ),
    ] {
        assert_eq!(
            gates::readiness(&parse(&yaml).gates, "review", &stages, true),
            Some(want)
        );
    }
    let no_app = &good[..good.find("          application:").unwrap()];
    for (decision, want) in [
        ("approve", GateReadiness::Invalid),
        ("revise", GateReadiness::FeedbackPending),
        ("hold", GateReadiness::Held),
    ] {
        let data = parse(&no_app.replace(
            "decision: approve",
            &format!("decision: {decision}\n            reason: later"),
        ))
        .gates;
        assert_eq!(gates::readiness(&data, "review", &stages, true), Some(want));
    }
    let open = &good[..good.find("          resolution:").unwrap()];
    let withdrawn=format!("{open}          withdrawal: {{by: agent:first-officer, at: '2026-10-01T00:00:00Z', reason: retry}}\n");
    assert_eq!(
        gates::readiness(&parse(&withdrawn).gates, "review", &stages, true),
        Some(GateReadiness::WithdrawnAwaitingPrepare)
    );
    for at in [
        "2026-02-30T00:00:00Z",
        "2026-10-01T25:00:00Z",
        "2026-10-01T00:00:00+08:00",
        "nonsense",
        "2026-10-01T+1:00:00Z",
    ] {
        assert!(matches!(
            parse(&withdrawn.replace("2026-10-01T00:00:00Z", at)).gates,
            GateData::Invalid { .. }
        ));
    }
    let newer = open[open.find("        - id:").unwrap()..]
        .replace("attempt-one", "attempt-two")
        .replace("briefing-one", "briefing-two");
    let prior = good.replace("state: pending", "state: superseded");
    let data=parse(&(prior+&newer+"          resolution: {type: Resolution, id: second-resolution, briefing: briefing-two, by: person:captain, at: now, decision: revise, reason: retry}\n")).gates;
    assert_eq!(
        gates::readiness(&data, "review", &stages, false),
        Some(GateReadiness::FeedbackPending)
    );
    for status in ["implement", "done", "unknown"] {
        assert_eq!(
            gates::readiness(&parse(&good).gates, status, &stages, true),
            None
        );
    }
    for proven in [true, false] {
        assert_eq!(
            gates::readiness(&GateData::Absent, "review", &stages, proven),
            Some(if proven {
                GateReadiness::NeedsPreparation
            } else {
                GateReadiness::Validating
            })
        );
    }
    let prior = parse(&good.replace("stage: review", "stage: old")).gates;
    assert_eq!(
        gates::readiness(&prior, "review", &stages, true),
        Some(GateReadiness::NeedsPreparation)
    );
}
fn git(root: &Path, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
fn repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init", "-q"]);
    git(dir.path(), &["config", "user.name", "Test"]);
    git(dir.path(), &["config", "user.email", "test@example.test"]);
    dir
}
fn commit(root: &Path) {
    git(root, &["add", "."]);
    git(root, &["commit", "-qm", "fixture"]);
}
fn report(stage: &str) -> String {
    format!("## Stage Report: {stage}\n\n- DONE: exercised behavior\n  command and observed output\n- SKIPPED: optional backend\n  unchanged\n\n### Summary\nComplete.\n")
}
#[test]
fn preparation_requires_latest_exact_complete_report_and_clean_tracked_authority_path() {
    let dir = repo();
    let root = dir.path();
    let path = root.join("seed[1].md");
    let review_stage = stage("review", false, true, false);
    let good = report("review");
    fs::write(&path, &good).unwrap();
    let prove = |body: &str, s: &StageDefinition| {
        gate_proof::prove(&path, body, s, &spacetop_core::git::StdGitRunner)
    };
    assert!(!prove(&good, &review_stage).proven); // untracked
    commit(root);
    assert!(prove(&good, &review_stage).proven);
    fs::write(root.join("dirty-sibling"), "dirty").unwrap();
    assert!(prove(&good, &review_stage).proven);
    fs::write(&path, format!("{good}dirty")).unwrap();
    assert!(!prove(&good, &review_stage).proven);
    fs::write(&path, &good).unwrap();
    for bad in [
        report("other"),
        good.replace("DONE:", "FAILED:"),
        good.replace("command and observed output", ""),
        good.replace("Complete.", ""),
        good.replace("exercised behavior", ""),
        format!("{good}\n## Stage Report: review (cycle 2)\nIncomplete"),
        "## Stage Report: review\n### Summary\nEmpty checklist".into(),
    ] {
        fs::write(&path, &bad).unwrap();
        commit(root);
        assert!(!prove(&bad, &review_stage).proven, "{bad}");
    }
    let initial = stage("seed", true, true, false);
    fs::write(&path, "seed").unwrap();
    commit(root);
    assert!(prove("seed", &initial).proven);
    fs::write(&path, "dirty seed").unwrap();
    assert!(!prove("dirty seed", &initial).proven);
    let non_git = tempfile::tempdir().unwrap();
    let p = non_git.path().join("e.md");
    fs::write(&p, "seed").unwrap();
    assert!(!gate_proof::prove(&p, "seed", &initial, &spacetop_core::git::StdGitRunner).proven);
    struct FailedProbe;
    impl spacetop_core::git::GitRunner for FailedProbe {
        fn run(&self, _: &Path, _: &[&str]) -> std::io::Result<spacetop_core::git::GitCmdResult> {
            Err(std::io::Error::other("probe failure"))
        }
    }
    assert!(!gate_proof::prove(&p, "seed", &initial, &FailedProbe).proven);
}
#[test]
fn query_keeps_runtime_and_archive_authority_separate() {
    let root = fixture();
    let mut index = WorkflowIndex::load(&root, &root).unwrap();
    let before = fs::read(root.join("merge.md")).unwrap();
    let rows = index.query(EntityQuery {
        field_filters: vec![FieldFilter::GateReadiness(
            GateReadiness::ApprovedAwaitingMerge,
        )],
        ..Default::default()
    });
    assert_eq!(
        rows.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(),
        ["merge"]
    );
    assert!(!index.entity_has_current_activity("pending"));
    let mut entity = rows[0].clone();
    entity.id = "archived".into();
    index.replace_archive(spacetop_core::sources::ArchiveSnapshot {
        entities: vec![entity],
        parse_errors: vec![],
        error: None,
    });
    let archive = index.query(EntityQuery {
        scope: QueryScope::Archived,
        ..Default::default()
    });
    assert!(archive[0].gates.document().is_some());
    assert_eq!(archive[0].gate_readiness, None);
    let filtered = index.query(EntityQuery {
        scope: QueryScope::All,
        field_filters: vec![FieldFilter::GateReadiness(
            GateReadiness::ApprovedAwaitingMerge,
        )],
        ..Default::default()
    });
    assert_eq!(filtered.len(), 1);
    assert_eq!(fs::read(root.join("merge.md")).unwrap(), before);
}
#[test]
fn split_root_proof_and_merged_body_use_physical_authority_not_displayed_report() {
    let dir = repo();
    let root = dir.path();
    let workflow = root.join("docs/wf");
    fs::create_dir_all(&workflow).unwrap();
    let readme="---\ncommissioned-by: spacedock@0.27.3\nstages:\n  states:\n    - name: review\n      gate: true\n    - name: done\n      terminal: true\n---\n";
    fs::write(workflow.join("README.md"), readme).unwrap();
    let entity = |body: &str| format!("---\nid: e\ntitle: Proof\nstatus: review\n---\n{body}");
    fs::write(workflow.join("e.md"), entity("Incomplete")).unwrap();
    commit(root);
    let wt = root.join(".worktrees/task/docs/wf");
    fs::create_dir_all(&wt).unwrap();
    fs::write(wt.join("e.md"), entity(&report("review"))).unwrap();
    let sources = WorkflowSources::load_active(&workflow, root).unwrap();
    let e = &sources.active.items[0];
    assert!(e.body.contains("Stage Report"));
    assert!(!e.gate_preparation.proven);
    fs::write(workflow.join("e.md"), entity(&report("review"))).unwrap();
    commit(root);
    fs::write(wt.join("e.md"), entity("Incomplete worktree")).unwrap();
    let sources = WorkflowSources::load_active(&workflow, root).unwrap();
    assert!(sources.active.items[0].gate_preparation.proven);
    // A nested state checkout has its own HEAD, not the definition repo's.
    let state = workflow.join("state");
    fs::create_dir_all(&state).unwrap();
    git(&state, &["init", "-q"]);
    git(&state, &["config", "user.name", "Test"]);
    git(&state, &["config", "user.email", "test@example.test"]);
    fs::write(state.join("e.md"), entity(&report("review"))).unwrap();
    commit(&state);
    fs::write(
        workflow.join("README.md"),
        readme.replacen("stages:", "state: state\nstages:", 1),
    )
    .unwrap();
    let sources = WorkflowSources::load_active(&workflow, root).unwrap();
    assert!(
        sources
            .active
            .items
            .iter()
            .find(|e| e.id == "e")
            .unwrap()
            .gate_preparation
            .proven
    );
    fs::write(state.join("e.md"), entity("Dirty incomplete")).unwrap();
    let sources = WorkflowSources::load_active(&workflow, root).unwrap();
    assert!(
        !sources
            .active
            .items
            .iter()
            .find(|e| e.id == "e")
            .unwrap()
            .gate_preparation
            .proven
    );
}

#[cfg(unix)]
#[test]
fn external_unverified_state_remains_readable_without_preparation_probes() {
    let dir = tempfile::tempdir().unwrap();
    let workflow = dir.path().join("wf");
    fs::create_dir(&workflow).unwrap();
    let readme = fs::read_to_string(fixture().join("README.md")).unwrap();
    fs::write(
        workflow.join("README.md"),
        readme.replacen("stages:", "state: state\nstages:", 1),
    )
    .unwrap();
    let external = repo();
    fs::write(
        external.path().join("seed.md"),
        "---\nid: seed\ntitle: Clean external seed\nstatus: seed\n---\nSeed.\n",
    )
    .unwrap();
    commit(external.path());
    std::os::unix::fs::symlink(external.path(), workflow.join("state")).unwrap();
    let index = WorkflowIndex::load(&workflow, dir.path()).unwrap();
    let entity = index.entity_by_id("seed").unwrap();
    assert_eq!(entity.gate_readiness, Some(GateReadiness::Validating));
    assert!(!entity.gate_preparation.proven);
    assert!(entity.gate_preparation.diagnostics[0].contains("probes skipped"));
    assert!(matches!(
        index.storage(),
        WorkflowStorage::SplitRoot {
            disposition: StateCheckoutDisposition::Unverified { .. },
            ..
        }
    ));
}
