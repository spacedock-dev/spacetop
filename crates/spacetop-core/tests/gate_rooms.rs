use spacetop_core::{
    domain::*,
    gate_room,
    parser::{self, gate_room as decode},
};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/gate_rooms")
}
struct Setup {
    _temp: tempfile::TempDir,
    root: PathBuf,
    definition: WorkflowDefinition,
    entity: Entity,
    record: GateRecord,
}
impl Setup {
    fn new(origin: &str) -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(temp.path()).unwrap();
        let entity_path = root.join(origin);
        fs::create_dir_all(entity_path.parent().unwrap()).unwrap();
        fs::write(
            &entity_path,
            "---\nid: 3k\ntitle: Gate\nstatus: validation\n---\nBody",
        )
        .unwrap();
        let entity = parser::parse_work_item(&entity_path, &["validation".into()], None).unwrap();
        let mut definition =
            parser::parse_workflow_readme(&fixture().join("../durable-gates/README.md")).unwrap();
        definition.root = root.clone();
        definition.storage = WorkflowStorage::SingleRoot;
        let room = entity_path.parent().unwrap().join("room");
        fs::create_dir(&room).unwrap();
        for file in [
            "index.json",
            "gate-review.md",
            "entity-snapshot.md",
            "contract-snapshot.md",
        ] {
            fs::copy(fixture().join(file), room.join(file)).unwrap();
        }
        let bytes = fs::read(room.join("index.json")).unwrap();
        let attempt = GateAttempt {
            id: "gate-attempt:3k-validation-1".into(),
            briefing: GateBriefing {
                id: "briefing:docs-dev:3k:validation:attempt-1:revision-1".into(),
                digest: decode::canonical_digest(&bytes).unwrap(),
                request_digest: None,
                room_ref: "./room".into(),
            },
            withdrawal: None,
            resolution: None,
            application: None,
        };
        let record = GateRecord {
            id: "gate:docs-dev:3k:validation".into(),
            stage: "validation".into(),
            attempts: vec![attempt],
        };
        Self {
            _temp: temp,
            root,
            definition,
            entity,
            record,
        }
    }
    fn room(&self) -> PathBuf {
        self.entity.path.parent().unwrap().join("room")
    }
    fn attempt(&self) -> &GateAttempt {
        &self.record.attempts[0]
    }
    fn view(&self) -> GateRoomView {
        gate_room::load(&self.definition, &self.entity, &self.record, self.attempt())
    }
    fn read(&self) -> Result<Vec<u8>, RoomDiagnostic> {
        gate_room::read_item(
            &self.definition,
            &self.entity,
            &self.record,
            self.attempt(),
            "artifact:gate-review",
        )
    }
    fn replace_manifest(&mut self, v: serde_json::Value) {
        let bytes = serde_json::to_vec(&v).unwrap();
        fs::write(self.room().join("index.json"), &bytes).unwrap();
        self.record.attempts[0].briefing.digest = decode::canonical_digest(&bytes).unwrap();
    }
    fn request(&mut self) {
        let a = self.attempt();
        let v = serde_json::json!({"type":"spacedock-gate-presentation-request","version":"1","gate":self.record.id,"attempt":a.id,"briefing":{"locator":"gate-briefing.json","id":a.briefing.id,"digest":a.briefing.digest},"actor":"person:captain","approver":"person:captain"});
        let bytes = serde_json::to_vec(&v).unwrap();
        fs::write(self.room().join("request.json"), &bytes).unwrap();
        fs::rename(
            self.room().join("index.json"),
            self.room().join("gate-briefing.json"),
        )
        .unwrap();
        self.record.attempts[0].briefing.request_digest =
            Some(decode::canonical_digest(&bytes).unwrap());
    }
}
#[test]
fn formats_origins_history_and_order_do_not_require_request() {
    assert_eq!(
        decode::canonical_digest(&fs::read(fixture().join("upstream-briefing.json")).unwrap())
            .unwrap(),
        "sha256:20bff726e2328f30c8f6576fdc347d07f582d28a3580bf2d63ebbd0d951ed2c0"
    );
    for origin in [
        "entity.md",
        "folder/index.md",
        "_archive/entity.md",
        "_archive/folder/index.md",
    ] {
        let mut s = Setup::new(origin);
        s.entity.status = "done".into();
        let view = s.view();
        assert!(view.verified(), "{view:?}");
        assert_eq!(view.format, Some(RoomFormat::Current));
        let b = view.briefing.unwrap();
        assert_eq!(
            b.items.iter().map(|i| i.id.as_str()).collect::<Vec<_>>(),
            [
                "artifact:gate-review",
                "reference:entity-snapshot",
                "reference:recorder-contract"
            ]
        );
        assert_eq!(s.read().unwrap(), b"Exact candidate review.\n");
        fs::rename(s.room().join("index.json"), s.room().join("briefing.json")).unwrap();
        assert_eq!(s.view().format, Some(RoomFormat::Retained));
        fs::rename(s.room().join("briefing.json"), s.room().join("index.json")).unwrap();
        s.request();
        assert_eq!(s.view().format, Some(RoomFormat::RequestBacked));
        fs::remove_file(s.room().join("request.json")).unwrap();
        assert!(!s.view().verified());
        s.record.attempts[0].briefing.request_digest = None;
        assert!(s.view().verified());
        s.record.attempts[0].briefing.room_ref = "room/gate-briefing.json".into();
        assert_eq!(s.view().format, Some(RoomFormat::ExactFile));
    }
}
#[test]
fn precedence_and_binding_fail_closed_without_hiding_recorded_decision() {
    let mut s = Setup::new("e.md");
    fs::copy(
        s.room().join("index.json"),
        s.room().join("gate-briefing.json"),
    )
    .unwrap();
    fs::write(s.room().join("index.json"), "{}").unwrap();
    assert!(!s.view().verified());
    fs::copy(fixture().join("index.json"), s.room().join("index.json")).unwrap();
    s.record.stage = "plan".into();
    assert_eq!(s.view().diagnostics[0].kind, RoomProblem::IdentityMismatch);
    s.record.stage = "validation".into();
    let mut v: serde_json::Value =
        serde_json::from_slice(&fs::read(s.room().join("index.json")).unwrap()).unwrap();
    v["question"] = "tampered".into();
    fs::write(s.room().join("index.json"), serde_json::to_vec(&v).unwrap()).unwrap();
    assert_eq!(
        s.view().diagnostics[0].kind,
        RoomProblem::BriefingDigestMismatch
    );
    s.replace_manifest(v.clone());
    v["artifacts"][0]["id"] = "reference:recorder-contract".into();
    s.replace_manifest(v);
    assert_eq!(s.view().diagnostics[0].kind, RoomProblem::InvalidSchema);
    fs::copy(fixture().join("index.json"), s.room().join("index.json")).unwrap();
    s.record.attempts[0].briefing.digest =
        decode::canonical_digest(&fs::read(s.room().join("index.json")).unwrap()).unwrap();
    s.request();
    let mut request: serde_json::Value =
        serde_json::from_slice(&fs::read(s.room().join("request.json")).unwrap()).unwrap();
    request["attempt"] = "wrong".into();
    let bytes = serde_json::to_vec(&request).unwrap();
    fs::write(s.room().join("request.json"), &bytes).unwrap();
    assert_eq!(
        s.view().diagnostics[0].kind,
        RoomProblem::RequestDigestMismatch
    );
    s.record.attempts[0].briefing.request_digest = Some(decode::canonical_digest(&bytes).unwrap());
    assert_eq!(s.view().diagnostics[0].kind, RoomProblem::BindingMismatch);
}
#[test]
fn paths_schemes_limits_and_artifact_revisions_are_not_verified() {
    let mut s = Setup::new("e.md");
    for path in [
        "../outside",
        "/etc/passwd",
        "room/../room",
        "room\\index.json",
        "room%2findex.json",
        "room/./index.json",
    ] {
        s.record.attempts[0].briefing.room_ref = path.into();
        assert!(!s.view().verified(), "{path}");
    }
    s.record.attempts[0].briefing.room_ref = "provider:opaque".into();
    assert_eq!(s.view().diagnostics[0].kind, RoomProblem::UnsupportedScheme);
    s.record.attempts[0].briefing.room_ref = "room".into();
    let mut v: serde_json::Value =
        serde_json::from_slice(&fs::read(s.room().join("index.json")).unwrap()).unwrap();
    for uri in [
        "../e.md",
        "/etc/passwd",
        "sub/../../e.md",
        "a%2fb",
        "https://example.com/evidence",
    ] {
        v["artifacts"][0]["uri"] = uri.into();
        s.replace_manifest(v.clone());
        assert!(s.read().is_err(), "{uri}");
    }
    v["artifacts"][0]["uri"] = "gate-review.md".into();
    s.replace_manifest(v);
    fs::write(s.room().join("gate-review.md"), "tampered").unwrap();
    assert_eq!(
        s.read().unwrap_err().kind,
        RoomProblem::ArtifactDigestMismatch
    );
    fs::write(
        s.room().join("gate-review.md"),
        vec![b'a'; decode::MAX_BYTES + 1],
    )
    .unwrap();
    assert_eq!(s.read().unwrap_err().kind, RoomProblem::SizeLimit);
}
#[cfg(unix)]
#[test]
fn directory_final_and_state_root_symlinks_never_return_outside_bytes() {
    use std::os::unix::fs::symlink;
    let mut s = Setup::new("e.md");
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("secret"), "secret").unwrap();
    fs::remove_file(s.room().join("gate-review.md")).unwrap();
    symlink(
        outside.path().join("secret"),
        s.room().join("gate-review.md"),
    )
    .unwrap();
    assert!(s.read().is_err());
    fs::rename(s.room(), s.root.join("old-room")).unwrap();
    symlink(s.root.join("old-room"), s.room()).unwrap();
    assert_eq!(s.view().diagnostics[0].kind, RoomProblem::Symlink);
    fs::remove_file(s.room()).unwrap();
    symlink(outside.path(), s.room()).unwrap();
    assert!(!s.view().verified());
    s.definition.storage = WorkflowStorage::SplitRoot {
        entity_dir: s.room(),
        expected_branch: "state".into(),
        disposition: StateCheckoutDisposition::Attached,
    };
    assert!(gate_room::safe_root(&s.definition).is_err());
    assert!(
        gate_room::local_dependencies(&s.definition, &s.entity, s.attempt(), &s.view()).is_empty()
    );
}
fn git(root: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args(args)
        .current_dir(root)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{:?} {}",
        args,
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().into()
}
#[cfg(unix)]
#[test]
fn immutable_git_blobs_reject_missing_tree_symlink_and_dirty_fallback() {
    let mut s = Setup::new("e.md");
    git(&s.root, &["init", "-q"]);
    git(&s.root, &["config", "user.name", "Fixture"]);
    git(
        &s.root,
        &["config", "user.email", "fixture@example.invalid"],
    );
    std::os::unix::fs::symlink("room/gate-review.md", s.root.join("linked.md")).unwrap();
    git(&s.root, &["add", "."]);
    git(&s.root, &["commit", "-qm", "fixture"]);
    let commit = git(&s.root, &["rev-parse", "HEAD"]);
    let mut v: serde_json::Value =
        serde_json::from_slice(&fs::read(s.room().join("index.json")).unwrap()).unwrap();
    v["artifacts"][0]["uri"] = format!("git-root://main/{commit}/room/gate-review.md").into();
    v["context"] = serde_json::json!([]);
    s.replace_manifest(v.clone());
    let status = git(&s.root, &["status", "--porcelain"]);
    let refs = git(&s.root, &["show-ref", "--head"]);
    fs::write(s.room().join("gate-review.md"), "dirty").unwrap();
    assert_eq!(s.read().unwrap(), b"Exact candidate review.\n");
    for path in [
        "linked.md",
        "room",
        "absent",
        "%2E%2E/secret",
        "room%2Fgate-review.md",
        "room/%67ate-review.md",
    ] {
        v["artifacts"][0]["uri"] = format!("git-root://main/{commit}/{path}").into();
        s.replace_manifest(v.clone());
        assert!(s.read().is_err(), "{path}");
    }
    v["artifacts"][0]["uri"] =
        format!("git-root://main/{}/room/gate-review.md", "1".repeat(40)).into();
    s.replace_manifest(v.clone());
    assert_eq!(s.read().unwrap_err().kind, RoomProblem::MissingObject);
    v["artifacts"][0]["uri"] = format!("git-root://state/{commit}/room/gate-review.md").into();
    s.replace_manifest(v);
    assert_eq!(s.read().unwrap_err().kind, RoomProblem::UnsupportedRoot);
    assert_eq!(git(&s.root, &["show-ref", "--head"]), refs);
    assert!(!status.is_empty());
}
#[test]
fn browsing_is_read_only_and_changed_or_removed_room_invalidates() {
    let s = Setup::new("e.md");
    let before = fs::read(s.entity.path.clone()).unwrap();
    let room_before = fs::read(s.room().join("index.json")).unwrap();
    let entries_before = fs::read_dir(s.room()).unwrap().count();
    assert!(s.view().verified());
    assert!(s.read().is_ok());
    assert_eq!(fs::read(&s.entity.path).unwrap(), before);
    assert_eq!(fs::read(s.room().join("index.json")).unwrap(), room_before);
    assert_eq!(fs::read_dir(s.room()).unwrap().count(), entries_before);
    fs::remove_file(s.room().join("index.json")).unwrap();
    assert!(!s.view().verified());
    assert!(s.read().is_err());
    assert_eq!(fs::read(&s.entity.path).unwrap(), before);
}

