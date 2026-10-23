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
fn refreshed_report_distinguishes_removed_item_from_untrusted_briefing() {
    use crate::app::{App, AppMode};
    use crossterm::event::{KeyEvent, KeyModifiers};
    use ratatui::{backend::TestBackend, Terminal};

    for failure in ["removed", "binding", "digest"] {
        let (_temp, mut snapshot) = tests::setup();
        let root = snapshot.definition.root.clone();
        fs::write(
            root.join("room/support.md"),
            "Distinct supporting evidence.\n",
        )
        .unwrap();
        rebind(&mut snapshot, |value| {
            let mut support = value["artifacts"][0].clone();
            support["id"] = "artifact:support".into();
            support["uri"] = "support.md".into();
            support["rev"] =
                spacetop_core::parser::gate_room::raw_digest(b"Distinct supporting evidence.\n")
                    .into();
            value["artifacts"]
                .as_array_mut()
                .unwrap()
                .insert(0, support);
        });
        let mut app = App::from_snapshot(root.clone(), snapshot.clone());
        for key in [KeyCode::Char('B'), KeyCode::Tab, KeyCode::Char('r')] {
            app.handle_key(KeyEvent::new(key, KeyModifiers::NONE));
        }
        let AppMode::GateRoom { browser, .. } = app.mode() else {
            panic!()
        };
        assert_eq!(
            browser.preview.as_deref(),
            Some("Exact candidate review.\n")
        );
        let selected = browser.selected.clone();
        rebind(&mut snapshot, |value| {
            value["artifacts"]
                .as_array_mut()
                .unwrap()
                .retain(|item| item["id"] != "artifact:gate-review");
        });
        if failure != "removed" {
            let GateData::Valid { document, .. } = &mut snapshot.items[0].gates else {
                panic!()
            };
            let attempt = &mut document.records[0].attempts[1];
            if failure == "binding" {
                // A request-backed binding must name a directory, not a Briefing file.
                attempt.briefing.room_ref = "room/index.json".into();
                attempt.briefing.request_digest = Some(attempt.briefing.digest.clone());
            } else {
                attempt.briefing.digest = format!("sha256:{}", "0".repeat(64));
            }
        }
        app.reload_from_snapshot(snapshot);
        let AppMode::GateRoom { browser, .. } = app.mode() else {
            panic!()
        };
        assert_eq!(browser.selected, selected);
        assert_eq!(browser.focus, RoomFocus::Items);
        assert!(browser.preview.is_none());
        assert!(browser.preview_item.is_none());
        assert_eq!(browser.view.verified(), failure == "removed");
        let content = browser.content();
        assert!(content.contains("Recorded Approve"));
        assert!(!content.contains("Distinct supporting evidence."));
        let mut terminal = Terminal::new(TestBackend::new(180, 50)).unwrap();
        terminal
            .draw(|frame| crate::ui::render(frame, &app))
            .unwrap();
        let rendered: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(rendered.contains("Recorded Approve"));
        assert!(!rendered.contains("Item SHA-256 verified"));
        if failure == "removed" {
            assert!(content.contains("SelectedItemMissing"));
            assert!(rendered.contains("Bound Briefing verified"));
            assert!(rendered.contains("Should this exact candidate"));
            assert!(rendered.contains("selected item is no longer in the verified inventory"));
            assert!(!content.contains("cannot be trusted"));
            assert!(!rendered.contains("cannot be trusted"));
            assert!(content.contains("Main report not identified"));
            app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
            let AppMode::GateRoom { browser, .. } = app.mode() else {
                panic!()
            };
            assert_eq!(browser.preview_item.as_deref(), Some("artifact:support"));
            assert_eq!(
                browser.preview.as_deref(),
                Some("Distinct supporting evidence.\n")
            );
        } else {
            assert!(rendered.contains("UNVERIFIED"));
            assert!(rendered.contains("Question unavailable"));
            assert!(rendered.contains("cannot be trusted"));
            assert!(!rendered.contains("Bound Briefing verified"));
            assert!(content.contains(if failure == "binding" {
                "BindingMismatch"
            } else {
                "BriefingDigestMismatch"
            }));
            assert!(!content.contains("SelectedItemMissing"));
        }
    }
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
        RoomProblem::SelectedItemMissing,
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
