//! Strict raw JSON decoding precedes RFC 8785 canonicalization.
use crate::domain::{
    CanonicalBriefing, GateAttempt, GateRecord, RoomDiagnostic, RoomItem, RoomItemKind, RoomProblem,
};
use serde::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashSet;

pub const MAX_BYTES: usize = 2 * 1024 * 1024;
pub fn raw_digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}
pub fn digest_valid(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|v| {
        v.len() == 64
            && v.bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    })
}
fn invalid(message: impl Into<String>) -> RoomDiagnostic {
    RoomDiagnostic::new(RoomProblem::InvalidJson, message)
}
struct Strict(Value);
impl<'de> Deserialize<'de> for Strict {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct StrictVisitor;
        impl<'de> Visitor<'de> for StrictVisitor {
            type Value = Strict;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("duplicate-free JSON")
            }
            fn visit_bool<E: de::Error>(self, v: bool) -> Result<Strict, E> {
                Ok(Strict(Value::Bool(v)))
            }
            fn visit_unit<E: de::Error>(self) -> Result<Strict, E> {
                Ok(Strict(Value::Null))
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<Strict, E> {
                Ok(Strict(Value::String(v.into())))
            }
            fn visit_i64<E: de::Error>(self, v: i64) -> Result<Strict, E> {
                self.visit_f64(v as f64)
            }
            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Strict, E> {
                self.visit_f64(v as f64)
            }
            fn visit_f64<E: de::Error>(self, v: f64) -> Result<Strict, E> {
                serde_json::Number::from_f64(v)
                    .map(|n| Strict(Value::Number(n)))
                    .ok_or_else(|| E::custom("nonfinite JSON number"))
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> Result<Strict, A::Error> {
                let mut v = Vec::new();
                while let Some(item) = a.next_element::<Strict>()? {
                    v.push(item.0);
                }
                Ok(Strict(Value::Array(v)))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> Result<Strict, A::Error> {
                let mut v = serde_json::Map::new();
                while let Some(k) = a.next_key::<String>()? {
                    if v.contains_key(&k) {
                        return Err(de::Error::custom("duplicate JSON member"));
                    }
                    v.insert(k, a.next_value::<Strict>()?.0);
                }
                Ok(Strict(Value::Object(v)))
            }
        }
        d.deserialize_any(StrictVisitor)
    }
}
pub fn strict_json(bytes: &[u8]) -> Result<Value, RoomDiagnostic> {
    if bytes.len() > MAX_BYTES {
        return Err(RoomDiagnostic::new(
            RoomProblem::SizeLimit,
            "JSON exceeds 2 MiB",
        ));
    }
    let mut d = serde_json::Deserializer::from_slice(bytes);
    let value = Strict::deserialize(&mut d)
        .map_err(|e| invalid(e.to_string()))?
        .0;
    d.end().map_err(|e| invalid(e.to_string()))?;
    Ok(value)
}
pub fn canonical_bytes(bytes: &[u8]) -> Result<Vec<u8>, RoomDiagnostic> {
    serde_json_canonicalizer::to_vec(&strict_json(bytes)?).map_err(|e| invalid(e.to_string()))
}
pub fn canonical_digest(bytes: &[u8]) -> Result<String, RoomDiagnostic> {
    Ok(raw_digest(&canonical_bytes(bytes)?))
}
fn text<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or("")
}
fn schema(message: &str) -> RoomDiagnostic {
    RoomDiagnostic::new(RoomProblem::InvalidSchema, message)
}
pub fn decode_briefing(
    bytes: &[u8],
    record: &GateRecord,
    attempt: &GateAttempt,
) -> Result<CanonicalBriefing, RoomDiagnostic> {
    let v = strict_json(bytes)?;
    if text(&v, "type") != "Briefing"
        || text(&v, "version") != "1"
        || text(&v, "question").trim().is_empty()
    {
        return Err(schema("requires complete Briefing v1"));
    }
    if text(&v, "id") != attempt.briefing.id || !stage_identity(&attempt.briefing.id, &record.stage)
    {
        return Err(RoomDiagnostic::new(
            RoomProblem::IdentityMismatch,
            "Briefing identity/stage differs from selected recorded attempt",
        ));
    }
    if canonical_digest(bytes)? != attempt.briefing.digest {
        return Err(RoomDiagnostic::new(
            RoomProblem::BriefingDigestMismatch,
            "canonical Briefing digest differs from binding",
        ));
    }
    let artifacts = v
        .get("artifacts")
        .and_then(Value::as_array)
        .filter(|a| !a.is_empty())
        .ok_or_else(|| schema("requires artifacts"))?;
    let mut items = Vec::new();
    let mut ids = HashSet::new();
    for artifact in artifacts {
        add_item(artifact, RoomItemKind::Artifact, &mut ids, &mut items)?;
    }
    if let Some(context) = v.get("context") {
        flatten(context, &mut ids, &mut items, 0)?;
    }
    let git = items
        .iter()
        .filter(|i| i.uri.starts_with("git-root://"))
        .count();
    if git > 0 && git != items.len() {
        return Err(schema("mixed Git-root and local inventory"));
    }
    Ok(CanonicalBriefing {
        id: attempt.briefing.id.clone(),
        question: text(&v, "question").into(),
        items,
    })
}
fn stage_identity(id: &str, stage: &str) -> bool {
    let parts = id.split(':').collect::<Vec<_>>();
    parts.len() >= 5
        && parts[0] == "briefing"
        && !parts[1..parts.len() - 3].join(":").is_empty()
        && parts[parts.len() - 3] == stage
        && positive_suffix(parts[parts.len() - 2], "attempt-")
        && positive_suffix(parts[parts.len() - 1], "revision-")
}
fn positive_suffix(s: &str, prefix: &str) -> bool {
    s.strip_prefix(prefix).is_some_and(|n| {
        matches!(n.as_bytes().first(), Some(b'1'..=b'9')) && n.bytes().all(|c| c.is_ascii_digit())
    })
}
fn add_item(
    v: &Value,
    kind: RoomItemKind,
    ids: &mut HashSet<String>,
    items: &mut Vec<RoomItem>,
) -> Result<(), RoomDiagnostic> {
    let (id, uri, rev) = (text(v, "id"), text(v, "uri"), text(v, "rev"));
    if id.trim().is_empty() || uri.trim().is_empty() || !digest_valid(rev) || !ids.insert(id.into())
    {
        return Err(schema("incomplete/duplicate item binding"));
    }
    if kind == RoomItemKind::Reference && v.get("summary").is_some() {
        return Err(schema("References cannot carry summaries"));
    }
    let summary = match v.get("summary") {
        None => None,
        Some(Value::String(s)) => Some(s.clone()),
        _ => return Err(schema("Artifact summary must be text")),
    };
    items.push(RoomItem {
        kind,
        id: id.into(),
        uri: uri.into(),
        revision: rev.into(),
        summary,
    });
    Ok(())
}
fn flatten(
    nodes: &Value,
    ids: &mut HashSet<String>,
    items: &mut Vec<RoomItem>,
    depth: usize,
) -> Result<(), RoomDiagnostic> {
    if nodes.is_null() {
        return Ok(());
    }
    if depth > 64 {
        return Err(RoomDiagnostic::new(
            RoomProblem::SizeLimit,
            "context nesting exceeds 64",
        ));
    }
    for node in nodes
        .as_array()
        .ok_or_else(|| schema("context children must be arrays"))?
    {
        if !node.is_object() {
            return Err(schema("context node must be an object"));
        }
        if text(node, "type") == "Reference" {
            add_item(node, RoomItemKind::Reference, ids, items)?;
        }
        if let Some(children) = node.get("children") {
            flatten(children, ids, items, depth + 1)?;
        }
    }
    Ok(())
}
pub fn request_locator(
    bytes: &[u8],
    record: &GateRecord,
    attempt: &GateAttempt,
) -> Result<String, RoomDiagnostic> {
    let v = strict_json(bytes)?;
    if canonical_digest(bytes)? != attempt.briefing.request_digest.clone().unwrap_or_default() {
        return Err(RoomDiagnostic::new(
            RoomProblem::RequestDigestMismatch,
            "request digest differs from binding",
        ));
    }
    let b = &v["briefing"];
    if text(&v, "type") != "spacedock-gate-presentation-request"
        || text(&v, "version") != "1"
        || text(&v, "gate") != record.id
        || text(&v, "attempt") != attempt.id
        || text(&v, "actor") != "person:captain"
        || text(&v, "approver") != "person:captain"
        || text(b, "id") != attempt.briefing.id
        || text(b, "digest") != attempt.briefing.digest
    {
        return Err(RoomDiagnostic::new(
            RoomProblem::BindingMismatch,
            "request does not bind selected gate/attempt/Briefing and captain",
        ));
    }
    Ok(text(b, "locator").into())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn jcs_rfc8785_golden_vectors_and_strict_input() {
        let input = br#"{"numbers":[333333333.33333329,1E30,4.50,2e-3,0.000000000000000000000000001],"string":"\u20ac$\u000f\nA'B\"\\\\\"/","literals":[null,true,false]}"#;
        let expected = r#"{"literals":[null,true,false],"numbers":[333333333.3333333,1e+30,4.5,0.002,1e-27],"string":"€$\u000f\nA'B\"\\\\\"/"}"#;
        assert_eq!(canonical_bytes(input).unwrap(), expected.as_bytes());
        assert_eq!(String::from_utf8(canonical_bytes(br#"{"\ufb33":7,"\ud83d\ude00":6,"\u0080":3,"\r":1,"1":2,"\u20ac":5,"\u00f6":4}"#).unwrap()).unwrap(), "{\"\\r\":1,\"1\":2,\"\u{80}\":3,\"ö\":4,\"€\":5,\"😀\":6,\"דּ\":7}");
        for input in [
            br#"{"a":1,"\u0061":2}"#.as_slice(),
            br#"{"x":{"a":1,"a":2}}"#,
            br#""\ud800""#,
            br#"1e999"#,
            &[b'"', 0xff, b'"'],
        ] {
            assert!(canonical_bytes(input).is_err());
        }
        assert_eq!(
            canonical_digest(br#"{"b":2,"a":1}"#).unwrap(),
            canonical_digest(br#"{ "a":1.0, "b":2e0 }"#).unwrap()
        );
        assert_eq!(
            raw_digest(b"abc"),
            "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
