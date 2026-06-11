use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SpacetopConfig {
    #[serde(default)]
    pub theme: ThemeConfig,
    #[serde(default)]
    pub keybindings: KeybindingConfig,
    #[serde(default)]
    pub defaults: DefaultsConfig,
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

fn key_search() -> String {
    "/".to_string()
}

fn key_command() -> String {
    ":".to_string()
}

fn key_timeline() -> String {
    "T".to_string()
}

fn key_metrics() -> String {
    "M".to_string()
}

fn key_activity() -> String {
    "A".to_string()
}

fn key_relations() -> String {
    "R".to_string()
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DefaultSort {
    #[default]
    Id,
    Status,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DefaultScope {
    #[default]
    Active,
    Archived,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigLoad {
    pub config: SpacetopConfig,
    pub warnings: Vec<ConfigWarning>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigWarning {
    pub message: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("failed to read config: {0}")]
    Io(std::io::Error),
    #[error("failed to parse config: {0}")]
    Parse(serde_yaml::Error),
}

pub fn config_path(env: &impl ConfigEnv) -> Option<PathBuf> {
    if let Some(path) = absolute_env_path(env, "XDG_CONFIG_HOME") {
        return Some(path.join("spacetop").join("config.yaml"));
    }
    absolute_env_path(env, "HOME")
        .map(|home| home.join(".config").join("spacetop").join("config.yaml"))
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
        None => Ok(default_load()),
    }
}

pub fn load_config_file(path: &Path) -> Result<SpacetopConfig, ConfigError> {
    match std::fs::read_to_string(path) {
        Ok(body) => serde_yaml::from_str(&body).map_err(ConfigError::Parse),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(SpacetopConfig::default()),
        Err(err) => Err(ConfigError::Io(err)),
    }
}

pub fn load_config_file_with_warnings(path: &Path) -> Result<ConfigLoad, ConfigError> {
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
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(default_load()),
        Err(err) => Err(ConfigError::Io(err)),
    }
}

fn absolute_env_path(env: &impl ConfigEnv, key: &str) -> Option<PathBuf> {
    let value = env.var(key)?;
    if value.is_empty() {
        return None;
    }
    let path = PathBuf::from(value);
    path.is_absolute().then_some(path)
}

fn default_load() -> ConfigLoad {
    ConfigLoad {
        config: SpacetopConfig::default(),
        warnings: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
    fn default_config_preserves_existing_behavior() {
        let config = SpacetopConfig::default();
        assert_eq!(config.defaults.sort, DefaultSort::Id);
        assert_eq!(config.defaults.scope, DefaultScope::Active);
        assert_eq!(config.keybindings.search, "/");
        assert_eq!(config.keybindings.command, ":");
        assert_eq!(config.keybindings.timeline, "T");
        assert_eq!(config.keybindings.metrics, "M");
        assert_eq!(config.keybindings.activity, "A");
        assert_eq!(config.keybindings.relations, "R");
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

    #[test]
    fn empty_xdg_config_home_is_ignored() {
        let env = TestEnv {
            vars: HashMap::from([
                ("XDG_CONFIG_HOME".to_string(), String::new()),
                ("HOME".to_string(), "/home/kent".to_string()),
            ]),
        };
        assert_eq!(
            config_path(&env),
            Some(PathBuf::from("/home/kent/.config/spacetop/config.yaml"))
        );
    }

    #[test]
    fn relative_home_yields_no_config_path() {
        let env = TestEnv {
            vars: HashMap::from([("HOME".to_string(), "relative/home".to_string())]),
        };
        assert_eq!(config_path(&env), None);
    }

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
}