#[test]
#[ignore = "real notify gate-room and split-root reload verification"]
fn room_artifact_changes_removal_creation_and_state_replacement_refresh() {
    use spacetop_core::watcher::{WatcherConfig, WorkflowWatcher};
    use std::time::Duration;
    let mut s = Setup::new(".state/e.md");
    s.definition.storage = WorkflowStorage::SplitRoot {
        entity_dir: s.root.join(".state"),
        expected_branch: "state".into(),
        disposition: StateCheckoutDisposition::Detached,
    };
    let mut v: serde_json::Value =
        serde_json::from_slice(&fs::read(s.room().join("index.json")).unwrap()).unwrap();
    v["artifacts"][0]["uri"] = "evidence with spaces.txt".into();
    fs::copy(
        s.room().join("gate-review.md"),
        s.room().join("evidence with spaces.txt"),
    )
    .unwrap();
    s.replace_manifest(v);
    let (mut watcher, rx) = WorkflowWatcher::start(
        &s.root,
        WatcherConfig {
            debounce: Duration::from_millis(25),
        },
    )
    .unwrap();
    watcher.set_dependencies(gate_room::local_dependencies(
        &s.definition,
        &s.entity,
        s.attempt(),
        &s.view(),
    ));
    std::thread::sleep(Duration::from_millis(100));
    while rx.try_recv().is_ok() {}
    fs::write(s.room().join("evidence with spaces.txt"), "tampered").unwrap();
    rx.recv_timeout(Duration::from_secs(3)).unwrap();
    assert_eq!(
        s.read().unwrap_err().kind,
        RoomProblem::ArtifactDigestMismatch
    );
    while rx.try_recv().is_ok() {}
    fs::remove_file(s.room().join("index.json")).unwrap();
    rx.recv_timeout(Duration::from_secs(3)).unwrap();
    assert!(!s.view().verified());
    while rx.try_recv().is_ok() {}
    fs::copy(fixture().join("index.json"), s.room().join("index.json")).unwrap();
    s.record.attempts[0].briefing.digest =
        decode::canonical_digest(&fs::read(s.room().join("index.json")).unwrap()).unwrap();
    rx.recv_timeout(Duration::from_secs(3)).unwrap();
    assert!(s.view().verified());
    while rx.try_recv().is_ok() {}
    fs::rename(s.root.join(".state"), s.root.join("retired-state")).unwrap();
    fs::create_dir(s.root.join(".state")).unwrap();
    rx.recv_timeout(Duration::from_secs(3)).unwrap();
    assert!(!s.view().verified());
}

