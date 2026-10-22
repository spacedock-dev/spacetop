//! Advisory rounds share strict primitives with gates, not gate authority rules.
use super::gate_room::{add_item, canonical_digest, digest_valid, strict_json, MAX_BYTES};
use crate::domain::*;
use serde_json::Value;
use std::collections::HashSet;

fn schema(message: impl Into<String>) -> RoomDiagnostic {
    RoomDiagnostic::new(RoomProblem::InvalidSchema, message)
}
pub fn stage_valid(stage: &str) -> bool {
    matches!(stage.as_bytes().first(), Some(b'a'..=b'z'))
        && stage
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}
pub fn cycle_number(value: &str) -> Option<u32> {
    if !matches!(value.as_bytes().first(), Some(b'1'..=b'9'))
        || !value.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    value.parse().ok()
}
pub fn briefing_id(entity: &str, stage: &str, cycle: u32) -> String {
    format!("briefing:{entity}:{stage}:round-{cycle}")
}
pub fn parse_pointer(frontmatter: &str, entity: &str) -> ReviewPointer {
    let root = match serde_yaml::from_str::<super::gates::UniqueValue>(frontmatter) {
        Ok(root) => root.0,
        Err(error) => {
            let absent =
                super::frontmatter::top_level_scalar_entries(frontmatter).is_some_and(|entries| {
                    entries.iter().all(|(key, _)| {
                        serde_yaml::from_str::<String>(key).is_ok_and(|k| k != "review-round")
                    })
                });
            return if absent {
                ReviewPointer::Absent
            } else {
                ReviewPointer::Invalid {
                    diagnostic: error.to_string(),
                }
            };
        }
    };
    let Some(value) = root.get("review-round") else {
        return ReviewPointer::Absent;
    };
    let decoded = serde_yaml::from_value::<RoundPointer>(value.clone())
        .map_err(|e| e.to_string())
        .and_then(|p| {
            if !stage_valid(&p.stage)
                || p.cycle == 0
                || p.id != format!("round:{entity}:{}:{}", p.stage, p.cycle)
                || p.briefing.id != briefing_id(entity, &p.stage, p.cycle)
                || p.briefing.room_ref != format!("./review/{}/round-{}", p.stage, p.cycle)
                || !digest_valid(&p.briefing.digest)
            {
                Err("invalid review-round identity, stage, cycle, room-ref or digest".into())
            } else {
                Ok(p)
            }
        });
    match decoded {
        Ok(pointer) => ReviewPointer::Valid { pointer },
        Err(diagnostic) => ReviewPointer::Invalid { diagnostic },
    }
}
fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str, RoomDiagnostic> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| schema(format!("{key} must be nonempty text")))
}
fn optional_text(value: &Value, key: &str) -> Result<Option<String>, RoomDiagnostic> {
    match value.get(key) {
        None => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.clone())),
        _ => Err(schema(format!("{key} must be text"))),
    }
}
pub fn decode_briefing(
    bytes: &[u8],
    identity: &RoundIdentity,
    pointer: Option<&RoundPointer>,
) -> Result<(CanonicalBriefing, Vec<LabelReference>, String), RoomDiagnostic> {
    let value = strict_json(bytes)?;
    if text(&value, "type")? != "Briefing" || text(&value, "version")? != "1" {
        return Err(schema("requires Briefing v1"));
    }
    if text(&value, "id")? != identity.briefing_id {
        return Err(RoomDiagnostic::new(
            RoomProblem::IdentityMismatch,
            "Briefing entity/stage/cycle differs from room candidate",
        ));
    }
    let digest = canonical_digest(bytes)?;
    if pointer.is_some_and(|p| p.briefing.id != identity.briefing_id || p.briefing.digest != digest)
    {
        return Err(RoomDiagnostic::new(
            RoomProblem::BriefingDigestMismatch,
            "current pointer does not bind this Briefing digest",
        ));
    }
    let question = text(&value, "question")?.to_owned();
    let artifacts = value
        .get("artifacts")
        .and_then(Value::as_array)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| schema("requires artifacts"))?;
    let mut ids = HashSet::new();
    let mut items = Vec::new();
    for artifact in artifacts {
        add_item(artifact, RoomItemKind::Artifact, &mut ids, &mut items)?;
    }
    let mut references = Vec::new();
    if let Some(context) = value.get("context") {
        context_items(context, &mut ids, &mut items, &mut references, 0)?;
    }
    Ok((
        CanonicalBriefing {
            id: identity.briefing_id.clone(),
            question,
            items,
        },
        references,
        digest,
    ))
}
fn context_items(
    value: &Value,
    ids: &mut HashSet<String>,
    items: &mut Vec<RoomItem>,
    labels: &mut Vec<LabelReference>,
    depth: usize,
) -> Result<(), RoomDiagnostic> {
    if depth > 64 {
        return Err(RoomDiagnostic::new(
            RoomProblem::SizeLimit,
            "context depth exceeds 64",
        ));
    }
    if value.is_null() {
        return Ok(());
    }
    for node in value
        .as_array()
        .ok_or_else(|| schema("context must be an array"))?
    {
        if !node.is_object() {
            return Err(schema("context node must be an object"));
        }
        if text(node, "type")? == "Reference" {
            if node.get("uri").is_none() && node.get("rev").is_none() {
                let id = text(node, "id")?;
                let label = text(node, "label")?;
                if node.get("summary").is_some() || !ids.insert(id.into()) {
                    return Err(schema("duplicate/invalid label-only Reference"));
                }
                labels.push(LabelReference {
                    id: id.into(),
                    label: label.into(),
                });
            } else {
                add_item(node, RoomItemKind::Reference, ids, items)?;
            }
        }
        if let Some(children) = node.get("children") {
            context_items(children, ids, items, labels, depth + 1)?;
        }
    }
    Ok(())
}
// Preserve gate UTC rules while accepting RFC3339 offsets in advisory logs.
fn timestamp(value: &str) -> bool {
    if value.ends_with('Z') {
        return super::gates::utc_timestamp(value);
    }
    let Some(split) = value
        .len()
        .checked_sub(6)
        .filter(|&n| value.is_char_boundary(n))
    else {
        return false;
    };
    let (date, offset) = value.split_at(split);
    let b = offset.as_bytes();
    b.len() == 6
        && matches!(b[0], b'+' | b'-')
        && b[3] == b':'
        && b[1..3].iter().chain(&b[4..]).all(u8::is_ascii_digit)
        && offset[1..3].parse::<u8>().is_ok_and(|v| v < 24)
        && offset[4..].parse::<u8>().is_ok_and(|v| v < 60)
        && super::gates::utc_timestamp(&format!("{date}Z"))
}
fn record(
    bytes: &[u8],
    round: &RoundIdentity,
    ordinal: usize,
) -> Result<ReviewRecord, RoomDiagnostic> {
    let value = strict_json(bytes)?;
    let id = text(&value, "id")?.to_owned();
    let by = text(&value, "by")?.to_owned();
    let at = text(&value, "at")?.to_owned();
    if !timestamp(&at) {
        return Err(schema("at must be RFC3339 time"));
    }
    if text(&value, "briefing")? != round.briefing_id {
        return Err(RoomDiagnostic::new(
            RoomProblem::BindingMismatch,
            "record belongs to another Briefing",
        ));
    }
    let includes: Vec<String> = match value.get("includes") {
        None => Vec::new(),
        Some(v) => {
            serde_json::from_value(v.clone()).map_err(|_| schema("includes must be string IDs"))?
        }
    };
    let entry = match text(&value, "type")? {
        "Annotation" => ReviewEntry::Annotation(ReviewAnnotation {
            body: optional_text(&value, "body")?,
            target: optional_text(&value, "target")?,
            kind: optional_text(&value, "kind")?,
        }),
        "Resolution" => {
            let resolution: GateResolution =
                serde_json::from_value(value).map_err(|e| schema(e.to_string()))?;
            super::gates::validate_resolution(&resolution, &round.briefing_id, "review")
                .map_err(schema)?;
            ReviewEntry::Resolution(resolution)
        }
        _ => return Err(schema("unknown review record type")),
    };
    Ok(ReviewRecord {
        identity: ReviewRecordIdentity {
            round: round.clone(),
            id,
        },
        ordinal,
        by,
        at,
        includes,
        entry,
    })
}
pub fn decode_log(bytes: &[u8], round: &mut CorrectionRound) {
    let path = round.room.join("briefing.review.jsonl");
    let fail = |round: &mut CorrectionRound, line, problem| {
        round.completeness = ReviewCompleteness::Invalid;
        round.diagnostics.push(ReviewDiagnostic {
            path: path.clone(),
            line: Some(line),
            problem,
        });
    };
    if bytes.len() > MAX_BYTES {
        fail(
            round,
            1,
            RoomDiagnostic::new(RoomProblem::SizeLimit, "review log exceeds 2 MiB"),
        );
        return;
    }
    if bytes.is_empty() {
        fail(round, 1, schema("empty review log"));
        return;
    }
    for (index, line) in bytes.split_inclusive(|b| *b == b'\n').enumerate() {
        let ordinal = index + 1;
        if line.last() != Some(&b'\n') {
            fail(
                round,
                ordinal,
                schema("partial JSONL tail: newline required"),
            );
            return;
        }
        let entry = match record(line, &round.identity, ordinal) {
            Ok(entry) => entry,
            Err(e) => {
                fail(round, ordinal, e);
                return;
            }
        };
        if round
            .records
            .iter()
            .any(|r| r.identity.id == entry.identity.id)
        {
            fail(round, ordinal, schema("duplicate record ID"));
            return;
        }
        let mut invalid = false;
        for (i, target) in entry.includes.iter().enumerate() {
            if !round.records.iter().any(|r| &r.identity.id == target) {
                invalid = true;
                round.invalid_relationships.push(RecordedRelationship {
                    source: entry.identity.clone(),
                    target: ReviewRecordIdentity {
                        round: round.identity.clone(),
                        id: target.clone(),
                    },
                    kind: RelationshipKind::RecordedIncludes,
                    direction: RelationshipDirection::Outgoing,
                    role: relationship_role(round, &entry),
                    ordinal: i + 1,
                    validation: RelationshipValidation::Invalid,
                });
            }
        }
        if invalid {
            fail(
                round,
                ordinal,
                schema("includes has dangling or forward IDs; invalid edges are not navigable"),
            );
            return;
        }
        if matches!(&entry.entry, ReviewEntry::Resolution(r) if r.decision != GateDecision::Approve && r.reason.trim().is_empty())
            && !entry.includes.iter().any(|id| {
                round
                    .records
                    .iter()
                    .any(|r| &r.identity.id == id && matches!(r.entry, ReviewEntry::Annotation(_)))
            })
        {
            fail(
                round,
                ordinal,
                schema("non-approve Resolution requires reason or included Annotation"),
            );
            return;
        }
        round.records.push(entry);
    }
    round.completeness = match (round.reviewer(), round.records.last()) {
        (Some(first), Some(last))
            if matches!(last.entry, ReviewEntry::Resolution(_))
                && (first.ordinal != last.ordinal
                    || round.decision() == Some(GateDecision::Approve)) =>
        {
            ReviewCompleteness::Answered
        }
        _ => ReviewCompleteness::Incomplete,
    };
}
pub(crate) fn relationship_role(
    round: &CorrectionRound,
    record: &ReviewRecord,
) -> RelationshipRole {
    match round.reviewer() {
        Some(first) if record.ordinal == first.ordinal => RelationshipRole::ReviewerEvidence,
        Some(first) if record.ordinal > first.ordinal => RelationshipRole::FollowUp,
        None if matches!(record.entry, ReviewEntry::Resolution(_)) => {
            RelationshipRole::ReviewerEvidence
        }
        _ => RelationshipRole::BeforeVerdict,
    }
}
