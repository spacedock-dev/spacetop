//! Bounded, read-only evidence I/O. Rendering never accesses files or Git.
use crate::domain::*;
use crate::parser::gate_room::{self as decode, MAX_BYTES};
use std::fs;
use std::io::{BufReader, Read};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};

type Result<T> = std::result::Result<T, RoomDiagnostic>;
fn problem(kind: RoomProblem, message: impl Into<String>) -> RoomDiagnostic {
    RoomDiagnostic::new(kind, message)
}
fn io_problem(e: std::io::Error) -> RoomDiagnostic {
    problem(
        if e.kind() == std::io::ErrorKind::NotFound {
            RoomProblem::Missing
        } else {
            RoomProblem::UnsafePath
        },
        e.to_string(),
    )
}

/// A state checkout may remain readable as entity state without authorizing evidence reads.
pub fn safe_root(definition: &WorkflowDefinition) -> Result<PathBuf> {
    let definition_root = fs::canonicalize(&definition.root).map_err(io_problem)?;
    let root = definition.storage.entity_dir(&definition.root);
    reject_symlinks(root)?;
    let canonical = fs::canonicalize(root).map_err(io_problem)?;
    if !canonical.starts_with(&definition_root) {
        return Err(problem(
            RoomProblem::UnsupportedRoot,
            "state evidence root escapes definition root",
        ));
    }
    Ok(canonical)
}
fn reject_symlinks(path: &Path) -> Result<()> {
    // Check every component of the original absolute path, including the root.
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component);
        if fs::symlink_metadata(&current)
            .map_err(io_problem)?
            .file_type()
            .is_symlink()
        {
            return Err(problem(RoomProblem::Symlink, "symlink evidence component"));
        }
    }
    Ok(())
}
pub fn clean_relative(value: &str, leading_dot: bool) -> Result<PathBuf> {
    let value = if leading_dot {
        value.strip_prefix("./").unwrap_or(value)
    } else {
        value
    };
    if value.is_empty()
        || value.contains(['\\', '\0', '%', ':', '?', '#'])
        || value
            .split('/')
            .any(|s| s.is_empty() || s == "." || s == "..")
        || Path::new(value)
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(problem(
            RoomProblem::UnsafePath,
            "requires an unencoded clean relative path",
        ));
    }
    Ok(PathBuf::from(value))
}
fn resolve_room(root: &Path, entity: &Entity, attempt: &GateAttempt) -> Result<PathBuf> {
    if attempt.briefing.room_ref.contains(':') {
        return Err(problem(
            RoomProblem::UnsupportedScheme,
            "opaque/scheme room binding is display only",
        ));
    }
    let parent = entity
        .path
        .parent()
        .ok_or_else(|| problem(RoomProblem::UnsafePath, "missing entity parent"))?;
    reject_symlinks(parent)?;
    let parent = fs::canonicalize(parent).map_err(io_problem)?;
    if !parent.starts_with(root) {
        return Err(problem(
            RoomProblem::UnsupportedRoot,
            "entity origin is outside safe evidence root",
        ));
    }
    let candidate = parent.join(clean_relative(&attempt.briefing.room_ref, true)?);
    reject_symlinks(&candidate)?;
    let resolved = fs::canonicalize(&candidate).map_err(io_problem)?;
    if !resolved.starts_with(root) {
        return Err(problem(
            RoomProblem::UnsafePath,
            "room escapes evidence root",
        ));
    }
    Ok(resolved)
}

