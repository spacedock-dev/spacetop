use std::path::{Path, PathBuf};

use spacetop_core::discovery::DiscoveredWorkflow;

use super::OverviewState;

/// Multi-workflow session: owns one [`OverviewState`] slot per discovered
/// workflow (lazy first-load), plus the active index and the scan root the
/// overlay re-discovery closure can use. Single-workflow constructors build a
/// 1-element session with `pinned_single = true` so the keymap and breadcrumb
/// pay zero UI cost when there is nothing to switch to.
#[derive(Debug, Clone, PartialEq)]
pub struct OverviewSession {
    scan_root: Option<PathBuf>,
    discovery: Vec<DiscoveredWorkflow>,
    workflows: Vec<Option<OverviewState>>,
    active: usize,
    pinned_single: bool,
}

/// A pending workflow switch produced by index-mutation handlers (`]`, `[`,
/// picker-overlay confirm). The event loop drains this after the key is
/// dispatched and performs the watcher teardown + load/reload + watcher
/// restart on the main thread.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowSwitch {
    pub target_index: usize,
    pub needs_first_load: bool,
}

impl OverviewSession {
    /// Build a single-workflow session from a pre-loaded state. `pinned`
    /// reflects the `-w/--workflow-dir` contract — when true, multi-workflow
    /// affordances stay hidden even if the discovery list later grows.
    pub fn single(state: OverviewState, pinned: bool) -> Self {
        let discovery = vec![DiscoveredWorkflow {
            root: state.workflow_dir().to_path_buf(),
            title: None,
        }];
        Self {
            scan_root: None,
            discovery,
            workflows: vec![Some(state)],
            active: 0,
            pinned_single: pinned,
        }
    }

    /// Build a multi-workflow session from a discovery result and a
    /// pre-loaded initial state at `initial_active`.
    pub fn from_discovery(
        scan_root: PathBuf,
        discovery: Vec<DiscoveredWorkflow>,
        initial_active: usize,
        initial_state: OverviewState,
    ) -> Self {
        let mut workflows: Vec<Option<OverviewState>> =
            (0..discovery.len()).map(|_| None).collect();
        let active = initial_active.min(discovery.len().saturating_sub(1));
        if active < workflows.len() {
            workflows[active] = Some(initial_state);
        }
        Self {
            scan_root: Some(scan_root),
            discovery,
            workflows,
            active,
            pinned_single: false,
        }
    }

    pub fn active_state(&self) -> &OverviewState {
        self.workflows[self.active]
            .as_ref()
            .expect("active workflow slot is materialized")
    }

    pub fn active_state_mut(&mut self) -> &mut OverviewState {
        self.workflows[self.active]
            .as_mut()
            .expect("active workflow slot is materialized")
    }

    pub fn discovery(&self) -> &[DiscoveredWorkflow] {
        &self.discovery
    }

    pub fn active_index(&self) -> usize {
        self.active
    }

    pub fn len(&self) -> usize {
        self.discovery.len()
    }

    pub fn is_empty(&self) -> bool {
        self.discovery.is_empty()
    }

    pub fn is_multi(&self) -> bool {
        self.discovery.len() >= 2 && !self.pinned_single
    }

    pub fn pinned_single(&self) -> bool {
        self.pinned_single
    }

    pub fn scan_root(&self) -> Option<&Path> {
        self.scan_root.as_deref()
    }

    fn slot_loaded(&self, index: usize) -> bool {
        self.workflows
            .get(index)
            .map(|slot| slot.is_some())
            .unwrap_or(false)
    }

    /// True when the currently active slot has a materialized `OverviewState`.
    /// Used by `App::reload_with_rediscovery` to decide whether to first-load
    /// the active slot after `replace_discovery` remapped it from an
    /// unloaded entry.
    pub fn active_slot_loaded(&self) -> bool {
        self.slot_loaded(self.active)
    }

    /// Active workflow path (canonical, from discovery).
    pub fn active_dir(&self) -> &Path {
        &self.discovery[self.active].root
    }

    pub fn cycle_next(&mut self) -> WorkflowSwitch {
        let len = self.discovery.len();
        let next = if len <= 1 {
            self.active
        } else {
            (self.active + 1) % len
        };
        self.select(next)
    }

    pub fn cycle_prev(&mut self) -> WorkflowSwitch {
        let len = self.discovery.len();
        let prev = if len <= 1 {
            self.active
        } else if self.active == 0 {
            len - 1
        } else {
            self.active - 1
        };
        self.select(prev)
    }

    pub fn select(&mut self, target_index: usize) -> WorkflowSwitch {
        let target = target_index.min(self.discovery.len().saturating_sub(1));
        let needs_first_load = !self.slot_loaded(target);
        self.active = target;
        WorkflowSwitch {
            target_index: target,
            needs_first_load,
        }
    }

    /// Replace the discovery list (e.g. from a re-discovery via `P`).
    /// Previously-loaded states are remapped by canonical-path match so we
    /// don't drop them unnecessarily; the active workflow is preserved if
    /// still present, otherwise active falls back to 0.
    pub fn replace_discovery(&mut self, new_discovery: Vec<DiscoveredWorkflow>) {
        let prior_active_root = self.discovery.get(self.active).map(|d| d.root.clone());
        let mut new_slots: Vec<Option<OverviewState>> =
            (0..new_discovery.len()).map(|_| None).collect();
        for (old_idx, slot) in self.workflows.drain(..).enumerate() {
            if let Some(state) = slot {
                let old_root = self.discovery.get(old_idx).map(|d| &d.root);
                if let Some(root) = old_root {
                    if let Some(new_idx) = new_discovery.iter().position(|d| &d.root == root) {
                        new_slots[new_idx] = Some(state);
                    }
                }
            }
        }
        let new_active = prior_active_root
            .as_ref()
            .and_then(|root| new_discovery.iter().position(|d| &d.root == root))
            .unwrap_or(0);
        self.discovery = new_discovery;
        self.workflows = new_slots;
        self.active = new_active.min(self.discovery.len().saturating_sub(1));
    }

    /// Materialize the active slot from a loaded `OverviewState`. Used by the
    /// event loop after a `WorkflowSwitch` with `needs_first_load == true`.
    pub fn install_active_state(&mut self, state: OverviewState) {
        if self.active < self.workflows.len() {
            self.workflows[self.active] = Some(state);
        }
    }
}
