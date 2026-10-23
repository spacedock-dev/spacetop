"""Test the actual Darwin syscall filters; no coding CLI or credentials."""

import argparse
import json
from pathlib import Path
import platform
import sys
import tempfile

from probe import environment, FIXTURE, profile, sandbox_run


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("candidate", choices=("setcontrol", "direct-syscalls", "all-session-syscalls"))
    args = parser.parse_args()
    if platform.system() != "Darwin":
        raise SystemExit("macOS only")
    rules = {
        "setcontrol": "(deny process-info-setcontrol)",
        "direct-syscalls": "(deny syscall-unix (syscall-number SYS_setsid) (syscall-number SYS_setpgid))",
        "all-session-syscalls": "(deny syscall-unix (syscall-number SYS_setsid) (syscall-number SYS_setpgid) (syscall-number SYS_posix_spawn))",
    }
    # The observed Homebrew launcher itself uses posix_spawn. Select its actual
    # framework binary for the syscall canary, not a different runtime policy.
    framework = Path(sys.base_prefix) / "Resources/Python.app/Contents/MacOS/Python"
    executable = framework if framework.is_file() else Path(sys.executable).resolve()
    with tempfile.TemporaryDirectory(prefix="summary-session-policy-") as temporary:
        scratch = Path(temporary).resolve()
        policy = profile(scratch, executable, helpers=("/bin/sh",), python=True) + rules[args.candidate]
        result = sandbox_run(policy, [str(executable), "-B", str(FIXTURE / "canary.py"), "session-syscalls"],
                             scratch, environment(scratch))
        print(json.dumps({"candidate": args.candidate, "outcome": result.outcome,
                          "exit_code": result.exit_code, "stdout": result.stdout.decode(errors="replace"),
                          "stderr": result.stderr.decode(errors="replace"), "production_ready": False}, indent=2))


if __name__ == "__main__":
    main()
