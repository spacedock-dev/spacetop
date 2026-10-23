use super::SummaryProvider;
use std::process::Command;

/// Native controls verified against Claude 2.1.309 / Codex 0.164.0.
/// User auth/provider configuration remains CLI-owned. Unsupported controls fail;
/// no fallback removes controls or changes provider/model.
pub fn command(provider: SummaryProvider) -> Command {
    let mut command = Command::new(provider.executable());
    command.args(arguments(provider));
    command
}

pub fn arguments(provider: SummaryProvider) -> Vec<String> {
    let args = match provider {
        SummaryProvider::Claude => vec![
            "-p",
            "--safe-mode",
            "--tools",
            "",
            "--disable-slash-commands",
            "--strict-mcp-config",
            "--mcp-config",
            "{\"mcpServers\":{}}",
            "--no-session-persistence",
            "--output-format",
            "json",
            "--model",
            provider.model(),
        ],
        SummaryProvider::Codex => vec![
            "exec",
            "--json",
            "--ephemeral",
            "--ignore-rules",
            "--skip-git-repo-check",
            "--sandbox",
            "read-only",
            "--color",
            "never",
            "--model",
            provider.model(),
            "--disable",
            "hooks",
            "--disable",
            "plugins",
            "--disable",
            "apps",
            "--disable",
            "shell_tool",
            "--disable",
            "multi_agent",
            "--disable",
            "view_image",
            "-c",
            "project_doc_max_bytes=0",
            "-c",
            "web_search=\"disabled\"",
            "-c",
            "mcp_servers={}",
            "-",
        ],
    };
    args.into_iter().map(str::to_owned).collect()
}