#[test]
fn distinct_state_git_history_is_readable_when_wrong_branch_or_detached() {
    let mut s = Setup::new(".state/e.md");
    git(&s.root, &["init", "-q"]);
    git(&s.root, &["config", "user.name", "Fixture"]);
    git(
        &s.root,
        &["config", "user.email", "fixture@example.invalid"],
    );
    fs::write(s.root.join("definition.md"), "definition").unwrap();
    git(&s.root, &["add", "definition.md"]);
    git(&s.root, &["commit", "-qm", "definition"]);
    let state = s.root.join(".state");
    git(&state, &["init", "-q"]);
    git(&state, &["config", "user.name", "Fixture"]);
    git(&state, &["config", "user.email", "fixture@example.invalid"]);
    git(&state, &["add", "."]);
    git(&state, &["commit", "-qm", "state"]);
    let commit = git(&state, &["rev-parse", "HEAD"]);
    let mut v: serde_json::Value =
        serde_json::from_slice(&fs::read(s.room().join("index.json")).unwrap()).unwrap();
    v["artifacts"][0]["uri"] = format!("git-root://state/{commit}/room/gate-review.md").into();
    v["context"] = serde_json::json!([]);
    s.replace_manifest(v);
    for detached in [false, true] {
        if detached {
            git(&state, &["checkout", "--detach", "-q"]);
        }
        s.definition.storage = spacetop_core::state_checkout::classify_storage(
            &spacetop_core::git::StdGitRunner,
            &s.root,
            Some(".state"),
            Some("expected-state"),
        );
        assert!(matches!(
            s.definition.storage,
            WorkflowStorage::SplitRoot {
                disposition: StateCheckoutDisposition::WrongBranch { .. }
                    | StateCheckoutDisposition::Detached,
                ..
            }
        ));
        assert!(s.view().verified());
        assert_eq!(s.read().unwrap(), b"Exact candidate review.\n");
    }
}

