//! Explicit, bounded execution of a trusted installed summarizer CLI.
//! This is not an OS isolation or complete descendant-cleanup boundary.
pub mod adapters;
pub mod output;
pub mod process;
pub mod prompt;
pub mod snapshot;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::atomic::AtomicBool;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SummaryProvider {
    #[default]
    Claude,
    Codex,
}

impl SummaryProvider {
    pub fn executable(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
        }
    }
    pub fn model(self) -> &'static str {
        match self {
            Self::Claude => "claude-haiku-5-5",
            Self::Codex => "gpt-6-luna",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SummaryError {
    #[error("Task input exceeds 64 KiB. Original remains available.")]
    InputLimit,
    #[error("Task changed or could not be captured. Reload Original, then request again.")]
    Changed,
    #[error("Selected CLI is unavailable. Install it or select your other existing CLI in summary.provider.")]
    Unavailable,
    #[error("CLI could not complete with its existing login and requested model. Check that CLI outside Spacetop; no fallback was attempted.")]
    Provider,
    #[error("CLI returned malformed or empty summary output.")]
    Output,
    #[error("CLI output exceeded the capture limit.")]
    OutputLimit,
    #[error("Summary timed out after the configured deadline.")]
    Timeout,
    #[error("Summary canceled.")]
    Canceled,
    #[error("Could not prepare or supervise the summary process.")]
    Process,
}

pub fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub fn generate(
    provider: SummaryProvider,
    input: &[u8],
    cancel: &AtomicBool,
) -> Result<String, SummaryError> {
    let prompt = prompt::build(input)?;
    let scratch = process::Scratch::new()?;
    let mut command = adapters::command(provider);
    command.current_dir(scratch.path());
    let output = process::run(
        command,
        prompt.as_bytes(),
        cancel,
        process::Limits::default(),
    )?;
    if !output.success {
        return Err(SummaryError::Provider);
    }
    output::parse(provider, &output.stdout)
}
