//! Strict canonical decoding, with tolerance confined to application extensions.
use crate::domain::*;
use serde::{
    de::{self, MapAccess, SeqAccess, Visitor},
    Deserialize, Deserializer,
};
use serde_yaml::{Mapping, Value};
use std::collections::HashSet;

// Preserve duplicate-key refusal before filtering compatibility fields. Value's
// normal map conversion can lose duplicate keys; recurse through this visitor.
struct UniqueValue(Value);
impl<'de> Deserialize<'de> for UniqueValue {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct UniqueVisitor;
        impl<'de> Visitor<'de> for UniqueVisitor {
            type Value = UniqueValue;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("YAML without duplicate mapping keys")
            }
            fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Self::Value, M::Error> {
                let mut result = Mapping::new();
                while let Some((UniqueValue(key), UniqueValue(value))) = map.next_entry()? {
                    if result.contains_key(&key) {
                        return Err(de::Error::custom(format!("duplicate key {key:?}")));
                    }
                    result.insert(key, value);
                }
                Ok(UniqueValue(Value::Mapping(result)))
            }
            fn visit_seq<S: SeqAccess<'de>>(self, mut seq: S) -> Result<Self::Value, S::Error> {
                let mut values = Vec::new();
                while let Some(UniqueValue(value)) = seq.next_element()? {
                    values.push(value);
                }
                Ok(UniqueValue(Value::Sequence(values)))
            }
            fn visit_bool<E: de::Error>(self, value: bool) -> Result<Self::Value, E> {
                Ok(UniqueValue(Value::Bool(value)))
            }
            fn visit_i64<E: de::Error>(self, value: i64) -> Result<Self::Value, E> {
                Ok(UniqueValue(Value::Number(value.into())))
            }
            fn visit_u64<E: de::Error>(self, value: u64) -> Result<Self::Value, E> {
                Ok(UniqueValue(Value::Number(value.into())))
            }
            fn visit_f64<E: de::Error>(self, value: f64) -> Result<Self::Value, E> {
                Ok(UniqueValue(Value::Number(value.into())))
            }
            fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
                Ok(UniqueValue(Value::String(value.into())))
            }
            fn visit_string<E: de::Error>(self, value: String) -> Result<Self::Value, E> {
                Ok(UniqueValue(Value::String(value)))
            }
            fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
                Ok(UniqueValue(Value::Null))
            }
            fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
                self.visit_unit()
            }
        }
        deserializer.deserialize_any(UniqueVisitor)
    }
}
pub(super) fn parse(frontmatter: &str) -> GateData {
    // Only the existing flat-scalar parser can establish legacy absence when
    // YAML decoding fails. Once YAML is structural, every gate decode error
    // stays invalid regardless of flow style, explicit keys, or aliases.
    let root = match serde_yaml::from_str::<UniqueValue>(frontmatter) {
        Ok(root) => root.0,
        Err(error) => {
            let legacy_absence = super::frontmatter::top_level_scalar_entries(frontmatter)
                .is_some_and(|entries| {
                    entries.iter().all(|(key, _)| {
                        serde_yaml::from_str::<String>(key).is_ok_and(|key| key != "gates")
                    })
                });
            return if legacy_absence {
                GateData::Absent
            } else {
                invalid(error.to_string())
            };
        }
    };
    let Some(gates) = root.get("gates").cloned() else {
        return GateData::Absent;
    };
    match decode(gates) {
        Ok((document, warnings)) => GateData::Valid { document, warnings },
        Err(error) => invalid(error),
    }
}
fn invalid(error: String) -> GateData {
    GateData::Invalid {
        diagnostics: vec![format!("gates: {error}")],
    }
}
fn decode(mut gates: Value) -> Result<(GateDocument, Vec<GateWarning>), String> {
    let mut warnings = Vec::new();
    if let Some(records) = gates.get_mut("records").and_then(Value::as_sequence_mut) {
        for (ri, record) in records.iter_mut().enumerate() {
            if let Some(attempts) = record.get_mut("attempts").and_then(Value::as_sequence_mut) {
                for (ai, attempt) in attempts.iter_mut().enumerate() {
                    if let Some(mapping) = attempt.as_mapping_mut() {
                        mapping.remove(Value::String("provider-evidence".into()));
                        if let Some(application) =
                            mapping.get_mut(Value::String("application".into()))
                        {
                            let path = format!("gates.records[{ri}].attempts[{ai}].application");
                            let fields = application
                                .as_mapping_mut()
                                .ok_or_else(|| format!("{path} must be a mapping"))?;
                            let keys: Vec<_> = fields.keys().cloned().collect();
                            for key in keys {
                                let field = key
                                    .as_str()
                                    .ok_or_else(|| format!("{path} keys must be strings"))?;
                                if field != "state" && field != "target-stage" {
                                    warnings.push(GateWarning {
                                        path: path.clone(),
                                        field: field.into(),
                                    });
                                    fields.remove(&key);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    let doc: GateDocument = serde_yaml::from_value(gates).map_err(|e| e.to_string())?;
    validate(&doc)?;
    warnings.sort_by(|a, b| (&a.path, &a.field).cmp(&(&b.path, &b.field)));
    warnings.dedup();
    Ok((doc, warnings))
}
fn unique(set: &mut HashSet<String>, value: &str, path: &str) -> Result<(), String> {
    if value.is_empty() || !set.insert(value.into()) {
        return Err(format!("{path} missing or duplicate identity"));
    }
    Ok(())
}
fn digest(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|s| {
        s.len() == 64
            && s.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}
fn validate(doc: &GateDocument) -> Result<(), String> {
    if doc.version != 1 || doc.records.is_empty() {
        return Err("version must be 1 and records nonempty".into());
    }
    let (mut gates, mut stages, mut attempts, mut briefings, mut resolutions) = (
        HashSet::new(),
        HashSet::new(),
        HashSet::new(),
        HashSet::new(),
        HashSet::new(),
    );
    for (ri, record) in doc.records.iter().enumerate() {
        let path = format!("gates.records[{ri}]");
        unique(&mut gates, &record.id, &path)?;
        unique(&mut stages, &record.stage, &path)?;
        if record.attempts.is_empty() {
            return Err(format!("{path}.attempts must be nonempty"));
        }
        let mut pending = 0;
        for (ai, attempt) in record.attempts.iter().enumerate() {
            let path = format!("{path}.attempts[{ai}]");
            unique(&mut attempts, &attempt.id, &path)?;
            unique(&mut briefings, &attempt.briefing.id, &path)?;
            validate_attempt(attempt, &path)?;
            if let Some(resolution) = &attempt.resolution {
                unique(&mut resolutions, &resolution.id, &path)?;
            }
            pending += usize::from(
                attempt
                    .application
                    .as_ref()
                    .is_some_and(|a| a.state == GateApplicationState::Pending),
            );
        }
        if pending > 1 {
            return Err(format!("{path} multiple pending applications"));
        }
    }
    Ok(())
}
// RFC3339 UTC validation without a new time dependency. Validate calendar and
// clock ranges too; a suffix alone is not timestamp proof.
fn utc_timestamp(value: &str) -> bool {
    let value = value
        .strip_suffix('Z')
        .or_else(|| value.strip_suffix("+00:00"));
    let Some(value) = value else {
        return false;
    };
    let Some((date, time)) = value.split_once('T') else {
        return false;
    };
    let date: Vec<_> = date.split('-').collect();
    let time: Vec<_> = time.split(':').collect();
    if date.len() != 3
        || time.len() != 3
        || date[0].len() != 4
        || date[1].len() != 2
        || date[2].len() != 2
        || time[0].len() != 2
        || time[1].len() != 2
    {
        return false;
    }
    let parse = |s: &str| {
        if s.bytes().all(|b| b.is_ascii_digit()) {
            s.parse::<u32>().ok()
        } else {
            None
        }
    };
    let (Some(year), Some(month), Some(day), Some(hour), Some(minute)) = (
        parse(date[0]),
        parse(date[1]),
        parse(date[2]),
        parse(time[0]),
        parse(time[1]),
    ) else {
        return false;
    };
    let (seconds, fraction) = time[2]
        .split_once('.')
        .map_or((time[2], None), |(s, f)| (s, Some(f)));
    if seconds.len() != 2
        || fraction.is_some_and(|s| s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()))
    {
        return false;
    }
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        _ => 0,
    };
    day > 0 && day <= days && hour < 24 && minute < 60 && parse(seconds).is_some_and(|s| s < 60)
}

fn validate_attempt(attempt: &GateAttempt, path: &str) -> Result<(), String> {
    if !digest(&attempt.briefing.digest)
        || attempt.briefing.room_ref.is_empty()
        || attempt
            .briefing
            .request_digest
            .as_ref()
            .is_some_and(|s| !s.is_empty() && !digest(s))
    {
        return Err(format!("{path}.briefing invalid binding or digest"));
    }
    if attempt.withdrawal.is_some() && attempt.resolution.is_some() {
        return Err(format!("{path} conflicting withdrawal and resolution"));
    }
    if let Some(withdrawal) = &attempt.withdrawal {
        if withdrawal.by != "agent:first-officer"
            || withdrawal.reason.trim().is_empty()
            || !utc_timestamp(&withdrawal.at)
        {
            return Err(format!(
                "{path}.withdrawal invalid attribution, UTC time or reason"
            ));
        }
    }
    if let Some(resolution) = &attempt.resolution {
        validate_resolution(resolution, &attempt.briefing.id, path)?;
    }
    if let Some(app) = &attempt.application {
        if attempt
            .resolution
            .as_ref()
            .is_none_or(|r| r.decision != GateDecision::Approve)
            || attempt.withdrawal.is_some()
            || app.target_stage.trim().is_empty()
        {
            return Err(format!("{path}.application requires approval and target"));
        }
    }
    Ok(())
}
fn validate_resolution(
    resolution: &GateResolution,
    briefing_id: &str,
    path: &str,
) -> Result<(), String> {
    if resolution.record_type != "Resolution"
        || resolution.briefing != briefing_id
        || resolution.by.is_empty()
        || resolution.at.is_empty()
    {
        return Err(format!("{path}.resolution invalid binding or attribution"));
    }
    if resolution.decision != GateDecision::Approve
        && resolution.reason.trim().is_empty()
        && resolution.includes.is_empty()
    {
        return Err(format!("{path}.resolution requires reason or includes"));
    }
    if let Some(conn) = &resolution.conn {
        if resolution.by != "agent:first-officer"
            || conn.quote.trim().is_empty()
            || conn.source.trim().is_empty()
        {
            return Err(format!(
                "{path}.resolution.conn requires first-officer and nonblank quote/source"
            ));
        }
    }
    Ok(())
}
