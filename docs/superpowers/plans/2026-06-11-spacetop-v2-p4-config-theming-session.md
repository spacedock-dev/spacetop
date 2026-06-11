# SpaceTop v2 - Phase P4: Config, Theming, and Session Persistence Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add YAML config, theme defaults, configurable keybindings, and per-workflow session persistence shared by TUI and future headless commands.

**Architecture:** Configuration lives in `spacetop-core` and is loaded from XDG-style user paths using only `std`: `$XDG_CONFIG_HOME/spacetop/config.yaml` or `~/.config/spacetop/config.yaml`. Session state is stored separately under `$XDG_STATE_HOME/spacetop/session.yaml` or `~/.local/state/spacetop/session.yaml`. Spacetop never writes into workflow directories for config/session persistence.

**Tech Stack:** Rust 2021, `serde`, `serde_yaml`, `std::env`, `std::fs`, Ratatui color conversion in the bin crate.

---

## Prerequisites

- P0 through P3 are merged.
- The TUI already renders through query-backed app state.
- This phase may write only to the user's config/state directory, never to Spacedock workflow markdown.

## Decisions resolved in this plan

- **Format:** YAML, because `serde_yaml` already exists.
- **Config path:** `$XDG_CONFIG_HOME/spacetop/config.yaml`, fallback `~/.config/spacetop/config.yaml`.
- **State path:** `$XDG_STATE_HOME/spacetop/session.yaml`, fallback `~/.local/state/spacetop/session.yaml`.
- **Workflow-local config:** read-only support is deferred. P4 does not read or write config from the workflow tree.

## Hard constraints

- XDG and HOME-derived config/state paths must be absolute. Ignore relative `XDG_CONFIG_HOME`, `XDG_STATE_HOME`, and `HOME` values and fall back to the next valid absolute path.
- Config parse errors must be preserved as user-visible warnings; do not silently replace malformed config with defaults.
- Session scope must be typed, not raw strings.
- Keybindings must resolve through a validated `ResolvedKeymap` before app input handling. Duplicates or reserved-key collisions fall back to defaults with a warning.

## File map

- Create: `crates/spacetop-core/src/config.rs`
- Create: `crates/spacetop-core/src/session_state.rs`
- Modify: `crates/spacetop-core/src/lib.rs`
- Modify: `crates/spacetop/src/lib.rs`
- Modify: `crates/spacetop/src/app.rs`
- Modify: `crates/spacetop/src/app/keys.rs`
- Modify: `crates/spacetop/src/ui/color.rs`
- Modify: `crates/spacetop/src/ui/footer.rs`
- Modify: `crates/spacetop/src/ui/help.rs`
- Modify: `README.md`

---

## Task 1: Add core config model and path resolution

**Files:**
- Create: `crates/spacetop-core/src/config.rs`
- Modify: `crates/spacetop-core/src/lib.rs`

- [ ] **Step 1: Write config tests**

Create `crates/spacetop-core/src/config.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::path::PathBuf;

    #[test]
    fn default_config_preserves_existing_behavior() {
        let config = SpacetopConfig::default();
        assert_eq!(config.defaults.sort, DefaultSort::Id);
        assert_eq!(config.defaults.scope, DefaultScope::Active);
        assert_eq!(config.keybindings.search, "/");
        assert_eq!(config.keybindings.metrics, "M");
    }

    #[test]
    fn config_path_uses_xdg_config_home() {
        let env = TestEnv {
            vars: HashMap::from([
                ("XDG_CONFIG_HOME".to_string(), "/tmp/config".to_string()),
                ("HOME".to_string(), "/home/kent".to_string()),
            ]),
        };
        assert_eq!(
            config_path(&env),
            Some(PathBuf::from("/tmp/config/spacetop/config.yaml"))
        );
    }

    #[test]
    fn config_path_falls_back_to_home() {
        let env = TestEnv {
            vars: HashMap::from([("HOME".to_string(), "/home/kent".to_string())]),
        };
        assert_eq!(
            config_path(&env),
            Some(PathBuf::from("/home/kent/.config/spacetop/config.yaml"))
        );
    }

    #[test]
    fn relative_xdg_config_home_is_ignored() {
        let env = TestEnv {
            vars: HashMap::from([
                ("XDG_CONFIG_HOME".to_string(), "relative/config".to_string()),
                ("HOME".to_string(), "/home/kent".to_string()),
            ]),
        };
        assert_eq!(
            config_path(&env),
            Some(PathBuf::from("/home/kent/.config/spacetop/config.yaml"))
        );
    }
}
```