pub fn load(
    definition: &WorkflowDefinition,
    entity: &Entity,
    record: &GateRecord,
    attempt: &GateAttempt,
) -> GateRoomView {
    match load_verified(definition, entity, record, attempt) {
        Ok((format, briefing)) => GateRoomView {
            format: Some(format),
            briefing: Some(briefing),
            diagnostics: Vec::new(),
        },
        Err(e) => GateRoomView::failed(e),
    }
}
fn load_verified(
    definition: &WorkflowDefinition,
    entity: &Entity,
    record: &GateRecord,
    attempt: &GateAttempt,
) -> Result<(RoomFormat, CanonicalBriefing)> {
    let root = safe_root(definition)?;
    let room = resolve_room(&root, entity, attempt)?;
    let (format, file) = if attempt
        .briefing
        .request_digest
        .as_deref()
        .is_some_and(|v| !v.is_empty())
    {
        if !room.is_dir() {
            return Err(problem(
                RoomProblem::BindingMismatch,
                "request-backed binding must name directory",
            ));
        }
        let request = read_file(&root, &room.join("request.json"))?;
        let locator = decode::request_locator(&request, record, attempt)?;
        (
            RoomFormat::RequestBacked,
            room.join(clean_relative(&locator, false)?),
        )
    } else if room.is_dir() {
        match fs::symlink_metadata(room.join("briefing.json")) {
            Ok(_) => (RoomFormat::Retained, room.join("briefing.json")),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                match fs::symlink_metadata(room.join("index.json")) {
                    Ok(_) => (RoomFormat::Current, room.join("index.json")),
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                        (RoomFormat::Current, room.join("gate-briefing.json"))
                    }
                    Err(e) => return Err(io_problem(e)),
                }
            }
            Err(e) => return Err(io_problem(e)),
        }
    } else {
        (RoomFormat::ExactFile, room.clone())
    };
    if room.is_dir() && !file.starts_with(&room) {
        return Err(problem(RoomProblem::UnsafePath, "locator escapes room"));
    }
    let bytes = read_file(&root, &file)?;
    Ok((format, decode::decode_briefing(&bytes, record, attempt)?))
}

/// Reads selected bytes only; never launches an opener for unsupported references.
pub fn read_item(
    definition: &WorkflowDefinition,
    entity: &Entity,
    record: &GateRecord,
    attempt: &GateAttempt,
    item_id: &str,
) -> Result<Vec<u8>> {
    // Reverify the binding on every explicit selection; cached UI inventory is not authority.
    let (_, briefing) = load_verified(definition, entity, record, attempt)?;
    let item = briefing
        .items
        .iter()
        .find(|item| item.id == item_id)
        .ok_or_else(|| problem(RoomProblem::BindingMismatch, "item disappeared"))?;
    let bytes = if item.uri.starts_with("git-root://") {
        read_git_item(definition, &item.uri)?
    } else {
        if item.uri.contains(':') {
            return Err(problem(
                RoomProblem::UnsupportedScheme,
                "link visible but scheme is not fetched",
            ));
        }
        let root = safe_root(definition)?;
        let room = resolve_room(&root, entity, attempt)?;
        let dir = if room.is_dir() {
            room
        } else {
            room.parent()
                .ok_or_else(|| problem(RoomProblem::UnsafePath, "missing room parent"))?
                .to_path_buf()
        };
        read_file(&root, &dir.join(clean_relative(&item.uri, false)?))?
    };
    if decode::raw_digest(&bytes) != item.revision {
        return Err(problem(
            RoomProblem::ArtifactDigestMismatch,
            "selected bytes differ from raw SHA-256 revision",
        ));
    }
    Ok(bytes)
}

#[cfg(unix)]
fn read_file(root: &Path, path: &Path) -> Result<Vec<u8>> {
    read_file_checked(root, path, || {})
}
#[cfg(unix)]
fn read_file_checked(root: &Path, path: &Path, before_read: impl FnOnce()) -> Result<Vec<u8>> {
    use rustix::fs::{open, openat, Mode, OFlags};
    use std::os::unix::fs::MetadataExt;
    let relative = path
        .strip_prefix(root)
        .map_err(|_| problem(RoomProblem::UnsafePath, "file outside root"))?;
    if relative
        .components()
        .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(problem(RoomProblem::UnsafePath, "invalid file path"));
    }
    reject_symlinks(root)?;
    let root_before = fs::metadata(root).map_err(io_problem)?;
    let mut directory = open(
        root,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|e| io_problem(e.into()))?;
    let components = relative.components().collect::<Vec<_>>();
    if components.is_empty() {
        return Err(problem(
            RoomProblem::Nonregular,
            "evidence must be regular file",
        ));
    }
    for component in &components[..components.len() - 1] {
        directory = openat(
            &directory,
            Path::new(component.as_os_str()),
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|e| problem(RoomProblem::Symlink, format!("unsafe directory: {e}")))?;
    }
    let fd = openat(
        &directory,
        Path::new(components[components.len() - 1].as_os_str()),
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
        Mode::empty(),
    )
    .map_err(|e| io_problem(e.into()))?;
    let file = fs::File::from(fd);
    let before = file.metadata().map_err(io_problem)?;
    if !before.is_file() {
        return Err(problem(
            RoomProblem::Nonregular,
            "evidence is not regular file",
        ));
    }
    if before.len() > MAX_BYTES as u64 {
        return Err(problem(RoomProblem::SizeLimit, "evidence exceeds 2 MiB"));
    }
    before_read();
    let mut bytes = Vec::new();
    (&file)
        .take(MAX_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(io_problem)?;
    if bytes.len() > MAX_BYTES {
        return Err(problem(RoomProblem::SizeLimit, "evidence exceeds 2 MiB"));
    }
    reject_symlinks(path)?;
    let after = fs::metadata(path).map_err(io_problem)?;
    let root_after = fs::metadata(root).map_err(io_problem)?;
    if !fs::canonicalize(path)
        .map_err(io_problem)?
        .starts_with(root)
        || (
            before.dev(),
            before.ino(),
            before.len(),
            before.mtime(),
            before.mtime_nsec(),
            before.ctime(),
            before.ctime_nsec(),
        ) != (
            after.dev(),
            after.ino(),
            after.len(),
            after.mtime(),
            after.mtime_nsec(),
            after.ctime(),
            after.ctime_nsec(),
        )
        || (root_before.dev(), root_before.ino()) != (root_after.dev(), root_after.ino())
    {
        return Err(problem(
            RoomProblem::Replaced,
            "evidence path or bytes changed while reading",
        ));
    }
    Ok(bytes)
}
#[cfg(not(unix))]
fn read_file(_root: &Path, _path: &Path) -> Result<Vec<u8>> {
    Err(problem(
        RoomProblem::UnsupportedRoot,
        "race-safe evidence reads require Unix descriptor APIs",
    ))
}

