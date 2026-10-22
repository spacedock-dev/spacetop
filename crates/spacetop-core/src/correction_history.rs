//! Bounded read-only advisory history discovery. Paths from events are never read coordinates.
use crate::{domain::*, gate_room, parser::correction_history as decode};
use std::path::{Component, Path, PathBuf};

pub const MAX_ENTRIES: usize = 4096;
pub const MAX_ROUNDS: usize = 128;
type Result<T, E = RoomDiagnostic> = std::result::Result<T, E>;
fn problem(kind: RoomProblem, message: impl Into<String>) -> RoomDiagnostic {
    RoomDiagnostic::new(kind, message)
}
fn io_error(e: std::io::Error) -> RoomDiagnostic {
    problem(
        if e.kind() == std::io::ErrorKind::NotFound {
            RoomProblem::Missing
        } else {
            RoomProblem::UnsafePath
        },
        e.to_string(),
    )
}
fn origin(entity: &Entity) -> &Path {
    entity.evidence_origin.as_deref().unwrap_or(&entity.path)
}
fn parent(root: &Path, entity: &Entity) -> Result<PathBuf> {
    let path = origin(entity)
        .parent()
        .ok_or_else(|| problem(RoomProblem::UnsafePath, "missing entity parent"))?;
    gate_room::reject_symlinks(path)?;
    let path = std::fs::canonicalize(path).map_err(io_error)?;
    if !path.starts_with(root) {
        return Err(problem(
            RoomProblem::UnsupportedRoot,
            "entity origin is outside safe root",
        ));
    }
    Ok(path)
}
#[cfg(unix)]
fn names(root: &Path, path: &Path, remaining: &mut usize) -> Result<Vec<String>> {
    use rustix::fs::{open, openat, Dir, Mode, OFlags};
    gate_room::reject_symlinks(root)?;
    let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    let mut fd = open(root, flags, Mode::empty()).map_err(|e| io_error(e.into()))?;
    let relative = path
        .strip_prefix(root)
        .map_err(|_| problem(RoomProblem::UnsafePath, "directory outside safe root"))?;
    for component in relative.components() {
        if !matches!(component, Component::Normal(_)) {
            return Err(problem(RoomProblem::UnsafePath, "invalid discovery path"));
        }
        fd = openat(&fd, Path::new(component.as_os_str()), flags, Mode::empty())
            .map_err(|e| io_error(e.into()))?;
    }
    let mut result = Vec::new();
    for entry in Dir::new(fd).map_err(|e| io_error(e.into()))? {
        let entry = entry.map_err(|e| io_error(e.into()))?;
        let name = entry
            .file_name()
            .to_str()
            .map_err(|_| problem(RoomProblem::InvalidSchema, "non-UTF-8 review entry"))?;
        if name == "." || name == ".." {
            continue;
        }
        if *remaining == 0 {
            return Err(problem(
                RoomProblem::SizeLimit,
                "discovery truncated: entry limit reached",
            ));
        }
        *remaining -= 1;
        result.push(name.into());
    }
    gate_room::reject_symlinks(path)?;
    result.sort();
    Ok(result)
}
#[cfg(not(unix))]
fn names(_root: &Path, _path: &Path, _remaining: &mut usize) -> Result<Vec<String>> {
    Err(problem(
        RoomProblem::UnsupportedRoot,
        "safe discovery requires Unix descriptor APIs",
    ))
}
pub fn load(definition: &WorkflowDefinition, entity: &Entity) -> CorrectionHistory {
    let mut history = CorrectionHistory::default();
    if let ReviewPointer::Invalid { diagnostic } = &entity.review_round {
        history.diagnostics.push(ReviewDiagnostic {
            path: origin(entity).to_path_buf(),
            line: None,
            problem: problem(
                RoomProblem::InvalidSchema,
                format!("review-round: {diagnostic}"),
            ),
        });
    }
    let roots = gate_room::safe_root(definition)
        .and_then(|root| parent(&root, entity).map(|parent| (root, parent)));
    let (root, parent) = match roots {
        Ok(roots) => roots,
        Err(error) => {
            history.diagnostics.push(ReviewDiagnostic {
                path: origin(entity).into(),
                line: None,
                problem: error,
            });
            return history;
        }
    };
    let pointer = match &entity.review_round {
        ReviewPointer::Valid { pointer } => Some(pointer),
        _ => None,
    };
    let mut candidates = Vec::new();
    if let Some(p) = pointer {
        candidates.push((p.stage.clone(), p.cycle));
    }
    discover(&root, &parent, &mut candidates, &mut history.diagnostics);
    candidates.sort();
    candidates.dedup();
    for (stage, cycle) in candidates {
        let selected = pointer.filter(|p| p.stage == stage && p.cycle == cycle);
        let identity = RoundIdentity {
            origin: origin(entity).into(),
            entity_id: entity.id.clone(),
            briefing_id: decode::briefing_id(&entity.id, &stage, cycle),
            stage,
            cycle,
        };
        history
            .rounds
            .push(load_round(&root, &parent, identity, selected));
    }
    history.rounds.sort_by(|a, b| {
        let order = |stage: &str| {
            definition
                .stages
                .iter()
                .position(|s| s.name == stage)
                .unwrap_or(usize::MAX)
        };
        (
            order(&a.identity.stage),
            &a.identity.stage,
            a.identity.cycle,
        )
            .cmp(&(
                order(&b.identity.stage),
                &b.identity.stage,
                b.identity.cycle,
            ))
    });
    history
}
fn discover(
    root: &Path,
    parent: &Path,
    candidates: &mut Vec<(String, u32)>,
    diagnostics: &mut Vec<ReviewDiagnostic>,
) {
    let review = parent.join("review");
    let mut remaining = MAX_ENTRIES;
    let stages = match names(root, &review, &mut remaining) {
        Ok(stages) => stages,
        Err(e) if e.kind == RoomProblem::Missing => return,
        Err(problem) => {
            diagnostics.push(ReviewDiagnostic {
                path: review,
                line: None,
                problem,
            });
            return;
        }
    };
    for stage in stages {
        let dir = review.join(&stage);
        if !decode::stage_valid(&stage) {
            diagnostics.push(ReviewDiagnostic {
                path: dir,
                line: None,
                problem: problem(RoomProblem::InvalidSchema, "invalid review stage directory"),
            });
            continue;
        }
        let rounds = match names(root, &dir, &mut remaining) {
            Ok(rounds) => rounds,
            Err(problem) => {
                diagnostics.push(ReviewDiagnostic {
                    path: dir,
                    line: None,
                    problem,
                });
                continue;
            }
        };
        for name in rounds {
            let path = dir.join(&name);
            let Some(cycle) = name.strip_prefix("round-").and_then(decode::cycle_number) else {
                diagnostics.push(ReviewDiagnostic {
                    path,
                    line: None,
                    problem: problem(
                        RoomProblem::InvalidSchema,
                        "invalid round directory: normalized positive u32 required",
                    ),
                });
                continue;
            };
            if candidates.len() >= MAX_ROUNDS {
                diagnostics.push(ReviewDiagnostic {
                    path,
                    line: None,
                    problem: problem(
                        RoomProblem::SizeLimit,
                        "discovery truncated: round limit reached",
                    ),
                });
                return;
            }
            candidates.push((stage.clone(), cycle));
        }
    }
}
fn load_round(
    root: &Path,
    parent: &Path,
    identity: RoundIdentity,
    pointer: Option<&RoundPointer>,
) -> CorrectionRound {
    let room = parent
        .join("review")
        .join(&identity.stage)
        .join(format!("round-{}", identity.cycle));
    let mut round = CorrectionRound {
        identity,
        room,
        provenance: if pointer.is_some() {
            RoundProvenance::CurrentPointer
        } else {
            RoundProvenance::Retained
        },
        briefing: None,
        digest: None,
        references: Vec::new(),
        records: Vec::new(),
        completeness: ReviewCompleteness::Invalid,
        diagnostics: Vec::new(),
        invalid_relationships: Vec::new(),
    };
    let path = round.room.join("briefing.json");
    let briefing = gate_room::read_file(root, &path)
        .and_then(|bytes| decode::decode_briefing(&bytes, &round.identity, pointer));
    match briefing {
        Ok((briefing, references, digest)) => {
            round.briefing = Some(briefing);
            round.references = references;
            round.digest = Some(digest);
        }
        Err(problem) => {
            round.diagnostics.push(ReviewDiagnostic {
                path,
                line: None,
                problem,
            });
            return round;
        }
    }
    let path = round.room.join("briefing.review.jsonl");
    match gate_room::read_file(root, &path) {
        Ok(bytes) => decode::decode_log(&bytes, &mut round),
        Err(problem) => round.diagnostics.push(ReviewDiagnostic {
            path,
            line: None,
            problem,
        }),
    }
    round
}
/// Resolve round-local relative paths with the entity parent as an explicit floor.
pub fn artifact_path(parent: &Path, room: &Path, uri: &str) -> Result<PathBuf> {
    if uri.is_empty()
        || uri.contains(['\\', '\0', '%', ':', '?', '#'])
        || Path::new(uri).is_absolute()
    {
        return Err(problem(
            RoomProblem::UnsafePath,
            "requires unencoded relative artifact path",
        ));
    }
    let mut relative = room
        .strip_prefix(parent)
        .map_err(|_| problem(RoomProblem::UnsafePath, "room outside entity parent"))?
        .to_path_buf();
    for component in Path::new(uri).components() {
        match component {
            Component::CurDir => {}
            Component::Normal(p) => relative.push(p),
            Component::ParentDir if relative.pop() => {}
            _ => {
                return Err(problem(
                    RoomProblem::UnsafePath,
                    "artifact escapes entity parent",
                ))
            }
        }
    }
    if relative.file_name().is_some_and(|n| n == "index.md") {
        return Err(problem(
            RoomProblem::UnsafePath,
            "mutable index.md is not evidence",
        ));
    }
    Ok(parent.join(relative))
}
/// Explicit opens revalidate the original room digest and exact inventory item.
pub fn read_item(
    definition: &WorkflowDefinition,
    entity: &Entity,
    selected: &CorrectionRound,
    item: &RoomItem,
) -> Result<Vec<u8>> {
    let fresh = load(definition, entity);
    let round = fresh
        .correction_round(&selected.identity)
        .ok_or_else(|| problem(RoomProblem::Replaced, "selected round disappeared"))?;
    if selected.digest.is_none()
        || round.digest != selected.digest
        || !round
            .briefing
            .as_ref()
            .is_some_and(|b| b.items.contains(item))
    {
        return Err(problem(
            RoomProblem::Replaced,
            "selected Briefing or item changed",
        ));
    }
    let bytes = if item.uri.starts_with("git-root://") {
        gate_room::read_git_item(definition, &item.uri)?
    } else {
        if item.uri.contains(':') {
            return Err(problem(
                RoomProblem::UnsupportedScheme,
                "link visible but scheme is not fetched",
            ));
        }
        let root = gate_room::safe_root(definition)?;
        let parent = parent(&root, entity)?;
        gate_room::read_file(&root, &artifact_path(&parent, &round.room, &item.uri)?)?
    };
    if crate::parser::gate_room::raw_digest(&bytes) != item.revision {
        return Err(problem(
            RoomProblem::ArtifactDigestMismatch,
            "artifact bytes differ from recorded revision",
        ));
    }
    Ok(bytes)
}
pub fn dependencies(
    definition: &WorkflowDefinition,
    entity: &Entity,
    history: &CorrectionHistory,
) -> Vec<PathBuf> {
    let Ok(root) = gate_room::safe_root(definition) else {
        return Vec::new();
    };
    let Ok(parent) = parent(&root, entity) else {
        return Vec::new();
    };
    let mut paths = vec![parent.join("review")];
    for round in &history.rounds {
        paths.extend([
            round.room.clone(),
            round.room.join("briefing.json"),
            round.room.join("briefing.review.jsonl"),
        ]);
        if let Some(briefing) = &round.briefing {
            for item in &briefing.items {
                if let Ok(path) = artifact_path(&parent, &round.room, &item.uri) {
                    paths.push(path);
                }
            }
        }
    }
    paths
}
