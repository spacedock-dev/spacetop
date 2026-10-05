use anyhow::{bail, Context, Result};
use serde::{de::DeserializeOwned, Deserialize};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    process::{Command, Output},
};

#[derive(Deserialize)]
pub(super) struct Pane {
    pub pane_id: String,
    pub workspace_id: String,
    pub tab_id: String,
    pub cwd: Option<PathBuf>,
    pub focused: bool,
    #[serde(default)]
    pub tokens: BTreeMap<String, String>,
}

#[derive(Deserialize)]
pub(super) struct OwnedPane {
    pub plugin_id: String,
    pub entrypoint: String,
    pub pane: Pane,
}

#[derive(Deserialize)]
pub(super) struct PaneList {
    pub panes: Vec<Pane>,
}
#[derive(Deserialize)]
pub(super) struct PluginPane {
    pub plugin_pane: OwnedPane,
}

pub(super) struct Client {
    binary: PathBuf,
}
impl Client {
    pub fn new(binary: PathBuf) -> Self {
        Self { binary }
    }
    fn execute(&self, args: &[&str]) -> Result<Output> {
        let output = Command::new(&self.binary)
            .args(args)
            .output()
            .context("cannot run Herdr; check HERDR_BIN_PATH and installation")?;
        if !output.status.success() {
            bail!(
                "Herdr {} failed: {}; check session/socket, API version and plugin registration",
                args.join(" "),
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        Ok(output)
    }
    pub fn call<T: DeserializeOwned>(&self, args: &[&str]) -> Result<T> {
        let output = self.execute(args)?;
        #[derive(Deserialize)]
        struct Envelope<T> {
            result: T,
        }
        let response: Envelope<T> = serde_json::from_slice(&output.stdout)
            .context("malformed Herdr API response; check supported Herdr version and session")?;
        Ok(response.result)
    }
    pub fn panes(&self, workspace: &str) -> Result<Vec<Pane>> {
        Ok(self
            .call::<PaneList>(&["pane", "list", "--workspace", workspace])?
            .panes)
    }
    pub fn focus(&self, pane: &str) -> Result<OwnedPane> {
        Ok(self
            .call::<PluginPane>(&["plugin", "pane", "focus", pane])?
            .plugin_pane)
    }
    pub fn open(&self, target: &str, project: &Path, binary: &Path) -> Result<OwnedPane> {
        let cwd = project.to_str().context("project path must be UTF-8")?;
        let executable = format!("SPACETOP_SIDECAR_BIN={}", binary.display());
        let root = format!("SPACETOP_SIDECAR_PROJECT={cwd}");
        Ok(self
            .call::<PluginPane>(&[
                "plugin",
                "pane",
                "open",
                "--plugin",
                "spacetop.sidecar",
                "--entrypoint",
                "inspector",
                "--target-pane",
                target,
                "--placement",
                "split",
                "--direction",
                "right",
                "--cwd",
                cwd,
                "--no-focus",
                "--env",
                &executable,
                "--env",
                &root,
            ])?
            .plugin_pane)
    }
    pub fn tag(&self, pane: &str) -> Result<()> {
        self.execute(&[
            "pane",
            "report-metadata",
            pane,
            "--source",
            "spacetop.sidecar",
            "--token",
            "spacetop_sidecar=v1",
        ])?;
        Ok(())
    }
}
