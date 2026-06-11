use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::config::ConfigEnv;

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SessionState {
    #[serde(default)]
    pub workflows: BTreeMap<String, WorkflowSession>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowSession {
    #[serde(default)]
    pub selected_entity_id: Option<String>,
    #[serde(default)]
    pub scope: WorkflowScope,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkflowScope {
    #[default]
    Active,
    Archived,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct WorkflowSessionKey(String);

impl WorkflowSessionKey {
    pub fn from_workflow_dir(path: &Path) -> Result<Self, SessionError> {
        let canonical = std::fs::canonicalize(path).map_err(SessionError::Io)?;
        Ok(Self(canonical.to_string_lossy().into_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("session path must be absolute: {0}")]
    UnsafePath(PathBuf),
    #[error("failed to read or write session state: {0}")]
    Io(std::io::Error),
    #[error("failed to parse or encode session state: {0}")]
    Parse(serde_yaml::Error),
}

pub fn state_path(env: &impl ConfigEnv) -> Option<PathBuf> {
    if let Some(path) = absolute_env_path(env, "XDG_STATE_HOME") {
        return Some(path.join("spacetop").join("session.yaml"));
    }
    absolute_env_path(env, "HOME").map(|home| {
        home.join(".local")
            .join("state")
            .join("spacetop")
            .join("session.yaml")
    })
}

pub fn load_session_file(path: &Path) -> Result<SessionState, SessionError> {
    ensure_absolute(path)?;
    match std::fs::read_to_string(path) {
        Ok(body) => serde_yaml::from_str(&body).map_err(SessionError::Parse),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(SessionState::default()),
        Err(err) => Err(SessionError::Io(err)),
    }
}

pub fn save_session_file(path: &Path, state: &SessionState) -> Result<(), SessionError> {
    ensure_absolute(path)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(SessionError::Io)?;
    }
    let body = serde_yaml::to_string(state).map_err(SessionError::Parse)?;
    std::fs::write(path, body).map_err(SessionError::Io)
}

fn absolute_env_path(env: &impl ConfigEnv, key: &str) -> Option<PathBuf> {
    let value = env.var(key)?;
    if value.is_empty() {
        return None;
    }
    let path = PathBuf::from(value);
    path.is_absolute().then_some(path)
}

fn ensure_absolute(path: &Path) -> Result<(), SessionError> {
    if path.is_absolute() {
        Ok(())
    } else {
        Err(SessionError::UnsafePath(path.to_path_buf()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ConfigEnv;
    use std::collections::{BTreeMap, HashMap};
    use std::path::{Path, PathBuf};

    struct TestEnv {
        vars: HashMap<String, String>,
    }

    impl ConfigEnv for TestEnv {
        fn var(&self, key: &str) -> Option<String> {
            self.vars.get(key).cloned()
        }
    }

    #[test]
    fn state_path_uses_xdg_state_home() {
        let env = TestEnv {
            vars: HashMap::from([
                ("XDG_STATE_HOME".to_string(), "/tmp/state".to_string()),
                ("HOME".to_string(), "/home/kent".to_string()),
            ]),
        };
        assert_eq!(
            state_path(&env),
            Some(PathBuf::from("/tmp/state/spacetop/session.yaml"))
        );
    }

    #[test]
    fn state_path_falls_back_to_home() {
        let env = TestEnv {
            vars: HashMap::from([("HOME".to_string(), "/home/kent".to_string())]),
        };
        assert_eq!(
            state_path(&env),
            Some(PathBuf::from(
                "/home/kent/.local/state/spacetop/session.yaml"
            ))
        );
    }

    #[test]
    fn relative_xdg_state_home_is_ignored() {
        let env = TestEnv {
            vars: HashMap::from([
                ("XDG_STATE_HOME".to_string(), "relative/state".to_string()),
                ("HOME".to_string(), "/home/kent".to_string()),
            ]),
        };
        assert_eq!(
            state_path(&env),
            Some(PathBuf::from(
                "/home/kent/.local/state/spacetop/session.yaml"
            ))
        );
    }

    #[test]
    fn empty_xdg_state_home_is_ignored() {
        let env = TestEnv {
            vars: HashMap::from([
                ("XDG_STATE_HOME".to_string(), String::new()),
                ("HOME".to_string(), "/home/kent".to_string()),
            ]),
        };
        assert_eq!(
            state_path(&env),
            Some(PathBuf::from(
                "/home/kent/.local/state/spacetop/session.yaml"
            ))
        );
    }

    #[test]
    fn relative_home_yields_no_state_path() {
        let env = TestEnv {
            vars: HashMap::from([("HOME".to_string(), "relative/home".to_string())]),
        };
        assert_eq!(state_path(&env), None);
    }

    #[test]
    fn missing_session_loads_default() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("missing.yaml");
        let loaded = load_session_file(&path).expect("load");
        assert_eq!(loaded, SessionState::default());
    }

    #[test]
    fn session_round_trips_yaml() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir
            .path()
            .join("state")
            .join("spacetop")
            .join("session.yaml");
        let mut workflows = BTreeMap::new();
        workflows.insert(
            "/repo/docs/workflow".to_string(),
            WorkflowSession {
                selected_entity_id: Some("050".to_string()),
                scope: WorkflowScope::Active,
            },
        );
        let state = SessionState { workflows };

        save_session_file(&path, &state).expect("save");
        let loaded = load_session_file(&path).expect("load");

        assert_eq!(loaded, state);
    }

    #[test]
    fn workflow_session_key_uses_canonical_absolute_path() {
        let dir = tempfile::tempdir().expect("tempdir");
        let workflow_dir = dir.path().join("repo").join("docs").join("workflow");
        std::fs::create_dir_all(&workflow_dir).expect("create workflow dir");
        let input = workflow_dir.join("..").join("workflow");

        let key = WorkflowSessionKey::from_workflow_dir(&input).expect("session key");
        let expected = std::fs::canonicalize(&workflow_dir).expect("canonical workflow dir");

        assert_eq!(key.as_str(), expected.as_path().to_str().expect("utf8"));
    }

    #[test]
    fn workflow_session_key_canonicalizes_existing_relative_path() {
        let cwd = std::env::current_dir().expect("cwd");
        let target_dir = cwd.join("target");
        std::fs::create_dir_all(&target_dir).expect("create target dir");
        let dir = tempfile::Builder::new()
            .prefix("spacetop-session-relative-")
            .tempdir_in(&target_dir)
            .expect("tempdir in target");
        let workflow_dir = dir.path().join("workflow");
        std::fs::create_dir_all(&workflow_dir).expect("create workflow dir");
        let relative = workflow_dir
            .strip_prefix(&cwd)
            .expect("target tempdir is under cwd");

        let key = WorkflowSessionKey::from_workflow_dir(relative).expect("session key");
        let expected = std::fs::canonicalize(&workflow_dir).expect("canonical workflow dir");

        assert_eq!(key.as_str(), expected.as_path().to_str().expect("utf8"));
    }

    #[test]
    fn workflow_session_key_rejects_paths_that_cannot_canonicalize() {
        let err = WorkflowSessionKey::from_workflow_dir(Path::new("missing/workflow"))
            .expect_err("missing paths cannot canonicalize");
        assert!(err.to_string().contains("failed to read or write"));
    }
}
