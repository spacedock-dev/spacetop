"""Reproduce the macOS isolation blocker without invoking a model.

No provider generation command exists in this script. Auth status is opt-in and
emits only booleans/enum fields. The probe never opens credential files.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import sys
import tempfile

from supervisor import Limits, run


FIXTURE = Path(__file__).resolve().parent
SANDBOX = "/usr/bin/sandbox-exec"


def literal(path):
    return "(literal " + json.dumps(str(path)) + ")"


def subpath(path):
    return "(subpath " + json.dumps(str(path)) + ")"


def profile(scratch, executable, reads=(), helpers=(), python=False):
    # Operation-specific feasibility policy, NOT a complete OS capability policy.
    rules = ["(version 1)", "(allow default)", "(deny network*)",
             "(deny file-read-data)", "(deny file-write*)",
             "(deny process-exec)"]
    if not helpers:
        rules.append("(deny process-fork)")
    for path in ("/", "/private", "/private/tmp", "/private/var/tmp", "/usr/bin"):
        rules.append("(allow file-read-data " + literal(path) + ")")
    for path in ("/System", "/dev", "/private/preboot", "/usr/lib", "/usr/share",
                 "/private/var/db/dyld", "/private/var/db/timezone", scratch):
        rules.append("(allow file-read-data " + subpath(path) + ")")
    rules.append("(allow file-write* " + subpath(scratch) + ")")
    rules.append('(allow file-write-data (literal "/dev/null") (literal "/dev/dtracehelper"))')
    for path in (executable, *helpers):
        rules += ["(allow file-read-data " + literal(path) + ")",
                  "(allow process-exec " + literal(path) + ")"]
    for path in reads:
        rules.append("(allow file-read-data " + literal(path) + ")")
    if python:
        # Homebrew's Python launcher re-execs its framework binary. This broad
        # interpreter runtime allowance is CANARY-ONLY, never an adapter policy.
        rules += ["(allow file-read-data " + subpath(Path(sys.base_prefix).resolve()) + ")",
                  "(allow process-exec " + subpath(Path(sys.base_prefix).resolve()) + ")",
                  "(allow file-read-data " + literal(FIXTURE / "canary.py") + ")"]
    return "\n".join(rules) + "\n"


def environment(scratch, real_home=False):
    home = Path.home() if real_home else scratch
    env = {"HOME": str(home), "PATH": "/usr/bin:/bin",
           "CLAUDE_CODE_TMPDIR": str(scratch), "TMPDIR": str(scratch),
           "PYTHONDONTWRITEBYTECODE": "1"}
    # Preserve absence as part of the selected configuration. Do not invent
    # config overrides while testing existing-login behavior.
    for name in ("CODEX_HOME", "CLAUDE_CONFIG_DIR"):
        if real_home and name in os.environ:
            env[name] = os.environ[name]
    # Claude's local Keychain lookup changes when USER is absent, even with
    # HOME preserved. This is account selection metadata, not a credential.
    if real_home and "USER" in os.environ:
        env["USER"] = os.environ["USER"]
    return env


def sandbox_run(policy, argv, scratch, env):
    policy_file = scratch / "policy.sb"
    policy_file.write_text(policy)
    return run([SANDBOX, "-f", str(policy_file), *argv], scratch, env,
               limits=Limits(seconds=10))


def boundary_probe(scratch, outside):
    executable = Path(sys.executable).resolve()
    policy = profile(scratch, executable, python=True)
    env = environment(scratch)
    argv = [str(executable), "-B", str(FIXTURE / "canary.py")]
    result = sandbox_run(policy, [*argv, "boundary", str(scratch), str(outside)], scratch, env)
    execute = sandbox_run(policy, [*argv, "exec"], scratch, env)
    try:
        observed = json.loads(result.stdout)
    except ValueError:
        observed = {"decode_error": True}
    return {"outcome": result.outcome, "exit_code": result.exit_code,
            "checks": observed, "exec": execute.stdout.decode(errors="replace").strip(),
            "exec_outcome": execute.outcome, "exec_exit_code": execute.exit_code,
            "outside_unchanged": sorted(p.name for p in outside.iterdir()) == ["sentinel"],
            "stderr_bytes": len(result.stderr) + len(execute.stderr)}


def auth_result(name, result):
    safe = {"outcome": result.outcome, "exit_code": result.exit_code}
    if name == "claude":
        try:
            data = json.loads(result.stdout)
            safe["logged_in"] = data.get("loggedIn") is True
            # Never forward arbitrary provider strings or account identity.
            safe["claude_ai"] = data.get("authMethod") == "claude.ai"
        except ValueError:
            safe["decode_error"] = True
    else:
        safe["logged_in_chatgpt"] = b"Logged in using ChatGPT" in result.stdout + result.stderr
    return safe


def auth_probe(name, scratch):
    located = shutil.which(name)
    if located is None:
        return {"outcome": "missing_executable"}
    executable = Path(located).resolve(strict=True)
    env = environment(scratch, real_home=True)
    home = Path(env["HOME"])
    codex_home = Path(env.get("CODEX_HOME", str(home / ".codex")))
    claude_home = Path(env.get("CLAUDE_CONFIG_DIR", str(home / ".claude")))
    if not codex_home.is_absolute() or not claude_home.is_absolute():
        return {"outcome": "unsupported_relative_cli_home"}
    reads = [home / ".claude.json", claude_home / ".credentials.json",
             codex_home / "auth.json", codex_home / "config.toml",
             home / ".CFUserTextEncoding",
             Path(f"/private/var/db/mds/messages/{os.getuid()}/se_SecurityMessages")]
    argv = [str(executable), *(["--safe-mode", "auth", "status", "--json"]
                              if name == "claude" else ["login", "status"])]
    reference = "(version 1)(allow default)(deny network*)(deny file-write*)" + (
        "(allow file-write* " + subpath(scratch) + ")"
        '(allow file-write-data (literal "/dev/null") (literal "/dev/dtracehelper"))')
    results = {"executable": str(executable),
               "sha256": hashlib.sha256(executable.read_bytes()).hexdigest()}
    policies = {"network_denied_reference": reference,
                "strict_nofork": profile(scratch, executable, reads)}
    if name == "claude":
        policies["nofork_diagnostic_only"] = reference + "(deny process-fork)"
        policies["exec_helpers_diagnostic_only"] = reference + "(deny process-exec)" + "".join(
            "(allow process-exec " + literal(path) + ")"
            for path in (executable, "/usr/bin/security", "/bin/sh"))
        # Diagnostic only: allowing fork and shells gives up the descendant
        # property. It cannot graduate to production even if auth starts working.
        policies["helpers_diagnostic_only"] = profile(
            scratch, executable, reads, helpers=("/usr/bin/security", "/bin/sh"))
    for label, policy in policies.items():
        result = sandbox_run(policy, argv, scratch, env)
        results[label] = auth_result(name, result)
    return results


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--auth-status", action="store_true",
                        help="Run network-denied local status commands with existing login")
    args = parser.parse_args()
    report = {"platform": platform.system(), "machine": platform.machine(),
              "live_generation_launches": 0, "production_ready": False}
    if platform.system() != "Darwin":
        report["outcome"] = "unsupported_probe_platform"
        print(json.dumps(report, indent=2))
        return 2
    with tempfile.TemporaryDirectory(prefix="spacetop-summary-probe-") as temporary:
        root = Path(temporary).resolve()
        scratch, outside = root / "scratch", root / "outside"
        scratch.mkdir(mode=0o700)
        outside.mkdir(mode=0o700)
        (outside / "sentinel").write_text("synthetic non-secret data")
        report["boundary"] = boundary_probe(scratch, outside)
        if args.auth_status:
            report["auth"] = {name: auth_probe(name, scratch) for name in ("claude", "codex")}
    print(json.dumps(report, indent=2))
    checks = report["boundary"]["checks"]
    expected = {"fork": "EPERM", "outside_read": "EPERM", "outside_write": "EPERM",
                "network": "EPERM", "scratch_write": "allowed"}
    boundary = report["boundary"]
    passed = (checks == expected and boundary["exec"] == "EPERM"
              and boundary["exit_code"] == 0 and boundary["exec_exit_code"] == 0
              and boundary["outcome"] == "exited" and boundary["exec_outcome"] == "exited"
              and boundary["outside_unchanged"])
    return 0 if passed else 1


if __name__ == "__main__":
    sys.exit(main())
