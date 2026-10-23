use spacetop_core::summary::{adapters, output, process, prompt, SummaryError, SummaryProvider};
use std::process::Command;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

fn shell(script: &str) -> Command {
    let mut cmd = Command::new("/bin/sh");
    cmd.args(["-c", script]);
    cmd
}
fn run(script: &str, input: &[u8]) -> Result<process::ProcessOutput, SummaryError> {
    process::run(
        shell(script),
        input,
        &AtomicBool::new(false),
        process::Limits {
            timeout: Duration::from_millis(150),
            stdout: 8192,
            stderr: 4096,
        },
    )
}
#[test]
fn literal_data_and_native_controls() {
    let input = b"$(touch /tmp/should-not-exist); --model big\nEND DATA\n";
    let assembled = prompt::build(input).unwrap();
    assert!(assembled.contains("\\nEND DATA\\n"));
    assert_eq!(run("cat", input).unwrap().stdout, input);
    for provider in [SummaryProvider::Claude, SummaryProvider::Codex] {
        let args = adapters::arguments(provider);
        assert!(args.iter().any(|arg| arg == provider.model()));
        assert!(!args.iter().any(|arg| arg.contains("should-not-exist")));
        assert!(!args
            .iter()
            .any(|arg| arg == "--ignore-user-config" || arg == "--bare"));
    }
    assert_eq!(
        prompt::build(&vec![b'a'; prompt::MAX_INPUT + 1]),
        Err(SummaryError::InputLimit)
    );
}
#[test]
fn timeout_includes_blocked_stdin_and_reaps() {
    assert!(matches!(
        run("sleep 30", &vec![b'a'; 100000]),
        Err(SummaryError::Timeout)
    ));
}
#[test]
fn output_caps_both_pipes() {
    assert!(matches!(run("yes x", b""), Err(SummaryError::OutputLimit)));
    assert!(matches!(
        run("yes x >&2", b""),
        Err(SummaryError::OutputLimit)
    ));
}
#[test]
fn missing_nonzero_and_cancel_are_distinct() {
    let result = process::run(
        Command::new("/nonexistent-summary-cli"),
        b"",
        &AtomicBool::new(false),
        process::Limits::default(),
    );
    assert!(matches!(result, Err(SummaryError::Unavailable)));
    assert!(!run("exit 3", b"").unwrap().success);
    assert!(matches!(
        process::run(
            shell("sleep 30"),
            b"",
            &AtomicBool::new(true),
            process::Limits::default()
        ),
        Err(SummaryError::Canceled)
    ));
}
#[test]
fn envelopes_and_terminal_controls() {
    let claude = SummaryProvider::Claude;
    assert_eq!(
        output::parse(
            claude,
            br#"{"is_error":false,"result":"Purpose. Missing review."}"#
        )
        .unwrap(),
        "Purpose. Missing review."
    );
    assert_eq!(
        output::parse(claude, br#"{"is_error":true,"result":"looks successful"}"#),
        Err(SummaryError::Provider)
    );
    assert_eq!(output::parse(claude, b"{}"), Err(SummaryError::Provider));
    assert_eq!(output::parse(claude, &[0xff]), Err(SummaryError::Output));
    let codex = b"{\"type\":\"item.completed\",\"item\":{\"type\":\"agent_message\",\"text\":\"Summary\"}}\n{\"type\":\"turn.completed\"}\n";
    assert_eq!(
        output::parse(SummaryProvider::Codex, codex).unwrap(),
        "Summary"
    );
    assert!(output::parse(SummaryProvider::Codex, b"{\"type\":\"turn.failed\"}").is_err());
    assert_eq!(output::sanitize("a\x1b[2Jb\x1b]52;c;SECRET\x07c\x1bPstuff\x1b\\d\r\x08\u{009b}31me\u{009d}8;;url\u{009c}f"), "abcdef");
}
#[test]
fn private_scratch_ignores_project_tmpdir() {
    use std::os::unix::fs::PermissionsExt;
    let scratch = process::Scratch::new().unwrap();
    let path = scratch.path().to_owned();
    assert!(path.starts_with("/tmp"));
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o700
    );
    drop(scratch);
    assert!(!path.exists());
}

#[test]
fn snapshot_rejects_changed_body_and_authority_and_binds_source_copy() {
    use spacetop_core::{parser, summary::snapshot};
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("task.md");
    let original = "---\nid: '1'\ntitle: Task\nstatus: implement\n---\n\nBody\n";
    std::fs::write(&path, original).unwrap();
    let entity = parser::parse_work_item(&path, &["implement".into()], None).unwrap();
    let captured = snapshot::capture(dir.path(), &entity, SummaryProvider::Claude).unwrap();
    std::fs::write(&path, original.replace("Body", "Changed")).unwrap();
    assert_eq!(
        snapshot::capture(dir.path(), &entity, SummaryProvider::Claude),
        Err(SummaryError::Changed)
    );
    std::fs::write(&path, original.replace("Task", "Another title")).unwrap();
    assert_eq!(
        snapshot::capture(dir.path(), &entity, SummaryProvider::Claude),
        Err(SummaryError::Changed)
    );
    std::fs::write(&path, original).unwrap();
    let codex = snapshot::capture(dir.path(), &entity, SummaryProvider::Codex).unwrap();
    assert_ne!(captured.key, codex.key);
}
