//! Editor resolution and (blocking) invocation.
//!
//! Spacetop's "open file in $EDITOR" feature suspends the TUI, blocks on the
//! editor process, and resumes the TUI on return. The blocking model matches
//! lazygit/gitui conventions — it sidesteps zombie reaping, keeps resume
//! sequencing trivial, and works over SSH and inside tmux without fighting
//! the parent terminal for stdio.
//!
//! Two seams keep this testable without an editor installed:
//!
//! * [`EditorEnv`] — environment lookup for `$VISUAL` / `$EDITOR`. The
//!   production [`StdEnv`] delegates to [`std::env::var_os`].
//! * [`EditorLauncher`] — process spawn + wait. The production
//!   [`StdLauncher`] runs `Command::new(...).args(...).arg(file).status()`.
//!
//! [`resolve_editor`] is a pure function over [`EditorEnv`]: it returns an
//! [`EditorCommand`] with precedence `$VISUAL` → `$EDITOR` → platform default
//! (`open` on macOS, `xdg-open` elsewhere). When `$VISUAL` / `$EDITOR` carry
//! arguments (e.g. `"code --wait"`), the value is split on ASCII whitespace
//! into program + args.

use std::ffi::{OsStr, OsString};
use std::io;
use std::path::Path;
use std::process::{Command, ExitStatus};

/// A resolved editor invocation: the program to run and the args to pass
/// before the target file path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorCommand {
    pub program: OsString,
    pub args: Vec<OsString>,
}

/// Environment lookup seam. Allows the resolver to be unit-tested without
/// touching the process environment.
pub trait EditorEnv {
    fn var(&self, key: &str) -> Option<OsString>;
}

/// Production [`EditorEnv`] backed by [`std::env::var_os`].
pub struct StdEnv;

impl EditorEnv for StdEnv {
    fn var(&self, key: &str) -> Option<OsString> {
        std::env::var_os(key)
    }
}

/// Process-spawn seam. Allows the event loop to call into a stub launcher
/// in tests that don't have a real editor available.
pub trait EditorLauncher {
    /// Launch the resolved editor against `file` and block until it exits.
    fn launch(&self, cmd: &EditorCommand, file: &Path) -> io::Result<ExitStatus>;
}

/// Production [`EditorLauncher`] that calls `Command::status` — blocking.
pub struct StdLauncher;

impl EditorLauncher for StdLauncher {
    fn launch(&self, cmd: &EditorCommand, file: &Path) -> io::Result<ExitStatus> {
        Command::new(&cmd.program)
            .args(&cmd.args)
            .arg(file)
            .status()
    }
}

/// Platform default opener. Returns the command Spacetop falls back to when
/// neither `$VISUAL` nor `$EDITOR` is set.
fn platform_default() -> EditorCommand {
    let program = if cfg!(target_os = "macos") {
        OsString::from("open")
    } else {
        OsString::from("xdg-open")
    };
    EditorCommand {
        program,
        args: Vec::new(),
    }
}

/// Split an `EDITOR`/`VISUAL` value on ASCII whitespace into program + args.
/// Returns `None` when the value is empty or whitespace-only so the caller
/// can fall through to the next precedence level.
fn split_command(value: &OsStr) -> Option<EditorCommand> {
    // Many editor values like `"code --wait"` are pure ASCII; falling back to
    // a lossy `to_string_lossy().split_whitespace()` is acceptable here and
    // matches what `sh`-style shells do for the same variable. Non-UTF-8
    // bytes survive lossy conversion as U+FFFD but `var_os` rarely produces
    // them on real systems.
    let s = value.to_string_lossy();
    let mut parts = s.split_whitespace();
    let program = parts.next()?;
    let args: Vec<OsString> = parts.map(OsString::from).collect();
    Some(EditorCommand {
        program: OsString::from(program),
        args,
    })
}

/// Resolve which editor command to run, in precedence order
/// `$VISUAL` → `$EDITOR` → platform default.
pub fn resolve_editor(env: &dyn EditorEnv) -> EditorCommand {
    if let Some(value) = env.var("VISUAL") {
        if let Some(cmd) = split_command(&value) {
            return cmd;
        }
    }
    if let Some(value) = env.var("EDITOR") {
        if let Some(cmd) = split_command(&value) {
            return cmd;
        }
    }
    platform_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// Stub [`EditorEnv`] backed by an in-memory map.
    struct MapEnv {
        vars: HashMap<&'static str, OsString>,
    }

    impl MapEnv {
        fn empty() -> Self {
            Self {
                vars: HashMap::new(),
            }
        }
        fn with(mut self, key: &'static str, value: &str) -> Self {
            self.vars.insert(key, OsString::from(value));
            self
        }
    }

    impl EditorEnv for MapEnv {
        fn var(&self, key: &str) -> Option<OsString> {
            self.vars.get(key).cloned()
        }
    }

    /// AC-4: editor resolution falls back deterministically across all four
    /// supported precedence cases.
    #[test]
    fn resolve_editor_visual_editor_default_precedence() {
        // (a) VISUAL set — wins over EDITOR.
        let env = MapEnv::empty().with("VISUAL", "vim").with("EDITOR", "nano");
        let cmd = resolve_editor(&env);
        assert_eq!(cmd.program, OsString::from("vim"));
        assert!(cmd.args.is_empty());

        // (b) EDITOR set, VISUAL unset.
        let env = MapEnv::empty().with("EDITOR", "nano");
        let cmd = resolve_editor(&env);
        assert_eq!(cmd.program, OsString::from("nano"));
        assert!(cmd.args.is_empty());

        // (c) Both unset — platform default.
        let env = MapEnv::empty();
        let cmd = resolve_editor(&env);
        let expected_program = if cfg!(target_os = "macos") {
            OsString::from("open")
        } else {
            OsString::from("xdg-open")
        };
        assert_eq!(cmd.program, expected_program);
        assert!(cmd.args.is_empty());

        // (d) EDITOR with args — split on whitespace.
        let env = MapEnv::empty().with("EDITOR", "code --wait");
        let cmd = resolve_editor(&env);
        assert_eq!(cmd.program, OsString::from("code"));
        assert_eq!(cmd.args, vec![OsString::from("--wait")]);
    }

    #[test]
    fn empty_visual_falls_through_to_editor() {
        // VISUAL=" " is whitespace-only; resolver should treat it as unset
        // and fall through to EDITOR rather than spawning a blank program.
        let env = MapEnv::empty().with("VISUAL", "   ").with("EDITOR", "nano");
        let cmd = resolve_editor(&env);
        assert_eq!(cmd.program, OsString::from("nano"));
    }
}