#[test]
fn malformed_inventory_request_locator_and_raw_json_are_rejected() {
    let mut s = Setup::new("e.md");
    let original: serde_json::Value =
        serde_json::from_slice(&fs::read(s.room().join("index.json")).unwrap()).unwrap();
    for mutation in 0..5 {
        let mut v = original.clone();
        match mutation {
            0 => v["context"][0]["summary"] = "forbidden".into(),
            1 => v["artifacts"] = serde_json::json!([]),
            2 => v["version"] = "2".into(),
            3 => v["artifacts"][0]["rev"] = "sha1:wrong".into(),
            _ => v["question"] = "   ".into(),
        }
        s.replace_manifest(v);
        assert_eq!(s.view().diagnostics[0].kind, RoomProblem::InvalidSchema);
    }
    s.replace_manifest(original);
    s.request();
    let request_path = s.room().join("request.json");
    let mut v: serde_json::Value =
        serde_json::from_slice(&fs::read(&request_path).unwrap()).unwrap();
    v["briefing"]["locator"] = "../outside.json".into();
    let bytes = serde_json::to_vec(&v).unwrap();
    fs::write(request_path, &bytes).unwrap();
    s.record.attempts[0].briefing.request_digest = Some(decode::canonical_digest(&bytes).unwrap());
    assert_eq!(s.view().diagnostics[0].kind, RoomProblem::UnsafePath);
    fs::write(
        s.room().join("request.json"),
        "{\"type\":\"x\",\"type\":\"y\"}",
    )
    .unwrap();
    assert_eq!(s.view().diagnostics[0].kind, RoomProblem::InvalidJson);
}

