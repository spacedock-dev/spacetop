use spacetop_core::domain::*;

#[test]
fn canonical_report_uses_artifact_identity_not_order_filename_or_reference() {
    let item = |kind, id: &str| RoomItem {
        kind,
        id: id.into(),
        uri: "gate-review.md".into(),
        revision: "pinned".into(),
        summary: None,
    };
    let mut view = GateRoomView {
        format: Some(RoomFormat::Current),
        diagnostics: vec![],
        briefing: Some(CanonicalBriefing {
            id: "briefing".into(),
            question: "Proceed?".into(),
            items: vec![
                item(RoomItemKind::Artifact, "other"),
                item(RoomItemKind::Artifact, "artifact:gate-review"),
            ],
        }),
    };
    assert!(
        matches!(view.main_report(), MainReport::Identified(i) if i.id == "artifact:gate-review")
    );
    view.briefing.as_mut().unwrap().items.reverse();
    assert!(
        matches!(view.main_report(), MainReport::Identified(i) if i.id == "artifact:gate-review")
    );
    view.briefing.as_mut().unwrap().items[0].kind = RoomItemKind::Reference;
    assert_eq!(view.main_report(), MainReport::NotIdentified);
    view.diagnostics.push(RoomDiagnostic::new(
        RoomProblem::BriefingDigestMismatch,
        "changed",
    ));
    assert_eq!(view.main_report(), MainReport::Unavailable);
    view.briefing = None;
    assert_eq!(view.main_report(), MainReport::Unavailable);
}
