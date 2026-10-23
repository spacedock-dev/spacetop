"""One changed Claude helper capability per explicit local auth-status probe.

Never prints provider output or opens credentials. Kernel logs are limited to
this exact CLI PID; helper diagnostics must not be inferred from unrelated PIDs.
"""

import argparse
import ctypes
import datetime
import json
import os
from pathlib import Path
import platform
import shutil
import tempfile

from probe import auth_result, environment, literal, profile, sandbox_run
from supervisor import Limits, run


def child_pids(pid):
    """Sample current direct children only; this is not a containment primitive."""
    libproc = ctypes.CDLL("/usr/lib/libproc.dylib")
    libproc.proc_listchildpids.argtypes = [ctypes.c_int, ctypes.c_void_p, ctypes.c_int]
    libproc.proc_listchildpids.restype = ctypes.c_int
    children = (ctypes.c_int * 64)()
    # libproc returns a PID count here, unlike proc_listpids' byte count.
    count = libproc.proc_listchildpids(pid, children, ctypes.sizeof(children))
    return [child for child in children[:max(0, count)] if child > 0]


def observe_descendants(pid, observed):
    pending, visited = [pid], set()
    while pending:
        current = pending.pop()
        if current in visited:
            continue
        visited.add(current)
        observed.add(current)
        pending.extend(child_pids(current))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--read", action="append", default=[])
    parser.add_argument("--deny-setcontrol", action="store_true")
    parser.add_argument("--observe-helpers", action="store_true")
    parser.add_argument("--keychain-metadata", action="store_true",
                        help="Query only the default keychain path, never an item/value")
    args = parser.parse_args()
    if platform.system() != "Darwin":
        raise SystemExit("macOS only")
    executable = (Path("/usr/bin/security") if args.keychain_metadata
                  else Path(shutil.which("claude")).resolve(strict=True))
    with tempfile.TemporaryDirectory(prefix="summary-auth-diff-") as directory:
        scratch = Path(directory).resolve()
        env = environment(scratch, real_home=True)
        home = Path(env["HOME"])
        claude_home = Path(env.get("CLAUDE_CONFIG_DIR", str(home / ".claude")))
        reads = [home / ".claude.json", claude_home / ".credentials.json",
                 home / ".CFUserTextEncoding",
                 Path(f"/private/var/db/mds/messages/{os.getuid()}/se_SecurityMessages")]
        policy = profile(scratch, executable, reads, helpers=("/usr/bin/security", "/bin/sh"))
        for path in args.read:
            if not Path(path).is_absolute():
                raise SystemExit("Exact read paths must be absolute")
            policy += "(allow file-read-data " + literal(path) + ")\n"
        if args.deny_setcontrol:
            policy += "(deny process-info-setcontrol)\n"
        started = datetime.datetime.now().strftime("%Y-%m-%d %H:%M:%S")
        observed = set()
        def observe(pid):
            observe_descendants(pid, observed)
        argv = ([str(executable), "default-keychain", "-d", "user"] if args.keychain_metadata else
                [str(executable), "--safe-mode", "auth", "status", "--json"])
        result = sandbox_run(policy, argv,
                             scratch, env, observe=observe if args.observe_helpers else None)
        observed.add(result.pid)
        predicate = " OR ".join(f'eventMessage CONTAINS "({pid}) deny("' for pid in sorted(observed))
        logs = run(["/usr/bin/log", "show", "--start", started, "--style", "json",
                    "--predicate", predicate], scratch, environment(scratch),
                   limits=Limits(seconds=15))
        try:
            messages = [item["eventMessage"] for item in json.loads(logs.stdout)
                        if "Sandbox:" in item.get("eventMessage", "") and any(
                            f"({pid}) deny(" in item["eventMessage"] for pid in observed)]
        except ValueError:
            messages = ["Log output unavailable or exceeded bound"]
        status = ({"exit_code": result.exit_code, "outcome": result.outcome,
                   "default_keychain_path": result.stdout.decode(errors="replace").strip(),
                   "stderr": result.stderr.decode(errors="replace")} if args.keychain_metadata else auth_result("claude", result))
        report = {"candidate_reads": args.read, "deny_setcontrol": args.deny_setcontrol,
                  "keychain_metadata_only": args.keychain_metadata,
                  "auth": status, "pid": result.pid,
                  "observed_pids": sorted(observed), "own_pid_denials": messages,
                  "log_outcome": logs.outcome,
                  "live_generation_launches": 0, "production_ready": False}
        print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
