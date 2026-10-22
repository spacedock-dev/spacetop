use spacetop_core::{
    correction_history as history,
    domain::*,
    parser::{self, correction_history as decode, gate_room::canonical_digest},
};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/correction_history")
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
struct Setup {
    _temp: tempfile::TempDir,
    root: PathBuf,
    definition: WorkflowDefinition,
    entity: Entity,
}
impl Setup {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(temp.path()).unwrap();
        copy_tree(&fixtures().join("workflow"), &root);
        let definition = parser::parse_workflow_readme(&root.join("README.md")).unwrap();
        let entity = parser::parse_work_item(
            &root.join("sample/index.md"),
            &["implementation".into()],
            None,
        )
        .unwrap();
        Self {
            _temp: temp,
            root,
            definition,
            entity,
        }
    }
    fn load(&self) -> CorrectionHistory {
        history::load(&self.definition, &self.entity)
    }
    fn room(&self) -> PathBuf {
        self.root.join("sample/review/implementation/round-1")
    }
    fn log(&self, bytes: &[u8]) {
        fs::write(self.room().join("briefing.review.jsonl"), bytes).unwrap();
    }
    fn briefing(&mut self, value: serde_json::Value) {
        let bytes = serde_json::to_vec(&value).unwrap();
        fs::write(self.room().join("briefing.json"), &bytes).unwrap();
        if let ReviewPointer::Valid { pointer } = &mut self.entity.review_round {
            pointer.briefing.digest = canonical_digest(&bytes).unwrap();
        }
    }
}
fn edges(facts: &CorrectionHistory, record: &ReviewRecord) -> Vec<RecordedRelationship> {
    match facts.correction_relationships(&record.identity) {
        CorrectionRelationships::Recorded(edges) => edges,
        other => panic!("{other:?}"),
    }
}
#[test]
fn oracle_numeric_order_current_pointer_and_advisory_closure() {
    let setup = Setup::new();
    let facts = setup.load();
    assert!(facts.diagnostics.is_empty());
    assert_eq!(
        facts
            .rounds
            .iter()
            .map(|r| (r.identity.stage.as_str(), r.identity.cycle))
            .collect::<Vec<_>>(),
        [
            ("implementation", 1),
            ("implementation", 2),
            ("implementation", 10),
            ("validation", 1)
        ]
    );
    let round = &facts.rounds[0];
    assert_eq!(round.provenance, RoundProvenance::CurrentPointer);
    assert_eq!(round.decision(), Some(GateDecision::Revise));
    assert_eq!(round.completeness, ReviewCompleteness::Answered);
    assert_eq!(round.records.len(), 5);
    assert_eq!(
        round.references[0].label,
        "Replacement job 597 cleared the unchanged tip"
    );
    assert_eq!(round.records[0].by, "software:roborev");
    assert!(
        matches!(&round.records[3].entry, ReviewEntry::Annotation(a) if a.body.as_ref().unwrap().contains("correct-but-disproportionate"))
    );
    assert_eq!(
        history::read_item(
            &setup.definition,
            &setup.entity,
            round,
            &round.briefing.as_ref().unwrap().items[0]
        )
        .unwrap(),
        fs::read(fixtures().join("upstream/candidate.patch")).unwrap()
    );
    assert_eq!(
        round.digest.as_deref(),
        Some("sha256:006c419cc260eac142a2a038037d4d8a8d80156f7b1f7224b3be6eab682a0698")
    );
    assert_eq!(facts.rounds[2].records.len(), 5);
    assert_eq!(facts.rounds[2].completeness, ReviewCompleteness::Invalid);
}
#[test]
fn same_briefing_relationships_expose_actual_chain_not_prose_or_adjacent_rounds() {
    let setup = Setup::new();
    let facts = setup.load();
    let round = &facts.rounds[0];
    let first = edges(&facts, &round.records[0]);
    assert_eq!(first.len(), 2);
    assert_eq!(first[0].role, RelationshipRole::ReviewerEvidence);
    assert_eq!(first[1].role, RelationshipRole::FollowUp);
    assert_eq!(first[1].source, round.records[3].identity);
    assert_eq!(first[1].target, round.records[0].identity);
    let disposition = edges(&facts, &round.records[3]);
    assert_eq!(disposition.len(), 3);
    assert_eq!(disposition[2].source, round.records[4].identity);
    assert_eq!(disposition[2].target, round.records[3].identity);
    assert!(!first.iter().any(|e| e.source == round.records[4].identity));
    // Same IDs and prose in cycle 2 do not create a follow-up edge. Clock order is irrelevant.
    let other = &facts.rounds[1];
    assert_eq!(other.records[3].ordinal, 4);
    assert_eq!(edges(&facts, &other.records[0]).len(), 1);
    for edge in first {
        assert_eq!(edge.source.round, round.identity);
        assert_eq!(edge.target.round, round.identity);
    }
    let mut alien = round.records[0].identity.clone();
    alien.round.origin = setup.root.join("another-workflow/index.md");
    assert_eq!(
        facts.correction_relationships(&alien),
        CorrectionRelationships::Unavailable
    );
}
#[test]
fn pointer_schema_duplicate_keys_cycles_and_legacy_absence() {
    let setup = Setup::new();
    let original = fs::read_to_string(setup.entity.path.clone()).unwrap();
    let frontmatter = original.split("---").nth(1).unwrap();
    assert!(matches!(
        decode::parse_pointer(frontmatter, "3j"),
        ReviewPointer::Valid { .. }
    ));
    assert_eq!(
        decode::parse_pointer("id: legacy\ntitle: Legacy", "legacy"),
        ReviewPointer::Absent
    );
    for bad in [
        frontmatter.replace("cycle: 1", "cycle: 0"),
        frontmatter.replace("cycle: 1", "cycle: 4294967296"),
        frontmatter.replace("cycle: 1", "cycle: 1\n  cycle: 2"),
        frontmatter.replace("stage: implementation", "stage: ../escape"),
        frontmatter.replace(
            "./review/implementation/round-1",
            "./review/implementation/round-01",
        ),
        frontmatter.replace("round:3j", "round:other"),
        frontmatter.replace("briefing:3j", "briefing:other"),
        frontmatter.replace("cycle: 1", "cycle: 1\n  unexpected: true"),
    ] {
        assert!(
            matches!(
                decode::parse_pointer(&bad, "3j"),
                ReviewPointer::Invalid { .. }
            ),
            "{bad}"
        );
    }
    for number in ["0", "01", "-1", "+1", "4294967296", "1/2"] {
        assert_eq!(decode::cycle_number(number), None);
    }
}
#[test]
fn missing_invalid_stale_and_absent_pointers_never_choose_newest_as_current() {
    let mut setup = Setup::new();
    setup.entity.review_round = ReviewPointer::Absent;
    assert!(setup
        .load()
        .rounds
        .iter()
        .all(|r| r.provenance == RoundProvenance::Retained));
    setup.entity.review_round = ReviewPointer::Invalid {
        diagnostic: "bad YAML".into(),
    };
    let facts = setup.load();
    assert_eq!(facts.diagnostics.len(), 1);
    assert_eq!(facts.rounds.len(), 4);
    assert!(facts
        .rounds
        .iter()
        .all(|r| r.provenance == RoundProvenance::Retained));
    setup.entity = parser::parse_work_item(
        &setup.root.join("missing/index.md"),
        &["implementation".into()],
        None,
    )
    .unwrap();
    let facts = setup.load();
    assert_eq!(facts.rounds.len(), 1);
    assert_eq!(facts.rounds[0].identity.cycle, 7);
    assert_eq!(
        facts.rounds[0].diagnostics[0].problem.kind,
        RoomProblem::Missing
    );
    setup.entity = parser::parse_work_item(
        &setup.root.join("legacy.md"),
        &["implementation".into()],
        None,
    )
    .unwrap();
    assert!(setup.load().rounds.is_empty());
}
#[test]
fn malformed_log_preserves_only_trusted_prefix_and_invalid_edges_are_not_targets() {
    let setup = Setup::new();
    let original = fs::read(setup.room().join("briefing.review.jsonl")).unwrap();
    let first = original.split_inclusive(|b| *b == b'\n').next().unwrap();
    for tail in [b"{\n".as_slice(), b"\xff\n", b"{}", first] {
        setup.log(&[first, tail].concat());
        let facts = setup.load();
        let r = &facts.rounds[0];
        assert_eq!(r.records.len(), 1);
        assert_eq!(r.completeness, ReviewCompleteness::Invalid);
        assert_eq!(r.diagnostics[0].line, Some(2));
    }
    for record in [
        serde_json::json!({"type":"Alien","id":"new","briefing":"briefing:3j:implementation:round-1","by":"actor","at":"2026-07-20T01:00:00Z"}),
        serde_json::json!({"type":"Annotation","id":"new","briefing":"another-briefing","by":"actor","at":"2026-07-20T01:00:00Z"}),
        serde_json::json!({"type":"Annotation","id":"new","briefing":"briefing:3j:implementation:round-1","by":"actor","at":"2026-07-20T01:00:00Z","body":false}),
        serde_json::json!({"type":"Annotation","id":"new","briefing":"briefing:3j:implementation:round-1","by":"actor","at":"2026-07-20T01:00:00Z","includes":["missing"]}),
    ] {
        let mut bytes = first.to_vec();
        bytes.extend(serde_json::to_vec(&record).unwrap());
        bytes.push(b'\n');
        bytes.extend(&original[first.len()..]);
        setup.log(&bytes);
        let facts = setup.load();
        assert_eq!(facts.rounds[0].records.len(), 1);
        assert_eq!(facts.rounds[0].completeness, ReviewCompleteness::Invalid);
    }
    let facts = setup.load();
    assert_eq!(
        facts.rounds[0].invalid_relationships[0].validation,
        RelationshipValidation::Invalid
    );
    assert!(facts
        .correction_record(&facts.rounds[0].invalid_relationships[0].target)
        .is_none());
    setup.log(first);
    let facts = setup.load();
    assert_eq!(facts.rounds[0].completeness, ReviewCompleteness::Incomplete);
    assert_eq!(
        facts.correction_relationships(&facts.rounds[0].records[0].identity),
        CorrectionRelationships::NoRecordedRelationship
    );
    setup.log(b"");
    assert!(setup.load().rounds[0].records.is_empty());
}
#[test]
fn round_answered_is_not_approval_and_reason_requires_annotation() {
    let setup = Setup::new();
    let original = fs::read_to_string(setup.room().join("briefing.review.jsonl")).unwrap();
    let mut lines = original
        .lines()
        .map(|s| serde_json::from_str::<serde_json::Value>(s).unwrap())
        .collect::<Vec<_>>();
    let write = |lines: &[serde_json::Value]| {
        setup.log(
            lines
                .iter()
                .map(|v| format!("{v}\n"))
                .collect::<String>()
                .as_bytes(),
        )
    };
    write(&lines[..3]);
    assert_eq!(
        setup.load().rounds[0].completeness,
        ReviewCompleteness::Incomplete
    );
    write(&lines[..4]);
    assert_eq!(
        setup.load().rounds[0].completeness,
        ReviewCompleteness::Incomplete
    );
    lines[2]["decision"] = "approve".into();
    write(&lines[..3]);
    assert_eq!(
        setup.load().rounds[0].completeness,
        ReviewCompleteness::Answered
    );
    lines[0].as_object_mut().unwrap().remove("body");
    lines[0]["at"] = "2026-07-20T09:00:00.123456789+08:00".into();
    write(&lines[..1]);
    assert!(
        matches!(&setup.load().rounds[0].records[0].entry, ReviewEntry::Annotation(a) if a.body.is_none())
    );
    lines[0]["at"] = "2026-02-30T01:00:00Z".into();
    write(&lines[..1]);
    assert!(setup.load().rounds[0].records.is_empty());
    write(&lines[2..3]); // Approve Resolution has includes not present earlier: rejected.
    assert!(setup.load().rounds[0].records.is_empty());
}
#[test]
fn briefing_binding_failures_do_not_leak_unbound_records_and_missing_artifact_keeps_verdict() {
    let mut setup = Setup::new();
    let bytes = fs::read(setup.room().join("briefing.json")).unwrap();
    fs::write(setup.room().join("briefing.json"), b"{\"id\":1,\"id\":2}").unwrap();
    assert_eq!(
        setup.load().rounds[0].diagnostics[0].problem.kind,
        RoomProblem::InvalidJson
    );
    fs::write(setup.room().join("briefing.json"), &bytes).unwrap();
    let facts = setup.load();
    let round = &facts.rounds[0];
    let item = &round.briefing.as_ref().unwrap().items[0];
    fs::remove_file(setup.root.join("sample/candidate.patch")).unwrap();
    assert_eq!(
        setup.load().rounds[0].decision(),
        Some(GateDecision::Revise)
    );
    assert_eq!(
        history::read_item(&setup.definition, &setup.entity, round, item)
            .unwrap_err()
            .kind,
        RoomProblem::Missing
    );
    let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    value["question"] = "replacement".into();
    fs::write(
        setup.room().join("briefing.json"),
        serde_json::to_vec(&value).unwrap(),
    )
    .unwrap();
    assert_eq!(
        setup.load().rounds[0].diagnostics[0].problem.kind,
        RoomProblem::BriefingDigestMismatch
    );
    setup.briefing(value);
    assert!(history::read_item(&setup.definition, &setup.entity, round, item).is_err());
    fs::remove_file(setup.room().join("briefing.review.jsonl")).unwrap();
    assert!(setup.load().rounds[0].briefing.is_some());
}
#[test]
fn safe_paths_limits_and_discovery_failures_are_visible() {
    let setup = Setup::new();
    let parent = setup.root.join("sample");
    for uri in [
        "../../../../escape",
        "/etc/passwd",
        "file://etc/passwd",
        "../%2e/escape",
        "../../../index.md",
        "a\\b",
    ] {
        assert!(
            history::artifact_path(&parent, &setup.room(), uri).is_err(),
            "{uri}"
        );
    }
    fs::write(
        setup.room().join("briefing.review.jsonl"),
        vec![b'x'; 2 * 1024 * 1024 + 1],
    )
    .unwrap();
    assert_eq!(
        setup.load().rounds[0].diagnostics[0].problem.kind,
        RoomProblem::SizeLimit
    );
    for name in ["round-0", "round-01", "round-4294967296"] {
        fs::create_dir(setup.root.join("sample/review/implementation").join(name)).unwrap();
    }
    assert_eq!(setup.load().diagnostics.len(), 3);
    for cycle in 20..(history::MAX_ROUNDS as u32 + 30) {
        fs::create_dir(
            setup
                .root
                .join(format!("sample/review/implementation/round-{cycle}")),
        )
        .unwrap();
    }
    let facts = setup.load();
    assert!(facts.rounds.len() <= history::MAX_ROUNDS);
    assert!(facts
        .diagnostics
        .iter()
        .any(|d| d.problem.kind == RoomProblem::SizeLimit));
}
#[cfg(unix)]
#[test]
fn symlinks_nonregular_and_directory_replacement_are_refused() {
    use std::os::unix::fs::symlink;
    let setup = Setup::new();
    let facts = setup.load();
    let round = &facts.rounds[0];
    let item = &round.briefing.as_ref().unwrap().items[0];
    let artifact = setup.root.join("sample/candidate.patch");
    fs::remove_file(&artifact).unwrap();
    symlink(fixtures().join("upstream/candidate.patch"), &artifact).unwrap();
    assert!(history::read_item(&setup.definition, &setup.entity, round, item).is_err());
    fs::remove_file(artifact).unwrap();
    fs::create_dir(setup.root.join("sample/candidate.patch")).unwrap();
    assert_eq!(
        history::read_item(&setup.definition, &setup.entity, round, item)
            .unwrap_err()
            .kind,
        RoomProblem::Nonregular
    );
    let moved = setup.root.join("old-round");
    fs::rename(setup.room(), &moved).unwrap();
    symlink(&moved, setup.room()).unwrap();
    assert!(setup.load().rounds[0].briefing.is_none());
    fs::remove_file(setup.room()).unwrap();
    fs::rename(moved, setup.room()).unwrap();
    let stage = setup.root.join("sample/review/implementation");
    fs::rename(&stage, setup.root.join("old-stage")).unwrap();
    symlink(setup.root.join("old-stage"), &stage).unwrap();
    assert!(!setup.load().diagnostics.is_empty());
}
#[test]
fn flat_folder_archive_split_root_and_worktree_origins_remain_distinct() {
    let mut setup = Setup::new();
    // Displayed worktree body path is not evidence authority.
    setup.entity.path = setup.root.join(".worktrees/other/sample/index.md");
    assert_eq!(
        setup.load().rounds[0].decision(),
        Some(GateDecision::Revise)
    );
    let state = setup.root.join("state");
    fs::create_dir(&state).unwrap();
    copy_tree(&setup.root.join("sample"), &state.join("_archive/sample"));
    setup.entity.evidence_origin = Some(state.join("_archive/sample/index.md"));
    setup.definition.storage = WorkflowStorage::SplitRoot {
        entity_dir: state.clone(),
        expected_branch: "state".into(),
        disposition: StateCheckoutDisposition::Missing,
    };
    assert_eq!(setup.load().rounds.len(), 4);
    setup.definition.storage = WorkflowStorage::SingleRoot;
    copy_tree(
        &setup.root.join("sample/review"),
        &setup.root.join("review"),
    );
    setup.entity.evidence_origin = Some(setup.root.join("flat.md"));
    assert_eq!(setup.load().rounds.len(), 4);
    setup.entity.id = "unrelated".into();
    setup.entity.review_round = ReviewPointer::Absent;
    assert!(setup
        .load()
        .rounds
        .iter()
        .all(|r| r.briefing.is_none() && r.records.is_empty()));
}
fn git(root: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().into()
}
#[test]
fn exact_git_blob_not_dirty_working_bytes_and_read_only_snapshot() {
    let mut setup = Setup::new();
    git(&setup.root, &["init", "-q"]);
    git(&setup.root, &["add", "."]);
    git(
        &setup.root,
        &[
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
            "commit",
            "-qm",
            "fixture",
        ],
    );
    let commit = git(&setup.root, &["rev-parse", "HEAD"]);
    let mut value: serde_json::Value =
        serde_json::from_slice(&fs::read(setup.room().join("briefing.json")).unwrap()).unwrap();
    value["artifacts"][0]["uri"] =
        format!("git-root://main/{commit}/sample/candidate.patch").into();
    setup.briefing(value);
    fs::write(
        setup.root.join("sample/candidate.patch"),
        "dirty working bytes",
    )
    .unwrap();
    let before = git(&setup.root, &["diff", "--binary"]);
    let facts = setup.load();
    let round = &facts.rounds[0];
    assert_eq!(
        history::read_item(
            &setup.definition,
            &setup.entity,
            round,
            &round.briefing.as_ref().unwrap().items[0]
        )
        .unwrap(),
        fs::read(fixtures().join("upstream/candidate.patch")).unwrap()
    );
    assert_eq!(git(&setup.root, &["diff", "--binary"]), before);
}

#[test]
fn entry_limit_and_unreadable_briefing_report_explicit_failure() {
    let setup = Setup::new();
    let stage = setup.root.join("sample/review/implementation");
    for n in 0..history::MAX_ENTRIES {
        fs::write(stage.join(format!("entry-{n}")), "").unwrap();
    }
    assert!(setup
        .load()
        .diagnostics
        .iter()
        .any(|d| d.problem.kind == RoomProblem::SizeLimit));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let path = setup.room().join("briefing.json");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o0)).unwrap();
        assert!(setup.load().rounds[0].briefing.is_none());
        fs::set_permissions(path, fs::Permissions::from_mode(0o644)).unwrap();
    }
}
