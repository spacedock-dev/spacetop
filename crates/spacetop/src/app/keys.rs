use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent};
use spacetop_core::config::{KeybindingConfig, SpacetopConfig};

use super::{OverviewSession, WorkflowSwitch};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResolvedKey {
    key: char,
    label: String,
}

impl ResolvedKey {
    fn new(key: char) -> Self {
        let label = match key {
            ' ' => "Space".to_string(),
            ch => ch.to_string(),
        };
        Self { key, label }
    }

    pub(crate) fn label(&self) -> &str {
        &self.label
    }

    fn matches(&self, code: KeyCode) -> bool {
        matches!(code, KeyCode::Char(ch) if ch == self.key)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResolvedKeymap {
    pub(crate) search: ResolvedKey,
    pub(crate) command: ResolvedKey,
    pub(crate) timeline: ResolvedKey,
    pub(crate) metrics: ResolvedKey,
    pub(crate) activity: ResolvedKey,
    pub(crate) relations: ResolvedKey,
    warnings: Vec<String>,
}

impl Default for ResolvedKeymap {
    fn default() -> Self {
        Self::from_config(&SpacetopConfig::default())
    }
}

impl ResolvedKeymap {
    pub(crate) fn from_config(config: &SpacetopConfig) -> Self {
        let specs = binding_specs(&config.keybindings);
        let parsed = specs
            .iter()
            .map(|spec| parse_binding(spec.configured))
            .collect::<Vec<_>>();
        let mut counts = HashMap::<char, usize>::new();
        for key in parsed.iter().flatten() {
            *counts.entry(*key).or_default() += 1;
        }

        let mut warnings = Vec::new();
        let mut resolved = Vec::with_capacity(specs.len());
        for (spec, parsed_key) in specs.iter().zip(parsed) {
            let binding = match parsed_key {
                None => {
                    warnings.push(format!(
                        "invalid keybinding for {}: expected a single printable character",
                        spec.name
                    ));
                    ResolvedBinding::fallback(spec, spec.default)
                }
                Some(key) if is_reserved_key(key) && key != spec.default => {
                    warnings.push(format!("reserved keybinding for {}: {key}", spec.name));
                    ResolvedBinding::fallback(spec, spec.default)
                }
                Some(key)
                    if counts.get(&key).copied().unwrap_or_default() > 1 && key != spec.default =>
                {
                    warnings.push(format!("duplicate keybinding for {}: {key}", spec.name));
                    ResolvedBinding::fallback(spec, spec.default)
                }
                Some(key) => ResolvedBinding::configured(spec, key),
            };
            resolved.push(binding);
        }
        resolve_final_duplicates(&mut resolved, &mut warnings);
        let resolved = resolved
            .into_iter()
            .map(|binding| ResolvedKey::new(binding.key))
            .collect::<Vec<_>>();

        Self {
            search: resolved[0].clone(),
            command: resolved[1].clone(),
            timeline: resolved[2].clone(),
            metrics: resolved[3].clone(),
            activity: resolved[4].clone(),
            relations: resolved[5].clone(),
            warnings,
        }
    }

    pub(crate) fn warnings(&self) -> &[String] {
        &self.warnings
    }
}

struct BindingSpec<'a> {
    name: &'static str,
    configured: &'a str,
    default: char,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BindingOrigin {
    Configured,
    Fallback,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ResolvedBinding {
    name: &'static str,
    default: char,
    key: char,
    origin: BindingOrigin,
}

impl ResolvedBinding {
    fn configured(spec: &BindingSpec<'_>, key: char) -> Self {
        Self {
            name: spec.name,
            default: spec.default,
            key,
            origin: BindingOrigin::Configured,
        }
    }

    fn fallback(spec: &BindingSpec<'_>, key: char) -> Self {
        Self {
            name: spec.name,
            default: spec.default,
            key,
            origin: BindingOrigin::Fallback,
        }
    }
}

fn binding_specs(config: &KeybindingConfig) -> [BindingSpec<'_>; 6] {
    [
        BindingSpec {
            name: "search",
            configured: &config.search,
            default: '/',
        },
        BindingSpec {
            name: "command",
            configured: &config.command,
            default: ':',
        },
        BindingSpec {
            name: "timeline",
            configured: &config.timeline,
            default: 'T',
        },
        BindingSpec {
            name: "metrics",
            configured: &config.metrics,
            default: 'M',
        },
        BindingSpec {
            name: "activity",
            configured: &config.activity,
            default: 'A',
        },
        BindingSpec {
            name: "relations",
            configured: &config.relations,
            default: 'R',
        },
    ]
}

fn parse_binding(value: &str) -> Option<char> {
    let mut chars = value.chars();
    let key = chars.next()?;
    if chars.next().is_some() || key.is_control() {
        return None;
    }
    Some(key)
}

fn is_reserved_key(key: char) -> bool {
    matches!(
        key,
        'a' | 's' | 'D' | 'Y' | '?' | 'q' | 'j' | 'k' | 'w' | 'o' | 'b' | 'g' | 'G' | 'P' | ' '
    )
}

fn resolve_final_duplicates(resolved: &mut [ResolvedBinding], warnings: &mut Vec<String>) {
    let duplicate_groups = duplicate_binding_groups(resolved);
    if duplicate_groups.is_empty() {
        return;
    }

    let mut keep = HashSet::new();
    for group in duplicate_groups {
        let winner = group
            .iter()
            .copied()
            .find(|index| resolved[*index].origin == BindingOrigin::Configured)
            .unwrap_or(group[0]);
        keep.insert(winner);
    }

    let mut reassign = duplicate_binding_groups(resolved)
        .into_iter()
        .flatten()
        .filter(|index| !keep.contains(index))
        .collect::<Vec<_>>();
    reassign.sort_unstable();
    reassign.dedup();

    let mut used = resolved
        .iter()
        .enumerate()
        .filter(|(index, _)| !reassign.contains(index))
        .map(|(_, binding)| binding.key)
        .collect::<HashSet<_>>();

    for index in reassign {
        let old_key = resolved[index].key;
        let replacement = fallback_key_for(resolved[index].default, &used);
        warnings.push(format!(
            "final duplicate keybinding for {}: {old_key}; using {replacement}",
            resolved[index].name
        ));
        resolved[index].key = replacement;
        resolved[index].origin = BindingOrigin::Fallback;
        used.insert(replacement);
    }
}

fn duplicate_binding_groups(resolved: &[ResolvedBinding]) -> Vec<Vec<usize>> {
    let mut groups = HashMap::<char, Vec<usize>>::new();
    for (index, binding) in resolved.iter().enumerate() {
        groups.entry(binding.key).or_default().push(index);
    }
    groups
        .into_values()
        .filter(|group| group.len() > 1)
        .collect()
}

fn fallback_key_for(default: char, used: &HashSet<char>) -> char {
    if !used.contains(&default) {
        return default;
    }
    ['/', ':', 'T', 'M', 'A', 'R']
        .into_iter()
        .find(|key| !used.contains(key))
        .unwrap_or(default)
}

pub(crate) enum OverviewKeyAction {
    None,
    OpenHelp,
    Quit,
    Switch(WorkflowSwitch),
    OpenPickerOverlay,
    OpenSelectedFile(PathBuf),
    /// `D` from Overview: open the full-pane Workflow Definition view.
    OpenDefinition,
    OpenSearch,
    OpenCommandPalette,
    OpenTimeline,
    OpenMetrics,
    OpenActivity,
    OpenRelations,
    /// `Y` from Overview: request a `git pull --ff-only` against the
    /// active workflow's repo root. Always emitted when the binding
    /// fires; the helper classifies availability and reports the result.
    RequestSync,
}

#[allow(dead_code)]
pub(crate) fn handle_overview_key(
    session: &mut OverviewSession,
    key: KeyEvent,
) -> OverviewKeyAction {
    handle_overview_key_with_keymap(session, key, &ResolvedKeymap::default())
}

pub(crate) fn handle_overview_key_with_keymap(
    session: &mut OverviewSession,
    key: KeyEvent,
    keymap: &ResolvedKeymap,
) -> OverviewKeyAction {
    let is_multi = session.is_multi();
    let pinned = session.pinned_single();
    let state = session.active_state_mut();
    match key.code {
        KeyCode::Char('?') => OverviewKeyAction::OpenHelp,
        KeyCode::Char('q') if state.preview_open() => {
            state.toggle_preview();
            OverviewKeyAction::None
        }
        KeyCode::Char('q') | KeyCode::Esc => OverviewKeyAction::Quit,
        KeyCode::Down | KeyCode::Char('j') => {
            state.select_next();
            OverviewKeyAction::None
        }
        KeyCode::Up | KeyCode::Char('k') => {
            state.select_previous();
            OverviewKeyAction::None
        }
        KeyCode::Enter => {
            state.toggle_preview();
            OverviewKeyAction::None
        }
        KeyCode::PageDown if state.preview_open() => {
            state.scroll_preview_down();
            OverviewKeyAction::None
        }
        KeyCode::PageUp if state.preview_open() => {
            state.scroll_preview_up();
            OverviewKeyAction::None
        }
        // less/vim-style page scroll inside the preview body. Space/b page, g/G
        // jump to the ends. Gated on preview_open so the list-mode bindings (s,
        // D, and a future plain g) stay free when the preview is closed. Arrow
        // keys and j/k deliberately stay task navigation.
        KeyCode::Char(' ') if state.preview_open() => {
            state.scroll_preview_down();
            OverviewKeyAction::None
        }
        KeyCode::Char('b') if state.preview_open() => {
            state.scroll_preview_up();
            OverviewKeyAction::None
        }
        KeyCode::Char('g') if state.preview_open() => {
            state.scroll_preview_to_top();
            OverviewKeyAction::None
        }
        KeyCode::Char('G') if state.preview_open() => {
            state.scroll_preview_to_bottom();
            OverviewKeyAction::None
        }
        KeyCode::PageDown => {
            state.page_selection_down();
            OverviewKeyAction::None
        }
        KeyCode::PageUp => {
            state.page_selection_up();
            OverviewKeyAction::None
        }
        KeyCode::Home => {
            state.select_first();
            OverviewKeyAction::None
        }
        KeyCode::End => {
            state.select_last();
            OverviewKeyAction::None
        }
        KeyCode::Char('a') => {
            state.toggle_scope();
            OverviewKeyAction::None
        }
        KeyCode::Right if state.preview_open() => {
            state.scroll_preview_right();
            OverviewKeyAction::None
        }
        KeyCode::Left if state.preview_open() => {
            state.scroll_preview_left();
            OverviewKeyAction::None
        }
        KeyCode::Char('w') if state.preview_open() => {
            state.toggle_preview_wrap();
            OverviewKeyAction::None
        }
        KeyCode::Char('o') if state.preview_open() => match state.selected_item() {
            Some(item) => {
                let target = item
                    .worktree_source
                    .clone()
                    .unwrap_or_else(|| item.path.clone());
                OverviewKeyAction::OpenSelectedFile(target)
            }
            None => OverviewKeyAction::None,
        },
        KeyCode::Char('s') if !state.preview_open() => {
            state.cycle_sort_mode();
            OverviewKeyAction::None
        }
        KeyCode::Char('D') if !state.preview_open() => OverviewKeyAction::OpenDefinition,
        code if keymap.search.matches(code) && !state.preview_open() => {
            OverviewKeyAction::OpenSearch
        }
        code if keymap.command.matches(code) && !state.preview_open() => {
            OverviewKeyAction::OpenCommandPalette
        }
        code if keymap.timeline.matches(code) && !state.preview_open() => {
            OverviewKeyAction::OpenTimeline
        }
        code if keymap.metrics.matches(code) && !state.preview_open() => {
            OverviewKeyAction::OpenMetrics
        }
        code if keymap.activity.matches(code) && !state.preview_open() => {
            OverviewKeyAction::OpenActivity
        }
        code if keymap.relations.matches(code) && !state.preview_open() => {
            OverviewKeyAction::OpenRelations
        }
        KeyCode::Char('Y') if !state.preview_open() => OverviewKeyAction::RequestSync,
        KeyCode::Right if is_multi => OverviewKeyAction::Switch(session.cycle_next()),
        KeyCode::Left if is_multi => OverviewKeyAction::Switch(session.cycle_prev()),
        KeyCode::Char('P') if is_multi && !pinned => OverviewKeyAction::OpenPickerOverlay,
        _ => OverviewKeyAction::None,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::PathBuf;

    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use super::{
        handle_overview_key, handle_overview_key_with_keymap, OverviewKeyAction, ResolvedKeymap,
    };
    use crate::app::{App, AppMode, OverviewSession, OverviewState};
    use spacetop_core::domain::{Entity, StageDefinition, WorkflowDefinition, WorkflowSnapshot};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn single_session_with_item(path: PathBuf) -> OverviewSession {
        single_session_with_item_worktree(path, None)
    }

    fn single_session_with_item_worktree(
        path: PathBuf,
        worktree_source: Option<PathBuf>,
    ) -> OverviewSession {
        let root = PathBuf::from("/tmp/spacetop-keys-test");
        let snapshot = WorkflowSnapshot {
            definition: WorkflowDefinition {
                root: root.clone(),
                state: None,
                stages: vec![StageDefinition {
                    name: "plan".to_string(),
                    initial: true,
                    terminal: false,
                    gate: false,
                    fresh: false,
                    feedback_to: None,
                    worktree: false,
                    concurrency: None,
                }],
                id_style: None,
                entity_type: None,
                entity_label: None,
                entity_label_plural: None,
                stage_colors: HashMap::new(),
                stage_prose: HashMap::new(),
                transitions: Vec::new(),
            },
            items: vec![Entity {
                path,
                id: "001".to_string(),
                title: "T".to_string(),
                status: "plan".to_string(),
                source: None,
                started: None,
                completed: None,
                verdict: None,
                score: None,
                worktree: None,
                issue: None,
                pr: None,
                body: String::new(),
                worktree_source,
                main_body: None,
            }],
            parse_errors: Vec::new(),
        };
        let state = OverviewState::from_snapshot(root, snapshot);
        OverviewSession::single(state, true)
    }

    /// AC-1: `o` with preview open emits an OpenSelectedFile intent carrying
    /// the selected item's absolute path.
    #[test]
    fn o_with_preview_open_emits_open_file_intent() {
        let expected_path = PathBuf::from("/tmp/spacetop-keys-test/task-001.md");
        let mut session = single_session_with_item(expected_path.clone());
        // Open the preview.
        session.active_state_mut().toggle_preview();
        assert!(session.active_state().preview_open());

        let action = handle_overview_key(&mut session, key(KeyCode::Char('o')));
        match action {
            OverviewKeyAction::OpenSelectedFile(path) => assert_eq!(path, expected_path),
            _ => panic!("expected OpenSelectedFile intent"),
        }
    }

    /// AC-2 (worktree branch): `o` on an item with `worktree_source = Some(_)`
    /// emits an OpenSelectedFile intent carrying the worktree-resident path,
    /// not the main-branch path.
    #[test]
    fn o_with_worktree_source_opens_worktree_path() {
        let main_path = PathBuf::from("/tmp/spacetop-keys-test/task-001.md");
        let worktree_path = PathBuf::from("/tmp/spacetop-keys-test/.worktrees/wt/task-001.md");
        let mut session =
            single_session_with_item_worktree(main_path.clone(), Some(worktree_path.clone()));
        session.active_state_mut().toggle_preview();
        assert!(session.active_state().preview_open());

        let action = handle_overview_key(&mut session, key(KeyCode::Char('o')));
        match action {
            OverviewKeyAction::OpenSelectedFile(path) => {
                assert_eq!(path, worktree_path);
                assert_ne!(path, main_path);
            }
            _ => panic!("expected OpenSelectedFile intent"),
        }
    }

    /// AC-2 (None branch): `o` on an item with `worktree_source = None`
    /// falls back to the main-branch path.
    #[test]
    fn o_without_worktree_source_opens_main_path() {
        let main_path = PathBuf::from("/tmp/spacetop-keys-test/task-001.md");
        let mut session = single_session_with_item_worktree(main_path.clone(), None);
        session.active_state_mut().toggle_preview();

        let action = handle_overview_key(&mut session, key(KeyCode::Char('o')));
        match action {
            OverviewKeyAction::OpenSelectedFile(path) => assert_eq!(path, main_path),
            _ => panic!("expected OpenSelectedFile intent"),
        }
    }

    /// AC-3: `o` with preview closed is a silent no-op (no intent recorded).
    #[test]
    fn o_with_preview_closed_is_noop() {
        let path = PathBuf::from("/tmp/spacetop-keys-test/task-001.md");
        let mut session = single_session_with_item(path);
        assert!(!session.active_state().preview_open());

        let action = handle_overview_key(&mut session, key(KeyCode::Char('o')));
        assert!(
            matches!(action, OverviewKeyAction::None),
            "expected None action when preview is closed"
        );
    }

    /// AC-1 (task 041): `D` from an overview with preview closed
    /// emits `OpenDefinition`.
    #[test]
    fn d_from_overview_emits_open_definition_action() {
        let path = PathBuf::from("/tmp/spacetop-keys-test/task-001.md");
        let mut session = single_session_with_item(path);
        assert!(!session.active_state().preview_open());
        let action = handle_overview_key(&mut session, key(KeyCode::Char('D')));
        assert!(
            matches!(action, OverviewKeyAction::OpenDefinition),
            "D with preview closed must emit OpenDefinition"
        );
    }

    #[test]
    fn p3_view_bindings_emit_actions_when_preview_closed() {
        let path = PathBuf::from("/tmp/spacetop-keys-test/task-001.md");
        let mut session = single_session_with_item(path);

        assert!(matches!(
            handle_overview_key(&mut session, key(KeyCode::Char('/'))),
            OverviewKeyAction::OpenSearch
        ));
        assert!(matches!(
            handle_overview_key(&mut session, key(KeyCode::Char(':'))),
            OverviewKeyAction::OpenCommandPalette
        ));
        assert!(matches!(
            handle_overview_key(&mut session, key(KeyCode::Char('T'))),
            OverviewKeyAction::OpenTimeline
        ));
        assert!(matches!(
            handle_overview_key(&mut session, key(KeyCode::Char('M'))),
            OverviewKeyAction::OpenMetrics
        ));
        assert!(matches!(
            handle_overview_key(&mut session, key(KeyCode::Char('A'))),
            OverviewKeyAction::OpenActivity
        ));
        assert!(matches!(
            handle_overview_key(&mut session, key(KeyCode::Char('R'))),
            OverviewKeyAction::OpenRelations
        ));
    }

    #[test]
    fn configured_search_key_opens_search() {
        let config = spacetop_core::config::SpacetopConfig {
            keybindings: spacetop_core::config::KeybindingConfig {
                search: "f".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };
        let keymap = ResolvedKeymap::from_config(&config);
        let path = PathBuf::from("/tmp/spacetop-keys-test/task-001.md");
        let mut session = single_session_with_item(path);

        let action =
            handle_overview_key_with_keymap(&mut session, key(KeyCode::Char('f')), &keymap);

        assert!(matches!(action, OverviewKeyAction::OpenSearch));
    }

    #[test]
    fn app_handle_key_uses_configured_search_key() {
        let config = spacetop_core::config::SpacetopConfig {
            keybindings: spacetop_core::config::KeybindingConfig {
                search: "f".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };
        let path = PathBuf::from("/tmp/spacetop-keys-test/task-001.md");
        let session = single_session_with_item(path);
        let mut app = App::from_session_with_config(session, config);

        app.handle_key(key(KeyCode::Char('f')));

        assert!(matches!(app.mode(), AppMode::Search { .. }));
    }

    #[test]
    fn duplicate_configured_keys_fall_back_to_defaults() {
        let config = spacetop_core::config::SpacetopConfig {
            keybindings: spacetop_core::config::KeybindingConfig {
                search: "f".to_string(),
                activity: "f".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };

        let resolved = ResolvedKeymap::from_config(&config);

        assert_eq!(resolved.search.label(), "/");
        assert_eq!(resolved.activity.label(), "A");
        assert!(resolved
            .warnings()
            .iter()
            .any(|warning| warning.contains("duplicate")));
    }

    #[test]
    fn reserved_overview_keys_fall_back_to_defaults() {
        let config = spacetop_core::config::SpacetopConfig {
            keybindings: spacetop_core::config::KeybindingConfig {
                search: "a".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };

        let resolved = ResolvedKeymap::from_config(&config);

        assert_eq!(resolved.search.label(), "/");
        assert!(resolved
            .warnings()
            .iter()
            .any(|warning| warning.contains("reserved")));
    }

    #[test]
    fn invalid_keybinding_strings_fall_back_to_defaults() {
        let config = spacetop_core::config::SpacetopConfig {
            keybindings: spacetop_core::config::KeybindingConfig {
                search: String::new(),
                command: "Ctrl-X".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };

        let resolved = ResolvedKeymap::from_config(&config);

        assert_eq!(resolved.search.label(), "/");
        assert_eq!(resolved.command.label(), ":");
        assert!(resolved
            .warnings()
            .iter()
            .any(|warning| warning.contains("invalid")));
    }

    #[test]
    fn invalid_search_fallback_does_not_collide_with_configured_command_slash() {
        let config = spacetop_core::config::SpacetopConfig {
            keybindings: spacetop_core::config::KeybindingConfig {
                search: String::new(),
                command: "/".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };

        let resolved = ResolvedKeymap::from_config(&config);
        let path = PathBuf::from("/tmp/spacetop-keys-test/task-001.md");
        let mut session = single_session_with_item(path);

        assert_ne!(resolved.search.label(), resolved.command.label());
        assert_eq!(resolved.command.label(), "/");
        assert!(resolved
            .warnings()
            .iter()
            .any(|warning| warning.contains("final duplicate")));
        assert!(matches!(
            handle_overview_key_with_keymap(
                &mut session,
                key(KeyCode::Char(resolved.command.key)),
                &resolved
            ),
            OverviewKeyAction::OpenCommandPalette
        ));
    }

    #[test]
    fn reserved_search_fallback_does_not_collide_with_configured_command_slash() {
        let config = spacetop_core::config::SpacetopConfig {
            keybindings: spacetop_core::config::KeybindingConfig {
                search: "a".to_string(),
                command: "/".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };

        let resolved = ResolvedKeymap::from_config(&config);
        let path = PathBuf::from("/tmp/spacetop-keys-test/task-001.md");
        let mut session = single_session_with_item(path);

        assert_ne!(resolved.search.label(), resolved.command.label());
        assert_eq!(resolved.command.label(), "/");
        assert!(resolved
            .warnings()
            .iter()
            .any(|warning| warning.contains("reserved")));
        assert!(resolved
            .warnings()
            .iter()
            .any(|warning| warning.contains("final duplicate")));
        assert!(matches!(
            handle_overview_key_with_keymap(
                &mut session,
                key(KeyCode::Char(resolved.command.key)),
                &resolved
            ),
            OverviewKeyAction::OpenCommandPalette
        ));
    }

    #[test]
    fn p3_view_bindings_are_ignored_when_preview_open() {
        let path = PathBuf::from("/tmp/spacetop-keys-test/task-001.md");
        let mut session = single_session_with_item(path);
        session.active_state_mut().toggle_preview();

        for code in ['/', ':', 'T', 'M', 'A', 'R'] {
            assert!(
                matches!(
                    handle_overview_key(&mut session, key(KeyCode::Char(code))),
                    OverviewKeyAction::None
                ),
                "{code} must be ignored while preview is open"
            );
        }
    }

    /// AC-1: `Y` from an overview with preview closed emits `RequestSync`.
    #[test]
    fn y_from_overview_emits_request_sync() {
        let path = PathBuf::from("/tmp/spacetop-keys-test/task-001.md");
        let mut session = single_session_with_item(path);
        assert!(!session.active_state().preview_open());
        let action = handle_overview_key(&mut session, key(KeyCode::Char('Y')));
        assert!(
            matches!(action, OverviewKeyAction::RequestSync),
            "Y with preview closed must emit RequestSync"
        );
    }

    /// AC-1: `Y` while the preview is open is a silent no-op so the
    /// binding doesn't collide with future preview shortcuts.
    #[test]
    fn y_with_preview_open_is_noop() {
        let path = PathBuf::from("/tmp/spacetop-keys-test/task-001.md");
        let mut session = single_session_with_item(path);
        session.active_state_mut().toggle_preview();
        assert!(session.active_state().preview_open());
        let action = handle_overview_key(&mut session, key(KeyCode::Char('Y')));
        assert!(
            matches!(action, OverviewKeyAction::None),
            "Y while preview is open must be a no-op"
        );
    }

    /// AC-1 (task 041): `D` while the preview is open is a silent no-op.
    #[test]
    fn d_with_preview_open_is_noop() {
        let path = PathBuf::from("/tmp/spacetop-keys-test/task-001.md");
        let mut session = single_session_with_item(path);
        session.active_state_mut().toggle_preview();
        assert!(session.active_state().preview_open());
        let action = handle_overview_key(&mut session, key(KeyCode::Char('D')));
        assert!(
            matches!(action, OverviewKeyAction::None),
            "D while preview is open must be a no-op"
        );
    }
}