fn git(root: &Path, args: &[&str]) -> Result<Vec<u8>> {
    if !matches!(
        args.first(),
        Some(&"rev-parse") | Some(&"ls-tree") | Some(&"cat-file")
    ) {
        return Err(problem(
            RoomProblem::UnsafePath,
            "only read Git plumbing is permitted",
        ));
    }
    let mut command = Command::new("git");
    for name in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_COMMON_DIR",
        "GIT_OBJECT_DIRECTORY",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
        "GIT_CONFIG",
        "GIT_CONFIG_COUNT",
    ] {
        command.env_remove(name);
    }
    let mut child = command
        .args(["-c", "protocol.allow=never", "--no-optional-locks"])
        .args(args)
        .current_dir(root)
        .env("GIT_NO_LAZY_FETCH", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(io_problem)?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| problem(RoomProblem::MissingObject, "Git stdout unavailable"))?;
    let mut bytes = Vec::new();
    BufReader::new(stdout)
        .take(MAX_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(io_problem)?;
    if bytes.len() > MAX_BYTES {
        let _ = child.kill();
        let _ = child.wait();
        return Err(problem(
            RoomProblem::SizeLimit,
            "Git evidence exceeds 2 MiB",
        ));
    }
    if !child.wait().map_err(io_problem)?.success() {
        return Err(problem(
            RoomProblem::MissingObject,
            "exact local Git object/probe unavailable; no fetch",
        ));
    }
    Ok(bytes)
}
fn git_top(root: &Path, run: &impl Fn(&Path, &[&str]) -> Result<Vec<u8>>) -> Result<PathBuf> {
    let bytes = run(root, &["rev-parse", "--show-toplevel"])?;
    let top = std::str::from_utf8(&bytes)
        .map_err(|_| problem(RoomProblem::UnsupportedRoot, "invalid Git root"))?
        .trim();
    let top = fs::canonicalize(top).map_err(io_problem)?;
    if !fs::canonicalize(root)
        .map_err(io_problem)?
        .starts_with(&top)
    {
        return Err(problem(
            RoomProblem::UnsupportedRoot,
            "Git top does not own workflow root",
        ));
    }
    Ok(top)
}
fn read_git_item(definition: &WorkflowDefinition, uri: &str) -> Result<Vec<u8>> {
    read_git_item_with(definition, uri, &git)
}
fn read_git_item_with(
    definition: &WorkflowDefinition,
    uri: &str,
    run: &impl Fn(&Path, &[&str]) -> Result<Vec<u8>>,
) -> Result<Vec<u8>> {
    let (logical, commit, path) = git_coordinates(uri)?;
    let root = safe_root(definition)?;
    let main = git_top(&definition.root, run)?;
    let repo = if logical == "main" {
        main.clone()
    } else {
        if !matches!(
            definition.storage,
            WorkflowStorage::SplitRoot {
                disposition: StateCheckoutDisposition::Attached
                    | StateCheckoutDisposition::Detached
                    | StateCheckoutDisposition::WrongBranch { .. },
                ..
            }
        ) {
            return Err(problem(
                RoomProblem::UnsupportedRoot,
                "state history is unverified",
            ));
        }
        let state = git_top(&root, run)?;
        if state != root || state == main {
            return Err(problem(
                RoomProblem::UnsupportedRoot,
                "distinct state history required",
            ));
        }
        state
    };
    let resolved = run(
        &repo,
        &["rev-parse", "--verify", &format!("{commit}^{{commit}}")],
    )?;
    if String::from_utf8_lossy(&resolved).trim() != commit {
        return Err(problem(
            RoomProblem::MissingObject,
            "requires exact full local commit",
        ));
    }
    let entry = run(
        &repo,
        &["ls-tree", "-z", &commit, "--", &format!(":(literal){path}")],
    )?;
    let text = std::str::from_utf8(&entry)
        .map_err(|_| problem(RoomProblem::Nonregular, "invalid tree entry"))?;
    let (header, name) = text
        .trim_end_matches('\0')
        .split_once('\t')
        .ok_or_else(|| problem(RoomProblem::MissingObject, "path absent from local commit"))?;
    let fields = header.split_whitespace().collect::<Vec<_>>();
    if fields.len() != 3
        || !matches!(fields[0], "100644" | "100755")
        || fields[1] != "blob"
        || name != path
        || text.matches('\0').count() != 1
    {
        return Err(problem(
            RoomProblem::Nonregular,
            "selected object is not exact regular blob",
        ));
    }
    let size = run(&repo, &["cat-file", "-s", fields[2]])?;
    if String::from_utf8_lossy(&size)
        .trim()
        .parse::<usize>()
        .map_err(|_| problem(RoomProblem::MissingObject, "invalid object size"))?
        > MAX_BYTES
    {
        return Err(problem(RoomProblem::SizeLimit, "Git blob exceeds 2 MiB"));
    }
    run(&repo, &["cat-file", "blob", fields[2]])
}
fn git_coordinates(uri: &str) -> Result<(String, String, String)> {
    let mut parts = uri
        .strip_prefix("git-root://")
        .ok_or_else(|| problem(RoomProblem::UnsupportedScheme, "unsupported link"))?
        .splitn(3, '/');
    let logical = parts.next().unwrap_or("");
    let commit = parts.next().unwrap_or("");
    let encoded = parts.next().unwrap_or("");
    if !matches!(logical, "main" | "state")
        || !matches!(commit.len(), 40 | 64)
        || !commit
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    {
        return Err(problem(
            RoomProblem::UnsafePath,
            "invalid Git root/full commit",
        ));
    }
    let mut segments = Vec::new();
    for encoded in encoded.split('/') {
        let mut decoded = Vec::new();
        let mut bytes = encoded.bytes();
        while let Some(b) = bytes.next() {
            if b == b'%' {
                let pair = [
                    bytes
                        .next()
                        .ok_or_else(|| problem(RoomProblem::UnsafePath, "invalid encoding"))?,
                    bytes
                        .next()
                        .ok_or_else(|| problem(RoomProblem::UnsafePath, "invalid encoding"))?,
                ];
                let hex = std::str::from_utf8(&pair)
                    .map_err(|_| problem(RoomProblem::UnsafePath, "invalid encoding"))?;
                decoded.push(
                    u8::from_str_radix(hex, 16)
                        .map_err(|_| problem(RoomProblem::UnsafePath, "invalid encoding"))?,
                );
            } else {
                decoded.push(b);
            }
        }
        let decoded = String::from_utf8(decoded)
            .map_err(|_| problem(RoomProblem::UnsafePath, "invalid UTF8 path"))?;
        // Go url.PathEscape's exact segment alphabet (not query escaping).
        let canonical = decoded
            .bytes()
            .map(|b| {
                if b.is_ascii_alphanumeric() || b"-_.~$&+:=@".contains(&b) {
                    (b as char).to_string()
                } else {
                    format!("%{b:02X}")
                }
            })
            .collect::<String>();
        if canonical != encoded
            || decoded.is_empty()
            || matches!(decoded.as_str(), "." | "..")
            || decoded.contains(['/', '\\', '\0'])
        {
            return Err(problem(
                RoomProblem::UnsafePath,
                "noncanonical/unsafe Git path segment",
            ));
        }
        segments.push(decoded);
    }
    Ok((logical.into(), commit.into(), segments.join("/")))
}

