use super::{
    client::{Client, OwnedPane},
    context::{absolute_env, Invocation},
};
use anyhow::{bail, Context, Result};
use std::{
    env,
    fs::{self, OpenOptions},
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    process::Command,
    thread,
    time::{Duration, Instant},
};

fn executable(path: &Path) -> bool {
    let Ok(metadata) = path.metadata() else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

fn spacetop_binary() -> Result<PathBuf> {
    if env::var_os("SPACETOP_BIN").is_some() {
        let path = absolute_env("SPACETOP_BIN")?;
        if executable(&path) {
            return path.canonicalize().context("cannot resolve SPACETOP_BIN");
        }
        bail!(
            "SPACETOP_BIN is not executable; install Spacetop or set an absolute executable path"
        );
    }
    let helper = env::current_exe().context("cannot locate sidecar helper")?;
    if let Some(parent) = helper.parent() {
        let sibling = parent.join("spacetop");
        if executable(&sibling) {
            return Ok(sibling);
        }
    }
    if let Some(path) = env::var_os("PATH") {
        for directory in env::split_paths(&path).filter(|p| p.is_absolute()) {
            let binary = directory.join("spacetop");
            if executable(&binary) {
                return binary
                    .canonicalize()
                    .context("cannot resolve Spacetop binary");
            }
        }
    }
    bail!("Spacetop executable missing; run make install, put both binaries on PATH, or set SPACETOP_BIN")
}

struct ActionLock(PathBuf);
impl ActionLock {
    fn acquire(workspace: &str, project: &Path) -> Result<Self> {
        let state = absolute_env("HERDR_PLUGIN_STATE_DIR")?;
        if state.starts_with(project) {
            bail!("plugin state directory must be outside the project; use Herdr's absolute user state directory");
        }
        let ancestor = state
            .ancestors()
            .find(|p| p.exists())
            .context("plugin state directory has no accessible parent")?;
        if ancestor
            .canonicalize()
            .context("cannot resolve plugin state parent")?
            .starts_with(project)
        {
            bail!("plugin state directory resolves inside the project; use Herdr's absolute user state directory");
        }
        fs::create_dir_all(&state).context("cannot create Herdr plugin state directory")?;
        let state = state
            .canonicalize()
            .context("cannot resolve Herdr plugin state directory")?;
        if state.starts_with(project) {
            bail!("plugin state directory must be outside the project; use Herdr's absolute user state directory");
        }
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        env::var_os("HERDR_SOCKET_PATH").hash(&mut hash);
        workspace.hash(&mut hash);
        let path = state.join(format!("open-{:016x}.lock", hash.finish()));
        let started = Instant::now();
        loop {
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(_) => return Ok(Self(path)),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists && started.elapsed() < Duration::from_secs(3) => thread::sleep(Duration::from_millis(50)),
                Err(error) => bail!("cannot lock sidecar action at {}: {error}; retry, or remove a stale lock only after confirming no open action runs", path.display()),
            }
        }
    }
}
impl Drop for ActionLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn verify_owned(owned: &OwnedPane, workspace: &str, project: &Path) -> Result<()> {
    if owned.plugin_id != "spacetop.sidecar"
        || owned.entrypoint != "inspector"
        || owned.pane.workspace_id != workspace
        || owned.pane.cwd.as_deref() != Some(project)
    {
        bail!("Herdr returned a mismatched sidecar owner/workspace/project; close the stale pane and retry");
    }
    Ok(())
}

pub fn open() -> Result<()> {
    let invocation = Invocation::read()?;
    let project = invocation.project()?;
    let binary = spacetop_binary()?;
    let client = Client::new(absolute_env("HERDR_BIN_PATH")?);
    let _lock = ActionLock::acquire(&invocation.workspace_id, &project)?;
    let panes = client.panes(&invocation.workspace_id)?;
    if panes
        .iter()
        .any(|p| p.workspace_id != invocation.workspace_id)
    {
        bail!("Herdr pane query returned another workspace; check the session API");
    }
    let target = panes
        .iter()
        .find(|p| p.pane_id == invocation.focused_pane_id)
        .context("invoking pane is no longer available; focus the agent and retry")?;
    if target.tab_id != invocation.tab_id {
        bail!("invoking pane does not belong to the supplied tab; focus the agent and retry");
    }
    let candidates: Vec<_> = panes
        .iter()
        .filter(|p| p.tokens.get("spacetop_sidecar").is_some_and(|v| v == "v1"))
        .collect();
    match candidates.as_slice() {
        [] => {
            let owned = client.open(&invocation.focused_pane_id, &project, &binary)?;
            verify_owned(&owned, &invocation.workspace_id, &project)?;
            if owned.pane.pane_id == invocation.focused_pane_id
                || owned.pane.tab_id != invocation.tab_id
                || owned.pane.focused
            {
                bail!("Herdr did not open a new unfocused sidecar in the invoking tab; inspect the layout before retrying");
            }
            client.tag(&owned.pane.pane_id).context(
                "sidecar opened but tagging failed; close it before retrying to avoid duplicates",
            )?;
        }
        [existing] => {
            if existing.cwd.as_deref() != Some(project.as_path()) {
                bail!("this workspace already has a sidecar for another project; close it and invoke the shortcut from the intended agent");
            }
            let owned = client.focus(&existing.pane_id)?;
            verify_owned(&owned, &invocation.workspace_id, &project)?;
            if owned.pane.pane_id != existing.pane_id || !owned.pane.focused {
                bail!("Herdr did not focus the selected sidecar; inspect the pane before retrying");
            }
        }
        _ => bail!("multiple Spacetop sidecars in this workspace; close extras and retry"),
    }
    Ok(())
}

pub fn pane() -> Result<()> {
    let project = absolute_env("SPACETOP_SIDECAR_PROJECT")?
        .canonicalize()
        .context("sidecar project unavailable; close and reopen from the agent")?;
    if !project.is_dir() {
        bail!("sidecar project is not a directory; close and reopen");
    }
    let binary = absolute_env("SPACETOP_SIDECAR_BIN")?;
    if !executable(&binary) {
        bail!("Spacetop executable unavailable; reinstall and reopen the sidecar");
    }
    let status = Command::new(binary)
        .arg("--workflow-dir")
        .arg(&project)
        .current_dir(project)
        .status()
        .context("cannot start Spacetop; check executable and project access")?;
    if !status.success() {
        bail!(
            "Spacetop exited with {status}; check workflow discovery and plugin log, then reopen"
        );
    }
    Ok(())
}
