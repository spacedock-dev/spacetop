use std::{env, path::PathBuf, process::Command};

use anyhow::{bail, Context, Result};
use serde::Deserialize;

#[derive(Deserialize)]
pub(super) struct Invocation {
    pub workspace_id: String,
    pub tab_id: String,
    pub focused_pane_id: String,
    pub focused_pane_cwd: PathBuf,
}

pub(super) fn absolute_env(name: &str) -> Result<PathBuf> {
    let path =
        PathBuf::from(env::var_os(name).with_context(|| {
            format!("{name} missing; invoke the registered action inside Herdr")
        })?);
    if !path.is_absolute() {
        bail!("{name} must be an absolute path");
    }
    Ok(path)
}

impl Invocation {
    pub fn read() -> Result<Self> {
        let raw = env::var("HERDR_PLUGIN_CONTEXT_JSON")
            .context("Herdr invocation context missing; use the registered plugin shortcut")?;
        let context: Self = serde_json::from_str(&raw).context(
            "invalid Herdr invocation context; check Herdr version and invoke from an agent pane",
        )?;
        for id in [
            &context.workspace_id,
            &context.tab_id,
            &context.focused_pane_id,
        ] {
            if id.is_empty()
                || !id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b':' || b == b'_')
            {
                bail!("invalid Herdr workspace/tab/pane ID; invoke from an available agent pane");
            }
        }
        if !context.focused_pane_cwd.is_absolute() || !context.focused_pane_cwd.is_dir() {
            bail!("focused pane cwd must be an existing absolute directory; open an agent in a Git repository");
        }
        Ok(context)
    }

    pub fn project(&self) -> Result<PathBuf> {
        let output = Command::new("git")
            .args(["rev-parse", "--show-toplevel"])
            .current_dir(&self.focused_pane_cwd)
            .output()
            .context("cannot run Git; install Git and retry")?;
        if !output.status.success() {
            bail!("focused pane is not in an accessible Git repository; open the agent in the project first");
        }
        let root = String::from_utf8(output.stdout).context("Git project path is not UTF-8")?;
        let root = PathBuf::from(root.trim_end_matches(['\r', '\n']));
        if !root.is_absolute() || !root.is_dir() {
            bail!("Git returned an unusable project directory");
        }
        root.canonicalize()
            .context("cannot resolve invoking project")
    }
}
