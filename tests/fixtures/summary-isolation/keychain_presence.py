"""Single metadata-only discriminator for one statically inspected CLI build.

No enumeration, secret data, attributes, CLI invocation, or provider request.
Unknown build/config identity fails closed before Security.framework is called.
"""

import hashlib
import json
import os
from pathlib import Path
import platform
import re
import subprocess
import tempfile

from probe import environment, profile, sandbox_run


FIXTURE = Path(__file__).resolve().parent
CLI = Path("/Users/kent/.local/share/claude/versions/2.1.309")
CLI_SHA256 = "5eadc0009cdcdbc0d06e58110d7b2baf88ced6088b1c33a60a21217fe410619c"


def verified_identity(digest, env):
    """Only the inspected production/default-config mapping is supported."""
    if digest != CLI_SHA256:
        raise ValueError("Uninspected CLI artifact; no Keychain query permitted")
    for key in ("CLAUDE_CONFIG_DIR", "CLAUDE_SECURESTORAGE_CONFIG_DIR",
                "CLAUDE_CODE_CUSTOM_OAUTH_URL"):
        if key in env:
            raise ValueError("Uninspected config identity; no Keychain query permitted")
    account = env.get("USER", "")
    if not re.fullmatch(r"[a-zA-Z0-9._-]+", account):
        raise ValueError("Uninspected account selection; no Keychain query permitted")
    return "Claude Code-credentials", account


def main():
    if platform.system() != "Darwin":
        raise SystemExit("macOS only")
    digest = hashlib.sha256(CLI.read_bytes()).hexdigest()
    service, account = verified_identity(digest, os.environ)
    with tempfile.TemporaryDirectory(prefix="summary-keychain-presence-") as directory:
        scratch = Path(directory).resolve()
        executable = scratch / "keychain-presence"
        # Deprecated legacy APIs intentionally pair with the modern query flag.
        subprocess.run(["/usr/bin/xcrun", "clang", "-Wall", "-Wextra", "-Werror",
                        "-Wno-deprecated-declarations", "-framework", "Security",
                        "-framework", "CoreFoundation", str(FIXTURE / "keychain_presence.c"),
                        "-o", str(executable)], check=True, timeout=30)
        env = environment(scratch, real_home=True)
        home = Path(env["HOME"])
        reads = [home / ".claude.json", home / ".claude" / ".credentials.json",
                 home / ".CFUserTextEncoding",
                 Path(f"/private/var/db/mds/messages/{os.getuid()}/se_SecurityMessages")]
        policy = profile(scratch, executable, reads)
        result = sandbox_run(policy, [str(executable), service, account], scratch, env)
        # Native probe stdout has only statuses/booleans. Never forward stderr.
        observed = json.loads(result.stdout) if result.stdout else None
        print(json.dumps({"cli_sha256": digest, "service_mapping": "production/default-config",
                          "policy": policy.replace(str(scratch), "<scratch>"),
                          "outcome": result.outcome, "exit_code": result.exit_code,
                          "stderr_bytes": len(result.stderr), "query": observed,
                          "live_generation_launches": 0, "production_ready": False}, indent=2))


if __name__ == "__main__":
    main()