/// Event dependencies are coordinates only; callers never read an event path.
pub fn local_dependencies(
    definition: &WorkflowDefinition,
    entity: &Entity,
    attempt: &GateAttempt,
    view: &GateRoomView,
) -> Vec<PathBuf> {
    let Ok(root) = safe_root(definition) else {
        return Vec::new();
    };
    let Ok(room) = resolve_room(&root, entity, attempt) else {
        return Vec::new();
    };
    let directory = if room.is_dir() {
        room
    } else {
        match room.parent() {
            Some(p) => p.to_path_buf(),
            None => return Vec::new(),
        }
    };
    view.briefing
        .as_ref()
        .map(|b| {
            b.items
                .iter()
                .filter_map(|i| {
                    clean_relative(&i.uri, false)
                        .ok()
                        .map(|p| directory.join(p))
                })
                .filter(|p| p.starts_with(&root) && reject_existing_symlinks(p, &root))
                .collect()
        })
        .unwrap_or_default()
}
fn reject_existing_symlinks(path: &Path, root: &Path) -> bool {
    let Ok(relative) = path.strip_prefix(root) else {
        return false;
    };
    let mut current = root.to_path_buf();
    for c in relative.components() {
        current.push(c);
        match fs::symlink_metadata(&current) {
            Ok(m) if m.file_type().is_symlink() => return false,
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return false,
        }
    }
    true
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    #[test]
    fn descriptor_read_rejects_final_and_directory_replacement_races() {
        use std::os::unix::fs::symlink;
        let temp = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(temp.path()).unwrap();
        fs::create_dir(root.join("room")).unwrap();
        let path = root.join("room/a.md");
        fs::write(&path, "original").unwrap();
        let result = read_file_checked(&root, &path, || {
            fs::rename(&path, root.join("original.md")).unwrap();
            fs::write(&path, "replacement").unwrap();
        });
        assert_eq!(result.unwrap_err().kind, RoomProblem::Replaced);
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("a.md"), "outside secret").unwrap();
        let result = read_file_checked(&root, &path, || {
            fs::rename(root.join("room"), root.join("old-room")).unwrap();
            symlink(outside.path(), root.join("room")).unwrap();
        });
        assert_eq!(result.unwrap_err().kind, RoomProblem::Symlink);
    }
    #[test]
    fn fake_git_audit_uses_only_exact_local_read_plumbing() {
        use std::cell::RefCell;
        let temp = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(temp.path()).unwrap();
        let mut definition = crate::parser::parse_workflow_readme(
            &PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/fixtures/durable-gates/README.md"),
        )
        .unwrap();
        definition.root = root.clone();
        definition.storage = WorkflowStorage::SingleRoot;
        let commit = "a".repeat(40);
        let object = "b".repeat(40);
        let calls = RefCell::new(Vec::new());
        let fake = |dir: &Path, args: &[&str]| {
            assert_eq!(dir, root);
            calls
                .borrow_mut()
                .push(args.iter().map(|s| s.to_string()).collect::<Vec<_>>());
            Ok(match args {
                ["rev-parse", "--show-toplevel"] => format!("{}\n", root.display()).into_bytes(),
                ["rev-parse", "--verify", identity] => {
                    assert_eq!(*identity, format!("{commit}^{{commit}}"));
                    format!("{commit}\n").into_bytes()
                }
                ["ls-tree", "-z", selected, "--", literal] => {
                    assert_eq!(*selected, commit);
                    assert_eq!(*literal, ":(literal)evidence.md");
                    format!("100644 blob {object}\tevidence.md\0").into_bytes()
                }
                ["cat-file", "-s", id] => {
                    assert_eq!(*id, object);
                    b"7\n".to_vec()
                }
                ["cat-file", "blob", id] => {
                    assert_eq!(*id, object);
                    b"fixture".to_vec()
                }
                _ => panic!("unexpected Git command {args:?}"),
            })
        };
        assert_eq!(
            read_git_item_with(
                &definition,
                &format!("git-root://main/{commit}/evidence.md"),
                &fake
            )
            .unwrap(),
            b"fixture"
        );
        assert_eq!(calls.borrow().len(), 5);
    }
}
