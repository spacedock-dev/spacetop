use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use walkdir::WalkDir;

use super::projection::{project_record, ProjectedRecord, ProjectedRecordKind};
use super::{AgentRuntime, SessionRoots};

const CHECKPOINT_BYTES: u64 = 128;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum SessionEvidenceKey {
    RuntimeSession(AgentRuntime, String),
    Source(AgentRuntime, PathBuf),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SessionEvidenceStore {
    records: HashMap<SessionEvidenceKey, Vec<ProjectedRecord>>,
}

impl SessionEvidenceStore {
    fn merge(
        &mut self,
        runtime: AgentRuntime,
        source: &Path,
        preferred_key: Option<&SessionEvidenceKey>,
        records: Vec<ProjectedRecord>,
    ) -> SessionEvidenceKey {
        let key = detected_evidence_key(runtime, &records)
            .or_else(|| preferred_key.cloned())
            .unwrap_or_else(|| SessionEvidenceKey::Source(runtime, source.to_path_buf()));
        let retained = self.records.entry(key.clone()).or_default();
        for record in records {
            if !retained.contains(&record) {
                retained.push(record);
            }
        }
        retained.sort();
        key
    }

    pub(crate) fn all_records(&self) -> Vec<&ProjectedRecord> {
        let mut records: Vec<_> = self.records.values().flatten().collect();
        records.sort();
        records
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionFileCursor {
    modified: Option<SystemTime>,
    len: u64,
    pub(super) cursor: u64,
    complete_lines: u64,
    checkpoint: Vec<u8>,
    evidence_key: SessionEvidenceKey,
    parse_errors: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SessionScanState {
    pub(crate) files: HashMap<PathBuf, SessionFileCursor>,
    pub(crate) evidence: SessionEvidenceStore,
}

#[derive(Debug)]
pub(crate) struct LoadedGeneration {
    pub(crate) state: SessionScanState,
    pub(crate) errors: Vec<String>,
}

#[derive(Debug)]
pub(crate) enum LoadGenerationError {
    Root(String),
    Unstable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InventoryEntry {
    runtime: AgentRuntime,
    modified: Option<SystemTime>,
    len: u64,
}

pub(crate) fn load_generation(
    roots: &SessionRoots,
    previous: &SessionScanState,
) -> Result<LoadedGeneration, LoadGenerationError> {
    load_generation_with_hook(roots, previous, || {})
}

fn load_generation_with_hook(
    roots: &SessionRoots,
    previous: &SessionScanState,
    after_read: impl FnOnce(),
) -> Result<LoadedGeneration, LoadGenerationError> {
    let (before, mut errors) = inventory(roots)?;
    let mut next = previous.clone();
    next.files.clear();

    let mut paths: Vec<_> = before.keys().cloned().collect();
    paths.sort();
    for path in paths {
        let entry = &before[&path];
        match load_file(&path, entry, previous.files.get(&path), &mut next.evidence) {
            Ok(cursor) => {
                errors.extend(cursor.parse_errors.iter().cloned());
                next.files.insert(path, cursor);
            }
            Err(err) => errors.push(format!(
                "{} scan could not read {}: {err}",
                entry.runtime.label(),
                path.display()
            )),
        }
    }

    after_read();
    let (after, after_errors) = inventory(roots)?;
    errors.extend(after_errors);
    if before != after {
        return Err(LoadGenerationError::Unstable);
    }
    Ok(LoadedGeneration {
        state: next,
        errors,
    })
}

fn inventory(
    roots: &SessionRoots,
) -> Result<(HashMap<PathBuf, InventoryEntry>, Vec<String>), LoadGenerationError> {
    let mut files = HashMap::new();
    let mut errors = Vec::new();
    for (runtime, root) in roots.all_roots() {
        if !root.exists() {
            continue;
        }
        if let Err(err) = fs::read_dir(root) {
            return Err(LoadGenerationError::Root(format!(
                "{} session root {} is unreadable: {err}",
                runtime.label(),
                root.display()
            )));
        }
        for entry in WalkDir::new(root)
            .into_iter()
            .filter_entry(|entry| !is_pruned_dir(entry.path()))
        {
            let entry = match entry {
                Ok(entry) => entry,
                Err(err) => {
                    errors.push(format!("{} scan skipped entry: {err}", runtime.label()));
                    continue;
                }
            };
            if !entry.file_type().is_file() || !is_session_file(entry.path()) {
                continue;
            }
            match entry.metadata() {
                Ok(metadata) => {
                    files.insert(
                        entry.path().to_path_buf(),
                        InventoryEntry {
                            runtime,
                            modified: metadata.modified().ok(),
                            len: metadata.len(),
                        },
                    );
                }
                Err(err) => errors.push(format!(
                    "{} scan could not read metadata for {}: {err}",
                    runtime.label(),
                    entry.path().display()
                )),
            }
        }
    }
    Ok((files, errors))
}

fn load_file(
    path: &Path,
    entry: &InventoryEntry,
    previous: Option<&SessionFileCursor>,
    evidence: &mut SessionEvidenceStore,
) -> Result<SessionFileCursor, std::io::Error> {
    if let Some(previous) = previous {
        if previous.len == entry.len
            && previous.modified == entry.modified
            && entry.modified.is_some()
        {
            return Ok(previous.clone());
        }
    }

    if path.extension().and_then(OsStr::to_str) == Some("json") {
        let (records, parse_errors) = parse_json(path)?;
        let evidence_key = if records.is_empty() {
            previous
                .map(|cursor| cursor.evidence_key.clone())
                .unwrap_or_else(|| SessionEvidenceKey::Source(entry.runtime, path.to_path_buf()))
        } else {
            evidence.merge(
                entry.runtime,
                path,
                previous.map(|cursor| &cursor.evidence_key),
                records,
            )
        };
        return Ok(SessionFileCursor {
            modified: entry.modified,
            len: entry.len,
            cursor: entry.len,
            complete_lines: 0,
            checkpoint: read_checkpoint(path, entry.len)?,
            evidence_key,
            parse_errors,
        });
    }

    let (start, starting_line) = match previous {
        Some(previous)
            if entry.len > previous.len && append_checkpoint_matches(path, previous)? =>
        {
            (previous.cursor, previous.complete_lines)
        }
        _ => (0, 0),
    };
    let parsed = parse_jsonl_from(path, start, starting_line)?;
    let evidence_key = if parsed.records.is_empty() {
        previous
            .map(|cursor| cursor.evidence_key.clone())
            .unwrap_or_else(|| SessionEvidenceKey::Source(entry.runtime, path.to_path_buf()))
    } else {
        evidence.merge(
            entry.runtime,
            path,
            previous.map(|cursor| &cursor.evidence_key),
            parsed.records,
        )
    };
    Ok(SessionFileCursor {
        modified: entry.modified,
        len: entry.len,
        cursor: parsed.cursor,
        complete_lines: parsed.complete_lines,
        checkpoint: read_checkpoint(path, parsed.cursor)?,
        evidence_key,
        parse_errors: parsed.errors,
    })
}

#[derive(Debug)]
struct ParsedChunk {
    records: Vec<ProjectedRecord>,
    cursor: u64,
    complete_lines: u64,
    errors: Vec<String>,
}

fn parse_json(path: &Path) -> Result<(Vec<ProjectedRecord>, Vec<String>), std::io::Error> {
    #[cfg(test)]
    super::record_session_file_parse(path, 0);
    let file = fs::File::open(path)?;
    let reader = BufReader::new(file);
    Ok(match serde_json::from_reader(reader) {
        Ok(value) => (
            project_record(value, path, 0).into_iter().collect(),
            Vec::new(),
        ),
        Err(err) => (
            Vec::new(),
            vec![format!(
                "malformed session record {}: {err}",
                path.display()
            )],
        ),
    })
}

fn parse_jsonl_from(
    path: &Path,
    start: u64,
    starting_line: u64,
) -> Result<ParsedChunk, std::io::Error> {
    #[cfg(test)]
    super::record_session_file_parse(path, start);
    let mut file = fs::File::open(path)?;
    file.seek(SeekFrom::Start(start))?;
    let mut reader = BufReader::new(file);
    let mut records = Vec::new();
    let mut errors = Vec::new();
    let mut cursor = start;
    let mut complete_lines = starting_line;
    loop {
        let record_offset = cursor;
        let mut line = Vec::new();
        let read = reader.read_until(b'\n', &mut line)?;
        if read == 0 {
            break;
        }
        let terminated = line.last() == Some(&b'\n');
        if line.iter().all(u8::is_ascii_whitespace) {
            cursor += read as u64;
            continue;
        }
        match serde_json::from_slice(&line) {
            Ok(value) => {
                cursor += read as u64;
                complete_lines += 1;
                if let Some(projected) = project_record(value, path, record_offset) {
                    records.push(projected);
                }
            }
            Err(_) if !terminated => break,
            Err(err) => {
                cursor += read as u64;
                complete_lines += 1;
                errors.push(format!(
                    "malformed session record {}:{}: {err}",
                    path.display(),
                    complete_lines
                ));
            }
        }
    }
    Ok(ParsedChunk {
        records,
        cursor,
        complete_lines,
        errors,
    })
}

fn detected_evidence_key(
    runtime: AgentRuntime,
    records: &[ProjectedRecord],
) -> Option<SessionEvidenceKey> {
    let session = records.iter().find_map(|record| match &record.kind {
        ProjectedRecordKind::CodexSession { session_id, .. } => Some(session_id.clone()),
        ProjectedRecordKind::ClaudeMessage {
            is_sidechain: true,
            agent_id: Some(agent_id),
            ..
        } => Some(agent_id.clone()),
        ProjectedRecordKind::ClaudeMessage {
            session_id: Some(session_id),
            ..
        } => Some(session_id.clone()),
        ProjectedRecordKind::ClaudeMeta {
            agent_id: Some(agent_id),
            ..
        } => Some(agent_id.clone()),
        _ => None,
    });
    session.map(|session| SessionEvidenceKey::RuntimeSession(runtime, session))
}

fn append_checkpoint_matches(
    path: &Path,
    previous: &SessionFileCursor,
) -> Result<bool, std::io::Error> {
    if previous.cursor == 0 || previous.checkpoint.is_empty() {
        return Ok(false);
    }
    Ok(read_checkpoint(path, previous.cursor)? == previous.checkpoint)
}

fn read_checkpoint(path: &Path, cursor: u64) -> Result<Vec<u8>, std::io::Error> {
    if cursor == 0 {
        return Ok(Vec::new());
    }
    let start = cursor.saturating_sub(CHECKPOINT_BYTES);
    let mut file = fs::File::open(path)?;
    file.seek(SeekFrom::Start(start))?;
    let mut checkpoint = vec![0; (cursor - start) as usize];
    file.read_exact(&mut checkpoint)?;
    Ok(checkpoint)
}

fn is_pruned_dir(path: &Path) -> bool {
    path.file_name()
        .and_then(OsStr::to_str)
        .is_some_and(|name| matches!(name, ".git" | "node_modules" | "target"))
}

fn is_session_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(OsStr::to_str),
        Some("jsonl" | "json")
    )
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    #[test]
    fn changed_inventory_rejects_the_generation_for_immediate_retry() {
        let temp = tempfile::tempdir().expect("temp");
        let path = temp.path().join("session.jsonl");
        fs::write(
            &path,
            "{\"timestamp\":1,\"type\":\"event_msg\",\"payload\":{\"type\":\"task_started\",\"turn_id\":\"turn\"}}\n",
        )
        .expect("fixture");
        let roots = SessionRoots {
            codex: vec![temp.path().to_path_buf()],
            claude_code: Vec::new(),
        };
        let result = load_generation_with_hook(&roots, &SessionScanState::default(), || {
            let mut append = fs::OpenOptions::new()
                .append(true)
                .open(&path)
                .expect("append");
            writeln!(
                append,
                r#"{{"timestamp":2,"type":"event_msg","payload":{{"type":"task_complete","turn_id":"turn"}}}}"#
            )
            .expect("terminal");
        });
        assert!(matches!(result, Err(LoadGenerationError::Unstable)));
    }
}
