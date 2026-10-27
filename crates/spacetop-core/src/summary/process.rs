use super::SummaryError;
use rustix::fs::{fcntl_getfl, fcntl_setfl, OFlags};
use rustix::process::{kill_process_group, waitid, Pid, Signal, WaitId, WaitIdOptions};
use std::fs::{self, DirBuilder};
use std::io::{Read, Write};
use std::os::fd::AsFd;
use std::os::unix::{fs::DirBuilderExt, process::CommandExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy)]
pub struct Limits {
    pub timeout: Duration,
    pub stdout: usize,
    pub stderr: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(45),
            stdout: 65536,
            stderr: 16384,
        }
    }
}

pub struct ProcessOutput {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub success: bool,
}

/// App-owned scratch is always under /tmp, never an inherited project TMPDIR.
pub struct Scratch(PathBuf);
impl Scratch {
    pub fn new() -> Result<Self, SummaryError> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| SummaryError::Process)?
            .as_nanos();
        let path = PathBuf::from("/tmp").join(format!(
            "spacetop-summary-{}-{stamp}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        DirBuilder::new()
            .mode(0o700)
            .create(&path)
            .map_err(|_| SummaryError::Process)?;
        Ok(Self(path))
    }
    pub fn path(&self) -> &Path {
        &self.0
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct Tracked {
    child: Child,
    reaped: bool,
}
impl Tracked {
    fn finish(&mut self) -> Result<ExitStatus, SummaryError> {
        if let Some(pid) = Pid::from_raw(self.child.id() as i32) {
            let _ = kill_process_group(pid, Signal::KILL);
        }
        let _ = self.child.kill();
        let status = self.child.wait().map_err(|_| SummaryError::Process)?;
        self.reaped = true;
        Ok(status)
    }
    fn exited(&self) -> Result<bool, SummaryError> {
        let pid = Pid::from_raw(self.child.id() as i32).ok_or(SummaryError::Process)?;
        // Keep the child unreaped until group cleanup so its PID cannot be reused.
        waitid(
            WaitId::Pid(pid),
            WaitIdOptions::EXITED | WaitIdOptions::NOHANG | WaitIdOptions::NOWAIT,
        )
        .map(|status| status.is_some())
        .map_err(|_| SummaryError::Process)
    }
}
impl Drop for Tracked {
    fn drop(&mut self) {
        if !self.reaped {
            let _ = self.finish();
        }
    }
}

fn nonblocking(fd: &impl AsFd) -> Result<(), SummaryError> {
    let flags = fcntl_getfl(fd).map_err(|_| SummaryError::Process)?;
    fcntl_setfl(fd, flags | OFlags::NONBLOCK).map_err(|_| SummaryError::Process)
}

fn drain(pipe: &mut impl Read, buffer: &mut Vec<u8>, limit: usize) -> Result<bool, SummaryError> {
    let mut chunk = [0; 4096];
    match pipe.read(&mut chunk) {
        Ok(0) => Ok(true),
        Ok(count) => {
            if buffer.len() + count > limit {
                return Err(SummaryError::OutputLimit);
            }
            buffer.extend_from_slice(&chunk[..count]);
            Ok(false)
        }
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
            ) =>
        {
            Ok(false)
        }
        Err(_) => Err(SummaryError::Process),
    }
}

/// All pipes are nonblocking in one bounded worker. No blocked I/O thread can
/// outlive cancellation; escaped descendants can survive but cannot hold us open.
pub fn run(
    mut command: Command,
    input: &[u8],
    cancel: &AtomicBool,
    limits: Limits,
) -> Result<ProcessOutput, SummaryError> {
    if input.len() > super::prompt::MAX_INPUT * 6 + 4096 {
        return Err(SummaryError::InputLimit);
    }
    if cancel.load(Ordering::Acquire) {
        return Err(SummaryError::Canceled);
    }
    let started = Instant::now();
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0);
    let child = command.spawn().map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            SummaryError::Unavailable
        } else {
            SummaryError::Process
        }
    })?;
    let mut tracked = Tracked {
        child,
        reaped: false,
    };
    let mut stdin = tracked.child.stdin.take().ok_or(SummaryError::Process)?;
    let mut stdout = tracked.child.stdout.take().ok_or(SummaryError::Process)?;
    let mut stderr = tracked.child.stderr.take().ok_or(SummaryError::Process)?;
    nonblocking(&stdin)?;
    nonblocking(&stdout)?;
    nonblocking(&stderr)?;
    let mut input_pipe = Some(&mut stdin);
    let mut written = 0;
    let mut output = ProcessOutput {
        stdout: Vec::new(),
        stderr: Vec::new(),
        success: false,
    };
    let (mut out_done, mut err_done) = (false, false);
    let mut exit = None;
    loop {
        if cancel.load(Ordering::Acquire) {
            return Err(SummaryError::Canceled);
        }
        if started.elapsed() >= limits.timeout {
            return Err(SummaryError::Timeout);
        }
        if let Some(pipe) = input_pipe.as_mut() {
            match pipe.write(&input[written..input.len().min(written + 4096)]) {
                Ok(count) => written += count,
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                    ) => {}
                Err(_) => input_pipe = None,
            }
            if written == input.len() {
                input_pipe = None;
            }
        }
        // Close the owned descriptor as soon as all input has been sent.
        if input_pipe.is_none() {
            break;
        }
        if !out_done {
            out_done = drain(&mut stdout, &mut output.stdout, limits.stdout)?;
        }
        if !err_done {
            err_done = drain(&mut stderr, &mut output.stderr, limits.stderr)?;
        }
        if tracked.exited()? {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    drop(stdin);
    loop {
        if cancel.load(Ordering::Acquire) {
            return Err(SummaryError::Canceled);
        }
        if started.elapsed() >= limits.timeout {
            return Err(SummaryError::Timeout);
        }
        if !out_done {
            out_done = drain(&mut stdout, &mut output.stdout, limits.stdout)?;
        }
        if !err_done {
            err_done = drain(&mut stderr, &mut output.stderr, limits.stderr)?;
        }
        if exit.is_none() && tracked.exited()? {
            exit = Some(tracked.finish()?);
        }
        if out_done && err_done {
            if let Some(status) = exit {
                output.success = status.success();
                return Ok(output);
            }
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}
