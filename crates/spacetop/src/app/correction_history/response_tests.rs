use super::*;
use std::fs;

fn setup() -> (tempfile::TempDir, OverviewState, HistoryBrowser) {
    let (temp, snapshot) = tests::setup();
    let state = OverviewState::from_snapshot(snapshot.definition.root.clone(), snapshot);
    let mut browser = HistoryBrowser::default();
    browser.refresh(
        state
            .index()
            .correction_rounds(&state.selected_item().unwrap()),
    );
    (temp, state, browser)
}

#[test]
fn response_filter_follows_only_recorded_inverse_edges_and_back_restores_exact_edge() {
    let (_temp, state, mut browser) = setup();
    let path = state.selected_item().unwrap().path;
    browser.key(KeyCode::Enter, &state, &path);
    let source = browser.stack.last().unwrap().clone();
    browser.key(KeyCode::Char('f'), &state, &path);
    assert_eq!(browser.edges().len(), 1);
    let edge = browser.relationship.clone();
    assert!(browser.content().contains("Recorded response available"));
    browser.key(KeyCode::Enter, &state, &path);
    assert_eq!(
        browser.stack.last().unwrap().id,
        "annotation:decline-duplicate-member"
    );
    browser.key(KeyCode::Enter, &state, &path);
    assert_eq!(
        browser.stack.last().unwrap().id,
        "resolution:ensign-3j-round-1"
    );
    browser.key(KeyCode::Esc, &state, &path);
    browser.key(KeyCode::Esc, &state, &path);
    assert_eq!(browser.stack.last(), Some(&source));
    assert_eq!(browser.relationship, edge);
    assert!(browser.responses_only);
    assert_eq!(
        browser.selected_round().unwrap().provenance,
        RoundProvenance::CurrentPointer
    );
}

#[test]
fn no_response_unknown_and_unrelated_rounds_stay_distinct() {
    let (_temp, state, mut browser) = setup();
    let path = state.selected_item().unwrap().path;
    browser.key(KeyCode::Down, &state, &path);
    assert_eq!(browser.round.as_ref().unwrap().cycle, 2);
    browser.key(KeyCode::Enter, &state, &path);
    browser.key(KeyCode::Char('f'), &state, &path);
    assert!(browser.edges().is_empty());
    assert!(browser
        .content()
        .contains("No recorded response: unresolved in this log"));
    let source = browser.stack.clone();
    browser.key(KeyCode::Enter, &state, &path);
    assert_eq!(browser.stack, source);
    browser.key(KeyCode::Esc, &state, &path);
    browser.key(KeyCode::Down, &state, &path);
    assert_eq!(browser.round.as_ref().unwrap().cycle, 10);
    browser.key(KeyCode::Enter, &state, &path);
    assert!(browser.content().contains("additional responses unknown"));
    let log = path
        .parent()
        .unwrap()
        .join("review/implementation/round-10/briefing.review.jsonl");
    let original = fs::read_to_string(&log).unwrap();
    fs::write(
        log,
        original.lines().take(2).collect::<Vec<_>>().join("\n") + "\n{",
    )
    .unwrap();
    browser.refresh(
        state
            .index()
            .correction_rounds(&state.selected_item().unwrap()),
    );
    assert!(browser.content().contains("Response unknown:"));
}

#[test]
fn multiple_responses_survive_refresh_and_dangling_relationship_does_not_open() {
    let (_temp, state, mut browser) = setup();
    let path = state.selected_item().unwrap().path;
    let log = path
        .parent()
        .unwrap()
        .join("review/implementation/round-1/briefing.review.jsonl");
    let original = fs::read_to_string(&log).unwrap();
    let mut rows = original
        .lines()
        .map(|l| serde_json::from_str::<serde_json::Value>(l).unwrap())
        .collect::<Vec<_>>();
    let mut second = rows[3].clone();
    second["id"] = "annotation:second-response".into();
    rows.insert(4, second);
    fs::write(
        &log,
        rows.iter().map(|r| format!("{r}\n")).collect::<String>(),
    )
    .unwrap();
    browser.refresh(
        state
            .index()
            .correction_rounds(&state.selected_item().unwrap()),
    );
    browser.key(KeyCode::Enter, &state, &path);
    browser.key(KeyCode::Char('f'), &state, &path);
    assert_eq!(browser.edges().len(), 2);
    browser.key(KeyCode::Down, &state, &path);
    let selected = browser.relationship.clone();
    browser.refresh(
        state
            .index()
            .correction_rounds(&state.selected_item().unwrap()),
    );
    assert_eq!(browser.relationship, selected);
    browser.key(KeyCode::Enter, &state, &path);
    assert_eq!(
        browser.stack.last().unwrap().id,
        "annotation:second-response"
    );
    browser.key(KeyCode::Esc, &state, &path);
    assert_eq!(browser.relationship, selected);
    // Invalid includes stays visible but cannot be followed, even when the ID exists elsewhere.
    let mut bad = rows[3].clone();
    bad["id"] = "annotation:dangling".into();
    bad["includes"] = serde_json::json!(["annotation:missing-target"]);
    rows.push(bad);
    fs::write(
        &log,
        rows.iter().map(|r| format!("{r}\n")).collect::<String>(),
    )
    .unwrap();
    browser.refresh(
        state
            .index()
            .correction_rounds(&state.selected_item().unwrap()),
    );
    assert!(browser.content().contains("additional responses unknown"));
    let round = browser.selected_round().unwrap();
    assert_eq!(round.invalid_relationships.len(), 1);
    let invalid = round.invalid_relationships[0].clone();
    browser.key(KeyCode::Esc, &state, &path);
    assert!(browser
        .content()
        .contains("Invalid includes (not navigable)"));
    assert!(browser.content().contains("annotation:missing-target"));
    assert!(!browser
        .selections()
        .contains(&HistorySelection::Record(invalid.source.clone())));
    browser.selection = Some(HistorySelection::Record(invalid.source));
    browser.key(KeyCode::Enter, &state, &path);
    assert!(browser.stack.is_empty());
    assert!(browser.notice.as_ref().unwrap().contains("not opened"));
}

#[test]
fn invalid_log_does_not_discredit_bound_question_or_call_a_resolution_a_correction() {
    let (_temp, state, mut browser) = setup();
    let path = state.selected_item().unwrap().path;
    browser.key(KeyCode::Down, &state, &path);
    browser.key(KeyCode::Down, &state, &path);
    let text = browser.content();
    assert!(text.starts_with("Question:"));
    assert!(text.contains("Review log is incomplete or invalid"));
    assert!(!text.contains("question and inventory cannot be trusted"));
    browser.key(KeyCode::Up, &state, &path);
    browser.key(KeyCode::Up, &state, &path);
    browser.key(KeyCode::Enter, &state, &path);
    browser.key(KeyCode::Char('f'), &state, &path);
    browser.key(KeyCode::Enter, &state, &path);
    browser.key(KeyCode::Enter, &state, &path);
    assert!(browser.content().contains("No later recorded response"));
    assert!(!browser.content().contains("unresolved in this log"));
}