- [ ] **Step 2: Run and verify it fails**

Run: `cargo test -p spacetop-core config::tests`

Expected: FAIL because config types do not exist.

- [ ] **Step 3: Implement config model**

Add:

```rust
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpacetopConfig {
    #[serde(default)]
    pub theme: ThemeConfig,
    #[serde(default)]
    pub keybindings: KeybindingConfig,
    #[serde(default)]
    pub defaults: DefaultsConfig,
}

impl Default for SpacetopConfig {
    fn default() -> Self {
        Self {
            theme: ThemeConfig::default(),
            keybindings: KeybindingConfig::default(),
            defaults: DefaultsConfig::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThemeConfig {
    #[serde(default = "default_selection_bg")]
    pub selection_bg: String,
    #[serde(default = "default_footer_bg")]
    pub footer_bg: String,
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            selection_bg: default_selection_bg(),
            footer_bg: default_footer_bg(),
        }
    }
}

fn default_selection_bg() -> String {
    "#283454".to_string()
}

fn default_footer_bg() -> String {
    "#3b4252".to_string()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeybindingConfig {
    #[serde(default = "key_search")]
    pub search: String,
    #[serde(default = "key_command")]
    pub command: String,
    #[serde(default = "key_timeline")]
    pub timeline: String,
    #[serde(default = "key_metrics")]
    pub metrics: String,
    #[serde(default = "key_activity")]
    pub activity: String,
    #[serde(default = "key_relations")]
    pub relations: String,
}

impl Default for KeybindingConfig {
    fn default() -> Self {
        Self {
            search: key_search(),
            command: key_command(),
            timeline: key_timeline(),
            metrics: key_metrics(),
            activity: key_activity(),
            relations: key_relations(),
        }
    }
}

fn key_search() -> String { "/".to_string() }
fn key_command() -> String { ":".to_string() }
fn key_timeline() -> String { "T".to_string() }
fn key_metrics() -> String { "M".to_string() }
fn key_activity() -> String { "A".to_string() }
fn key_relations() -> String { "R".to_string() }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DefaultsConfig {
    #[serde(default)]
    pub sort: DefaultSort,
    #[serde(default)]
    pub scope: DefaultScope,
}

impl Default for DefaultsConfig {
    fn default() -> Self {
        Self {
            sort: DefaultSort::Id,
            scope: DefaultScope::Active,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DefaultSort {
    Id,
    Status,
}

impl Default for DefaultSort {
    fn default() -> Self {
        Self::Id
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DefaultScope {
    Active,
    Archived,
}

impl Default for DefaultScope {
    fn default() -> Self {
        Self::Active
    }
}

pub trait ConfigEnv {
    fn var(&self, key: &str) -> Option<String>;
}

pub struct StdEnv;

impl ConfigEnv for StdEnv {
    fn var(&self, key: &str) -> Option<String> {
        std::env::var(key).ok()
    }
}

#[cfg(test)]
struct TestEnv {
    vars: std::collections::HashMap<String, String>,
}

#[cfg(test)]
impl ConfigEnv for TestEnv {
    fn var(&self, key: &str) -> Option<String> {
        self.vars.get(key).cloned()
    }
}

pub fn config_path(env: &impl ConfigEnv) -> Option<PathBuf> {
    if let Some(xdg) = env.var("XDG_CONFIG_HOME").filter(|s| !s.is_empty()) {
        let path = PathBuf::from(xdg);
        if path.is_absolute() {
            return Some(path.join("spacetop/config.yaml"));
        }
    }
    env.var("HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .map(|home| home.join(".config/spacetop/config.yaml"))
}
```

- [ ] **Step 4: Export and verify**

In `lib.rs`, add:

```rust
pub mod config;
```

Run: `cargo test -p spacetop-core config::tests`

Expected: PASS.

