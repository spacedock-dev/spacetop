use super::*;
use spacetop_core::domain::*;
use std::fs;

fn rebind(snapshot: &mut WorkflowSnapshot, change: impl FnOnce(&mut serde_json::Value)) {
    let path = snapshot.definition.root.join("room/index.json");
    let mut value: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    change(&mut value);
    let bytes = serde_json::to_vec(&value).unwrap();
    fs::write(path, &bytes).unwrap();
    let GateData::Valid { document, .. } = &mut snapshot.items[0].gates else {
        panic!()
    };
    document.records[0].attempts[1].briefing.digest =
        spacetop_core::parser::gate_room::canonical_digest(&bytes).unwrap();
}

#[test]
fn direct_report_preserves_support_selection_and_refreshes_exact_preview_identity() {
    let (_temp, mut snapshot) = tests::setup();
    rebind(&mut snapshot, |value| {
        let mut support = value["artifacts"][0].clone();
        support["id"] = "artifact:support".into();
        value["artifacts"]
            .as_array_mut()
            .unwrap()
            .insert(0, support);
    });
    let state = OverviewState::from_snapshot(snapshot.definition.root.clone(), snapshot.clone());
    let mut browser = GateBrowser::new(&state).unwrap();
    browser.key(KeyCode::Tab, &state);
    assert_eq!(browser.item, 0);
    browser.key(KeyCode::Char('r'), &state);
    assert_eq!(
        browser.preview_item.as_deref(),
        Some("artifact:gate-review")
    );
    assert!(browser.content().starts_with("Question:"));
    assert!(browser.content().contains("Exact candidate review."));
    rebind(&mut snapshot, |value| {
        value["artifacts"].as_array_mut().unwrap().reverse()
    });
    let state = OverviewState::from_snapshot(snapshot.definition.root.clone(), snapshot);
    browser.refresh(&state);
    assert_eq!(
        browser.preview_item.as_deref(),
        Some("artifact:gate-review")
    );
    assert_eq!(browser.item, 1);
    browser.key(KeyCode::Esc, &state);
    assert_eq!(browser.focus, RoomFocus::Items);
    assert_eq!(browser.item, 1);
    browser.key(KeyCode::Enter, &state);
    assert_eq!(browser.preview_item.as_deref(), Some("artifact:support"));
}

#[test]
fn report_absence_failures_and_binary_bytes_never_claim_verified_preview() {
    for case in [
        "missing",
        "tampered",
        "briefing",
        "unidentified",
        "binary",
        "blank",
        "unsafe",
        "unsupported",
        "oversized",
    ] {
        let (_temp, mut snapshot) = tests::setup();
        let root = snapshot.definition.root.clone();
        match case {
            "missing" => fs::remove_file(root.join("room/gate-review.md")).unwrap(),
            "tampered" => fs::write(root.join("room/gate-review.md"), "changed").unwrap(),
            "briefing" => fs::write(root.join("room/index.json"), "{}").unwrap(),
            "unidentified" => rebind(&mut snapshot, |v| {
                v["artifacts"][0]["id"] = "artifact:other".into()
            }),
            "blank" => rebind(&mut snapshot, |v| v["question"] = " ".into()),
            "unsafe" => rebind(&mut snapshot, |v| {
                v["artifacts"][0]["uri"] = "../outside.md".into()
            }),
            "unsupported" => rebind(&mut snapshot, |v| {
                v["artifacts"][0]["uri"] = "https://example.invalid/report".into()
            }),
            "oversized" => fs::write(
                root.join("room/gate-review.md"),
                vec![b'x'; 2 * 1024 * 1024 + 1],
            )
            .unwrap(),
            "binary" => {
                fs::write(root.join("room/gate-review.md"), [255]).unwrap();
                rebind(&mut snapshot, |v| {
                    v["artifacts"][0]["rev"] =
                        "sha256:a8100ae6aa1940d0b663bb31cd466142ebbdbd5187131b92d93818987832eb89"
                            .into()
                });
            }
            _ => unreachable!(),
        }
        let state = OverviewState::from_snapshot(root, snapshot);
        let mut browser = GateBrowser::new(&state).unwrap();
        if case == "unidentified" {
            browser.key(KeyCode::Tab, &state);
            browser.key(KeyCode::Enter, &state);
            assert!(browser.preview.is_some());
        }
        browser.key(KeyCode::Char('r'), &state);
        assert!(browser.preview.is_none(), "{case}");
        let text = browser.content();
        assert!(text.starts_with("Question"));
        assert!(text.contains("Recorded Approve"));
        let expected = match case {
            "missing" => "missing or unreadable",
            "tampered" => "do not match",
            "unidentified" => "Main report not identified",
            "binary" => "not UTF-8",
            "unsafe" => "Safe reading was refused",
            "unsupported" => "display-only",
            "oversized" => "safe size limit",
            _ => "Question unavailable",
        };
        assert!(text.contains(expected), "{case}: {text}");
    }
}

#[test]
fn every_typed_failure_has_plain_effect_and_exact_detail() {
    for kind in [
        RoomProblem::Missing,
        RoomProblem::UnsupportedRoot,
        RoomProblem::UnsupportedScheme,
        RoomProblem::UnsafePath,
        RoomProblem::Symlink,
        RoomProblem::InvalidJson,
        RoomProblem::InvalidSchema,
        RoomProblem::IdentityMismatch,
        RoomProblem::BindingMismatch,
        RoomProblem::RequestDigestMismatch,
        RoomProblem::BriefingDigestMismatch,
        RoomProblem::ArtifactDigestMismatch,
        RoomProblem::Nonregular,
        RoomProblem::SizeLimit,
        RoomProblem::Replaced,
        RoomProblem::MissingObject,
    ] {
        let d = RoomDiagnostic::new(kind, "exact original detail");
        let text = evidence_failure(&d);
        assert!(text.contains("Detail:"));
        assert!(text.ends_with(&d.to_string()));
        assert!(text.lines().next().unwrap().contains(' '));
    }
}
