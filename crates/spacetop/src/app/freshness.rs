//! Session-only observations. No filesystem time or remote-currency inference.
use std::path::{Path, PathBuf};
use std::time::Instant;

use spacetop_core::domain::{StateCheckoutDisposition, WorkflowStorage};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Attempt {
    #[default]
    Unknown,
    InFlight,
    Succeeded,
    Failed(String),
    Skipped(String),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Observation {
    pub last_success: Option<Instant>,
    pub latest: Attempt,
}

impl Observation {
    pub fn record(&mut self, attempt: Attempt, completed: Instant) {
        if attempt == Attempt::Succeeded {
            self.last_success = Some(completed);
        }
        self.latest = attempt;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StateTarget {
    declared_path: PathBuf,
    root: PathBuf,
    expected_branch: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Freshness {
    pub local: Observation,
    pub definition: Observation,
    pub state: Observation,
    definition_root: Option<PathBuf>,
    state_target: Option<StateTarget>,
    pub split_root: bool,
}

impl Freshness {
    /// Bind observations to the loaded target. An unverified identity has no
    /// current-target history; never transplant an old success onto it.
    pub fn bind(&mut self, definition_root: &Path, storage: &WorkflowStorage) {
        let root = definition_root.canonicalize().ok();
        if self.definition_root != root {
            self.definition = Observation::default();
            self.definition_root = root;
        }
        self.split_root = matches!(storage, WorkflowStorage::SplitRoot { .. });
        let target = match storage {
            WorkflowStorage::SplitRoot {
                entity_dir,
                expected_branch,
                disposition,
            } if !matches!(disposition, StateCheckoutDisposition::Unverified { .. }) => {
                let prior_missing_target = self.state_target.as_ref().filter(|prior| {
                    matches!(disposition, StateCheckoutDisposition::Missing)
                        && prior.declared_path == *entity_dir
                        && prior.expected_branch == *expected_branch
                });
                Some(prior_missing_target.cloned().unwrap_or_else(|| {
                    StateTarget {
                        declared_path: entity_dir.clone(),
                        root: entity_dir
                            .canonicalize()
                            .unwrap_or_else(|_| entity_dir.clone()),
                        expected_branch: expected_branch.clone(),
                    }
                }))
            }
            _ => None,
        };
        if self.state_target != target || target.is_none() {
            self.state = Observation::default();
            self.state_target = target;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn only_success_advances_observation() {
        let t1 = Instant::now();
        let t2 = t1 + Duration::from_secs(20);
        let mut observation = Observation::default();
        assert_eq!(observation.last_success, None);
        observation.record(Attempt::Succeeded, t1);
        for attempt in [
            Attempt::InFlight,
            Attempt::Failed("offline".into()),
            Attempt::Skipped("detached".into()),
        ] {
            observation.record(attempt.clone(), t2);
            assert_eq!(observation.last_success, Some(t1));
            assert_eq!(observation.latest, attempt);
        }
        observation.record(Attempt::Succeeded, t2);
        assert_eq!(observation.last_success, Some(t2));
    }

    #[test]
    fn state_identity_changes_invalidate_only_state_history() {
        let dir = tempfile::tempdir().unwrap();
        let mut freshness = Freshness::default();
        let storage = |branch: &str| WorkflowStorage::SplitRoot {
            entity_dir: dir.path().join("state"),
            expected_branch: branch.into(),
            disposition: StateCheckoutDisposition::Missing,
        };
        freshness.bind(dir.path(), &storage("one"));
        let t = Instant::now();
        freshness.definition.record(Attempt::Succeeded, t);
        freshness.state.record(Attempt::Succeeded, t);
        freshness.bind(dir.path(), &storage("one"));
        assert_eq!(freshness.state.last_success, Some(t));
        freshness.bind(dir.path(), &storage("two"));
        assert_eq!(freshness.state.last_success, None);
        assert_eq!(freshness.definition.last_success, Some(t));
        freshness.state.record(Attempt::Succeeded, t);
        freshness.bind(dir.path(), &WorkflowStorage::SingleRoot);
        freshness.bind(dir.path(), &storage("two"));
        assert_eq!(freshness.state.last_success, None);
    }
    #[test]
    fn changed_or_unverified_targets_never_inherit_success() {
        use spacetop_core::domain::StateTopologyProblem;
        let dir = tempfile::tempdir().unwrap();
        let other = tempfile::tempdir().unwrap();
        let storage = |name: &str, disposition| WorkflowStorage::SplitRoot {
            entity_dir: dir.path().join(name),
            expected_branch: "state/main".into(),
            disposition,
        };
        let mut freshness = Freshness::default();
        let first = storage("first", StateCheckoutDisposition::Missing);
        freshness.bind(dir.path(), &first);
        let t = Instant::now();
        freshness.definition.record(Attempt::Succeeded, t);
        freshness.state.record(Attempt::Succeeded, t);
        freshness.bind(
            dir.path(),
            &storage("second", StateCheckoutDisposition::Missing),
        );
        assert_eq!(freshness.state.last_success, None);
        freshness.state.record(Attempt::Succeeded, t);
        freshness.bind(
            dir.path(),
            &storage(
                "second",
                StateCheckoutDisposition::Unverified {
                    problem: StateTopologyProblem::EmptyBranch,
                },
            ),
        );
        assert_eq!(freshness.state.last_success, None);
        assert_eq!(freshness.definition.last_success, Some(t));
        freshness.bind(other.path(), &first);
        assert_eq!(freshness.definition.last_success, None);
    }
    #[cfg(unix)]
    #[test]
    fn missing_same_target_retains_success_even_through_symlinked_parent() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("real");
        let alias = dir.path().join("alias");
        std::fs::create_dir(&real).unwrap();
        std::os::unix::fs::symlink(&real, &alias).unwrap();
        let state_dir = alias.join("state");
        std::fs::create_dir(&state_dir).unwrap();
        let storage = |disposition| WorkflowStorage::SplitRoot {
            entity_dir: state_dir.clone(),
            expected_branch: "main".into(),
            disposition,
        };
        let mut freshness = Freshness::default();
        freshness.bind(dir.path(), &storage(StateCheckoutDisposition::Attached));
        let t = Instant::now();
        freshness.state.record(Attempt::Succeeded, t);
        std::fs::remove_dir(&state_dir).unwrap();
        freshness.bind(dir.path(), &storage(StateCheckoutDisposition::Missing));
        freshness.state.record(
            Attempt::Skipped("missing".into()),
            t + Duration::from_secs(1),
        );
        assert_eq!(freshness.state.last_success, Some(t));
    }
}