```bash
git add crates/spacetop-core/src/config.rs crates/spacetop-core/src/lib.rs
git commit -m "feat(core): add config model and XDG path resolution"
```

---

## Task 2: Load config from YAML with graceful fallback

**Files:**
- Modify: `crates/spacetop-core/src/config.rs`

- [ ] **Step 1: Add load tests**

Add:

```rust
#[test]
fn missing_config_loads_default() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("missing.yaml");
    let config = load_config_file(&path).expect("load config");
    assert_eq!(config, SpacetopConfig::default());
}

#[test]
fn partial_config_merges_with_defaults() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("config.yaml");
    std::fs::write(&path, "keybindings:\n  search: f\n").expect("write");

    let config = load_config_file(&path).expect("load config");
    assert_eq!(config.keybindings.search, "f");
    assert_eq!(config.keybindings.metrics, "M");
}

#[test]
fn malformed_config_returns_default_with_warning() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("config.yaml");
    std::fs::write(&path, "keybindings: [").expect("write");

    let load = load_config_file_with_warnings(&path).expect("load config");
    assert_eq!(load.config, SpacetopConfig::default());
    assert_eq!(load.warnings.len(), 1);
    assert!(load.warnings[0].message.contains("failed to parse config"));
}
```

- [ ] **Step 2: Run and verify it fails**

Run: `cargo test -p spacetop-core config::tests::missing_config_loads_default config::tests::partial_config_merges_with_defaults config::tests::malformed_config_returns_default_with_warning`

Expected: FAIL because `load_config_file_with_warnings` does not exist.

- [ ] **Step 3: Implement loading with warnings**

Add:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigLoad {
    pub config: SpacetopConfig,
    pub warnings: Vec<ConfigWarning>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigWarning {
    pub message: String,
}

pub fn load_config(env: &impl ConfigEnv) -> Result<SpacetopConfig, ConfigError> {
    match config_path(env) {
        Some(path) => load_config_file(&path),
        None => Ok(SpacetopConfig::default()),
    }
}

pub fn load_config_with_warnings(env: &impl ConfigEnv) -> Result<ConfigLoad, ConfigError> {
    match config_path(env) {
        Some(path) => load_config_file_with_warnings(&path),
        None => Ok(ConfigLoad {
            config: SpacetopConfig::default(),
            warnings: Vec::new(),
        }),
    }
}

pub fn load_config_file(path: &std::path::Path) -> Result<SpacetopConfig, ConfigError> {
    match std::fs::read_to_string(path) {
        Ok(body) => serde_yaml::from_str(&body).map_err(ConfigError::Parse),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(SpacetopConfig::default()),
        Err(err) => Err(ConfigError::Io(err)),
    }
}

pub fn load_config_file_with_warnings(path: &std::path::Path) -> Result<ConfigLoad, ConfigError> {
    match std::fs::read_to_string(path) {
        Ok(body) => match serde_yaml::from_str(&body) {
            Ok(config) => Ok(ConfigLoad {
                config,
                warnings: Vec::new(),
            }),
            Err(err) => Ok(ConfigLoad {
                config: SpacetopConfig::default(),
                warnings: vec![ConfigWarning {
                    message: format!("failed to parse config: {err}"),
                }],
            }),
        },
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(ConfigLoad {
            config: SpacetopConfig::default(),
            warnings: Vec::new(),
        }),
        Err(err) => Err(ConfigError::Io(err)),
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("failed to read config: {0}")]
    Io(std::io::Error),
    #[error("failed to parse config: {0}")]
    Parse(serde_yaml::Error),
}
```

- [ ] **Step 4: Verify and commit**

Run: `cargo test -p spacetop-core config::tests`

Expected: PASS.

```bash
git add crates/spacetop-core/src/config.rs
git commit -m "feat(core): load YAML config with defaults"
```

---

## Task 3: Add session state model and state path

**Files:**
- Create: `crates/spacetop-core/src/session_state.rs`
- Modify: `crates/spacetop-core/src/lib.rs`

- [ ] **Step 1: Write session tests**

Create `crates/spacetop-core/src/session_state.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ConfigEnv;
    use std::collections::HashMap;
    use std::path::PathBuf;

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
    fn relative_xdg_state_home_is_ignored() {
        let env = TestEnv {
            vars: HashMap::from([
                ("XDG_STATE_HOME".to_string(), "relative/state".to_string()),
                ("HOME".to_string(), "/home/kent".to_string()),
            ]),
        };
        assert_eq!(
            state_path(&env),
            Some(PathBuf::from("/home/kent/.local/state/spacetop/session.yaml"))
        );
    }

    #[test]
    fn session_round_trips_yaml() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("session.yaml");
        let mut state = SessionState::default();
        state.workflows.insert(
            "/repo/docs/workflow".to_string(),
            WorkflowSession {
                selected_entity_id: Some("050".to_string()),
                scope: WorkflowScope::Active,
            },
        );
        save_session_file(&path, &state).expect("save");
        let loaded = load_session_file(&path).expect("load");
        assert_eq!(loaded, state);
    }
}
```

- [ ] **Step 2: Run and verify it fails**

Run: `cargo test -p spacetop-core session_state::tests`

Expected: FAIL because session state does not exist.

- [ ] **Step 3: Implement session state**

Add:

```rust
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
    pub selected_entity_id: Option<String>,
    pub scope: WorkflowScope,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkflowScope {
    Active,
    Archived,
}