#[test]
fn worktree_body_never_rebases_main_gate_room_provenance() {
    let mut s = Setup::new("e.md");
    let other = tempfile::tempdir().unwrap();
    fs::write(other.path().join("e.md"), "worktree body").unwrap();
    fs::create_dir(other.path().join("room")).unwrap();
    fs::write(other.path().join("room/index.json"), "unrelated room").unwrap();
    s.entity.worktree_source = Some(other.path().join("e.md"));
    s.entity.body = "changed body".into();
    assert!(s.view().verified());
    assert_eq!(s.read().unwrap(), b"Exact candidate review.\n");
    let mut entity = s.entity.clone();
    entity.gates = GateData::Valid {
        document: GateDocument {
            version: 1,
            records: vec![s.record.clone()],
        },
        warnings: Vec::new(),
    };
    let index = spacetop_core::index::WorkflowIndex::from_sources(
        spacetop_core::sources::WorkflowSources {
            active: WorkflowSnapshot {
                definition: s.definition.clone(),
                items: vec![entity.clone()],
                parse_errors: Vec::new(),
            },
            archive: spacetop_core::sources::ArchiveSnapshot::empty(),
        },
    );
    assert_eq!(index.gate_attempts(&entity).len(), 1);
    assert!(index
        .gate_room(&entity, &s.record.id, &s.attempt().id)
        .verified());
    assert!(!index
        .gate_room(&entity, &s.record.id, "another-attempt")
        .verified());
}