pub fn state_path(env: &impl ConfigEnv) -> Option<PathBuf> {
    if let Some(xdg) = env.var("XDG_STATE_HOME").filter(|s| !s.is_empty()) {
        let path = PathBuf::from(xdg);
        if path.is_absolute() {
            return Some(path.join("spacetop/session.yaml"));
        }
    }
    env.var("HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .map(|home| home.join(".local/state/spacetop/session.yaml"))
}

pub fn load_session_file(path: &Path) -> Result<SessionState, SessionError> {
    match std::fs::read_to_string(path) {
        Ok(body) => serde_yaml::from_str(&body).map_err(SessionError::Parse),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(SessionState::default()),
        Err(err) => Err(SessionError::Io(err)),
    }
}

pub fn save_session_file(path: &Path, state: &SessionState) -> Result<(), SessionError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(SessionError::Io)?;
    }
    let body = serde_yaml::to_string(state).map_err(SessionError::Parse)?;
    std::fs::write(path, body).map_err(SessionError::Io)
}

#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("failed to read or write session state: {0}")]
    Io(std::io::Error),
    #[error("failed to parse or encode session state: {0}")]
    Parse(serde_yaml::Error),
}
```

- [ ] **Step 4: Export and verify**

In `lib.rs`, add:

```rust
pub mod session_state;
```

Run: `cargo test -p spacetop-core session_state::tests`

Expected: PASS.

```bash
git add crates/spacetop-core/src/lib.rs crates/spacetop-core/src/session_state.rs
git commit -m "feat(core): add XDG session persistence model"
```

---

## Task 4: Apply config at TUI startup

**Files:**
- Modify: `crates/spacetop/src/lib.rs`
- Modify: `crates/spacetop/src/app.rs`
- Modify: `crates/spacetop/src/app/keys.rs`

- [ ] **Step 1: Add config-carrying app test**

Add an app test:

```rust
#[test]
fn app_stores_config_for_key_handling() {
    let config = spacetop_core::config::SpacetopConfig {
        keybindings: spacetop_core::config::KeybindingConfig {
            search: "f".to_string(),
            ..Default::default()
        },
        ..Default::default()
    };
    let app = App::new_with_config("/tmp/workflow", config.clone());
    assert_eq!(app.config().keybindings.search, "f");
}
```

Add defaults-precedence tests:

```rust
#[test]
fn config_default_scope_applies_when_session_has_no_saved_scope() {
    let config = spacetop_core::config::SpacetopConfig {
        defaults: spacetop_core::config::DefaultsConfig {
            scope: spacetop_core::config::DefaultScope::Archived,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut state = overview_state_with_active_and_archived_items();
    state.apply_config_defaults(&config);
    assert_eq!(state.view_scope(), ViewScope::Archived);
}

#[test]
fn session_scope_overrides_config_default_scope() {
    let config = spacetop_core::config::SpacetopConfig {
        defaults: spacetop_core::config::DefaultsConfig {
            scope: spacetop_core::config::DefaultScope::Archived,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut state = overview_state_with_active_and_archived_items();
    state.apply_config_defaults(&config);
    state.apply_session(&spacetop_core::session_state::WorkflowSession {
        selected_entity_id: None,
        scope: spacetop_core::session_state::WorkflowScope::Active,
    });
    assert_eq!(state.view_scope(), ViewScope::Active);
}
```

- [ ] **Step 2: Run and verify it fails**

Run: `cargo test -p spacetop app::tests::app_stores_config_for_key_handling`

Expected: FAIL because app config is not stored.

- [ ] **Step 3: Store config on `App`**

Add a `config: SpacetopConfig` field to `App`. Keep existing constructors by delegating to `Default::default()`:

```rust
pub fn new_with_config(workflow_dir: impl Into<PathBuf>, config: SpacetopConfig) -> Self
```

and:

```rust
pub fn config(&self) -> &SpacetopConfig {
    &self.config
}
```

Add a `config_warnings: Vec<ConfigWarning>` field and a read-only accessor so footer/status rendering can expose malformed-config warnings without aborting the app.

- [ ] **Step 4: Load config in `run` without discarding warnings**

In `crates/spacetop/src/lib.rs`, before deciding/running the app:

```rust
let config_load = spacetop_core::config::load_config_with_warnings(
    &spacetop_core::config::StdEnv,
)?;
```

Thread `config_load.config` and `config_load.warnings` into app constructors. If parse fails, surface a footer/status warning rather than aborting; do not block read-only inspection due to a bad config file. Apply defaults before session restore. Precedence is:

1. persisted session state
2. config defaults
3. built-in behavior

So config defaults are used only when there is no saved per-workflow session value.

- [ ] **Step 5: Verify and commit**

Run: `cargo test -p spacetop app::tests app::keys::tests`

Expected: PASS.

```bash
git add crates/spacetop/src/lib.rs crates/spacetop/src/app.rs crates/spacetop/src/app/keys.rs
git commit -m "feat(tui): load and carry user config"
```

---

## Task 5: Apply theme colors at the UI boundary

**Files:**
- Modify: `crates/spacetop/src/ui/color.rs`
- Modify: `crates/spacetop/src/ui/footer.rs`
- Modify: `crates/spacetop/src/ui/list.rs`

- [ ] **Step 1: Add color parsing tests**

In `ui/color.rs`, add:

```rust
#[test]
fn parses_hex_rgb_color() {
    assert_eq!(
        color_from_hex("#283454"),
        Some(ratatui::style::Color::Rgb(40, 52, 84))
    );
}

#[test]
fn invalid_hex_color_returns_none() {
    assert_eq!(color_from_hex("blue"), None);
    assert_eq!(color_from_hex("#12"), None);
}
```

- [ ] **Step 2: Run and verify it fails**

Run: `cargo test -p spacetop ui::color::tests`

Expected: FAIL because `color_from_hex` does not exist.

- [ ] **Step 3: Implement hex parsing**

Add:

```rust
pub(crate) fn color_from_hex(value: &str) -> Option<ratatui::style::Color> {
    let hex = value.strip_prefix('#')?;
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(ratatui::style::Color::Rgb(r, g, b))
}
```

- [ ] **Step 4: Use config colors with defaults**

Replace hardcoded selection/footer background constants with helper functions that accept `&SpacetopConfig`, parse the configured hex, and fall back to existing constants when invalid.

- [ ] **Step 5: Verify and commit**

Run: `cargo test -p spacetop ui::tests::colors ui::tests::task_list`

Expected: PASS.

```bash
git add crates/spacetop/src/ui/color.rs crates/spacetop/src/ui/footer.rs crates/spacetop/src/ui/list.rs
git commit -m "feat(tui): apply configured theme colors"
```

---

## Task 6: Apply configurable keybindings

**Files:**
- Modify: `crates/spacetop/src/app/keys.rs`
- Modify: `crates/spacetop/src/ui/help.rs`
- Modify: `crates/spacetop/src/ui/footer.rs`

- [ ] **Step 1: Add keybinding resolution tests**

In `app/keys.rs`, add:

```rust
#[test]
fn configured_search_key_opens_search() {
    let config = spacetop_core::config::SpacetopConfig {
        keybindings: spacetop_core::config::KeybindingConfig {
            search: "f".to_string(),
            ..Default::default()
        },
        ..Default::default()
    };
    let mut session = single_session_with_item(PathBuf::from("/tmp/item.md"));
    let action = handle_overview_key_with_config(
        &mut session,
        key(KeyCode::Char('f')),
        &config,
    );
    assert!(matches!(action, OverviewKeyAction::OpenSearch));
}

#[test]
fn duplicate_configured_keys_fall_back_to_defaults() {
    let config = spacetop_core::config::SpacetopConfig {
        keybindings: spacetop_core::config::KeybindingConfig {
            search: "a".to_string(),
            activity: "a".to_string(),
            ..Default::default()
        },
        ..Default::default()
    };
    let resolved = ResolvedKeymap::from_config(&config);
    assert_eq!(resolved.search.label(), "/");
    assert!(resolved.warnings().iter().any(|warning| warning.contains("duplicate")));
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
    assert!(resolved.warnings().iter().any(|warning| warning.contains("reserved")));
}
```

- [ ] **Step 2: Run and verify it fails**

Run: `cargo test -p spacetop app::keys::tests::configured_search_key_opens_search app::keys::tests::duplicate_configured_keys_fall_back_to_defaults app::keys::tests::reserved_overview_keys_fall_back_to_defaults`

Expected: FAIL because key handling does not accept a resolved keymap.

- [ ] **Step 3: Add resolved keymap handling**

Introduce:

```rust
#[derive(Debug, Clone)]
pub(crate) struct ResolvedKeymap {
    pub search: ResolvedKey,
    pub command: ResolvedKey,
    pub timeline: ResolvedKey,
    pub metrics: ResolvedKey,
    pub activity: ResolvedKey,
    pub relations: ResolvedKey,
    warnings: Vec<String>,
}

impl ResolvedKeymap {
    pub(crate) fn from_config(config: &SpacetopConfig) -> Self;
    pub(crate) fn warnings(&self) -> &[String];
}
```

Resolve only single printable-character bindings. If a configured key string is empty, longer than one char, duplicates another P3 binding, or collides with reserved overview keys (`a`, `s`, `D`, `Y`, `Enter`, `Esc`, arrows, preview scrolling), keep the default for that action and record a warning.

Change `handle_overview_key` to delegate:

```rust
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
) -> OverviewKeyAction
```

In `App`, store a precomputed `ResolvedKeymap` alongside `SpacetopConfig` at construction time. `App::handle_key` passes `&self.resolved_keymap`, avoiding a mutable-borrow plus `self.config()` borrow conflict while handling input.

- [ ] **Step 4: Update help/footer labels from config**

Render labels from `ResolvedKeymap` for search/command/P3 view keys. Keep stable defaults when config is invalid, and expose keymap warnings in the same status-warning surface used for config parse warnings.

- [ ] **Step 5: Verify and commit**

Run: `cargo test -p spacetop app::keys::tests ui::tests::footer ui::tests::help`

Expected: PASS.

```bash
git add crates/spacetop/src/app/keys.rs crates/spacetop/src/ui/footer.rs crates/spacetop/src/ui/help.rs
git commit -m "feat(tui): honor configured keybindings"
```

---

## Task 7: Persist selected entity and scope

**Files:**
- Modify: `crates/spacetop/src/lib.rs`
- Modify: `crates/spacetop/src/app.rs`
- Modify: `crates/spacetop/src/app/overview.rs`

- [ ] **Step 1: Add app session restore test**

Add:

```rust
#[test]
fn overview_applies_saved_selected_entity() {
    let root = PathBuf::from("/tmp/spacetop-session-test");
    let snapshot = snapshot_with_items(vec![
        item_at(root.join("001-first.md"), "001", "first", "plan"),
        item_at(root.join("002-second.md"), "002", "second", "plan"),
    ]);
    let mut state = OverviewState::from_snapshot(root, snapshot);
    state.apply_session(&spacetop_core::session_state::WorkflowSession {
        selected_entity_id: Some("002".to_string()),
        scope: spacetop_core::session_state::WorkflowScope::Active,
    });
    assert_eq!(state.selected_item().expect("selected").id, "002");
}
```

Add a key-stability test in `session_state.rs`:

```rust
#[test]
fn workflow_session_key_uses_canonical_absolute_path() {
    let key = WorkflowSessionKey::from_workflow_dir(Path::new("/repo/docs/workflow"))
        .expect("session key");
    assert_eq!(key.as_str(), "/repo/docs/workflow");
}
```

- [ ] **Step 2: Run and verify it fails**

Run: `cargo test -p spacetop app::tests::overview_applies_saved_selected_entity`

Expected: FAIL because `apply_session` does not exist.

- [ ] **Step 3: Implement session apply/export on overview**

Add:

```rust
pub fn apply_session(&mut self, saved: &WorkflowSession) {
    match saved.scope {
        WorkflowScope::Active => self.view_scope = ViewScope::Active,
        WorkflowScope::Archived => self.view_scope = ViewScope::Archived,
    }
    if let Some(id) = &saved.selected_entity_id {
        let visible = self.visible_items();
        if let Some(pos) = visible.iter().position(|entity| &entity.id == id) {
            self.set_scope_index(pos);
        }
    }
}

pub fn to_workflow_session(&self) -> WorkflowSession {
    WorkflowSession {
        selected_entity_id: self.selected_item().map(|entity| entity.id.clone()),
        scope: match self.view_scope {
            ViewScope::Active => WorkflowScope::Active,
            ViewScope::Archived => WorkflowScope::Archived,
        },
    }
}
```

- [ ] **Step 4: Load session before TUI and save on exit**

In `session_state.rs`, add `WorkflowSessionKey` as a small owned wrapper whose constructor canonicalizes the workflow directory and rejects relative paths that cannot be canonicalized. Store `SessionState.workflows` by this key's string value, not by raw user input. In `run_terminal`, load session state from `state_path(&StdEnv)` before entering the loop, apply it to active workflow states after config defaults, and save it after the loop exits. Swallow save errors by setting a status warning; do not fail the app on session persistence errors.

- [ ] **Step 5: Verify and commit**

Run:

```bash
cargo test -p spacetop app::tests
cargo test -p spacetop-core session_state::tests
```

Expected: PASS.

```bash
git add crates/spacetop/src/lib.rs crates/spacetop/src/app.rs crates/spacetop/src/app/overview.rs
git commit -m "feat(tui): persist per-workflow session state"
```

---

## Task 8: Docs and full verification

**Files:**
- Modify: `README.md`
- Modify: `AGENTS.md`
- Modify: `docs/development-policy.md`

- [ ] **Step 1: Document config paths**

In `README.md`, add:

```markdown
## Configuration

Spacetop reads YAML config from `$XDG_CONFIG_HOME/spacetop/config.yaml`, falling back to `~/.config/spacetop/config.yaml`. It stores session state under `$XDG_STATE_HOME/spacetop/session.yaml`, falling back to `~/.local/state/spacetop/session.yaml`. It does not write config or session files into workflow directories.
```

- [ ] **Step 2: Document safety boundary**

In `AGENTS.md` and `docs/development-policy.md`, add that config/session writes are permitted only under user config/state paths and are not workflow markdown writes.

- [ ] **Step 3: Full verification**

Run:

```bash
cargo test --workspace
make lint
cargo test -p spacetop-core --test no_write_git_calls
```

Expected: all PASS.

- [ ] **Step 4: Commit**

```bash
git add README.md AGENTS.md docs/development-policy.md
git commit -m "docs: document config and session persistence paths"
```

## Definition of done (P4)

- [ ] Config loads from XDG-style YAML path with defaults.
- [ ] Session state loads/saves under XDG-style state path.
- [ ] Relative XDG/HOME path values are ignored so config/session writes cannot land in the repository by accident.
- [ ] Malformed config falls back to defaults with a user-visible warning.
- [ ] No config/session writes target workflow directories.
- [ ] Theme colors are configurable with safe fallback.
- [ ] P3 view keybindings are configurable through `ResolvedKeymap` with duplicate/reserved-key warnings.
- [ ] Config defaults apply before saved session state, and saved session state wins when both exist.
- [ ] Selected entity and scope persist per workflow.
- [ ] `cargo test --workspace` passes.
- [ ] `make lint` passes.
