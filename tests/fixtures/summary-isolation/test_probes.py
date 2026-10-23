"""Local synthetic tests. No installed coding CLI or login is required."""

from dataclasses import replace
import os
from pathlib import Path
import platform
import signal
import subprocess
import sys
import tempfile
import time
import unittest
from unittest.mock import patch

from probe import environment, profile
from supervisor import Limits, run


CANARY = str(Path(__file__).resolve().with_name("canary.py"))


class SupervisorTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="summary-fixture-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()

    def invoke(self, mode, source=b"", **kwargs):
        return run([sys.executable, "-B", CANARY, mode], self.root,
                   environment(self.root), source=source, **kwargs)

    def test_source_remains_literal_stdin(self):
        source = b'--model other; $(touch escape)\n@outside-file\x00'
        result = self.invoke("echo", source)
        self.assertEqual((result.outcome, result.exit_code, result.stdout),
                         ("exited", 0, source))
        self.assertFalse((self.root / "escape").exists())

    def test_oversized_source_is_rejected_before_launch(self):
        result = run(["/nonexistent-summary-fixture"], self.root, {}, b"a" * 8193)
        self.assertEqual(result.outcome, "input_limit")
        self.assertIsNone(result.exit_code)

    def test_missing_executable_is_distinct(self):
        result = run(["/nonexistent-summary-fixture"], self.root, {})
        self.assertEqual(result.outcome, "missing_executable")

    def test_nonzero_exit_does_not_turn_into_success_envelope(self):
        result = self.invoke("error")
        self.assertEqual(result.exit_code, 1)
        self.assertIn(b'"is_error":true', result.stdout)

    def test_both_pipe_caps_are_enforced_during_flood(self):
        for stdout_cap, stderr_cap, expected in ((1024, 65536, "stdout_limit"),
                                                  (65536, 1024, "stderr_limit")):
            with self.subTest(expected=expected):
                limits = replace(Limits(), stdout_bytes=stdout_cap, stderr_bytes=stderr_cap)
                result = self.invoke("flood", limits=limits)
                self.assertEqual(result.outcome, expected)
                self.assertLessEqual(len(result.stdout), stdout_cap)
                self.assertLessEqual(len(result.stderr), stderr_cap)
                self.assertEqual(result.exit_code, -signal.SIGKILL)

    def test_deadline_includes_blocked_stdin(self):
        # Larger synthetic-only input fills the pipe. No provider is involved.
        limits = replace(Limits(), input_bytes=262144, seconds=0.15)
        started = time.monotonic()
        result = self.invoke("blocked-stdin", b"s" * 262144, limits=limits)
        self.assertEqual(result.outcome, "timeout")
        self.assertEqual(result.exit_code, -signal.SIGKILL)
        self.assertLess(time.monotonic() - started, 2)

    def test_cancel_reaps_direct_child(self):
        result = self.invoke("blocked-stdin", cancel_after=0.15)
        self.assertEqual((result.outcome, result.exit_code), ("cancelled", -signal.SIGKILL))

    def test_observer_receives_only_direct_launched_pid(self):
        observed = set()
        result = self.invoke("blocked-stdin", cancel_after=0.05, observe=observed.add)
        self.assertEqual(observed, {result.pid})
        self.assertEqual(result.exit_code, -signal.SIGKILL)

    @unittest.skipUnless(platform.system() == "Darwin", "libproc is a Darwin API")
    def test_libproc_child_count_includes_one_child(self):
        from auth_differential import child_pids
        child = subprocess.Popen([sys.executable, "-B", CANARY, "blocked-stdin"],
                                 stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        try:
            self.assertIn(child.pid, child_pids(os.getpid()))
        finally:
            child.kill()
            child.wait(timeout=2)

    def test_auth_paths_are_all_in_profile(self):
        # The historical helper probe appended only the LAST auth path because
        # the allow statement was outside its loop. Pin the corrected recipe.
        paths = [Path("/synthetic/claude.json"), Path("/synthetic/codex/auth.json")]
        policy = profile(self.root, Path(sys.executable), reads=paths,
                         helpers=("/usr/bin/security", "/bin/sh"))
        for path in paths:
            self.assertIn('(allow file-read-data (literal "' + str(path) + '"))', policy)
        self.assertNotIn("(deny process-fork)", policy)
        self.assertIn("(deny process-fork)", profile(self.root, Path(sys.executable)))

    def test_auth_environment_preserves_account_metadata_without_invented_overrides(self):
        with patch.dict(os.environ, {"HOME": str(self.root), "USER": "synthetic-user"}, clear=True):
            env = environment(self.root, real_home=True)
            self.assertEqual(env["USER"], "synthetic-user")
            self.assertNotIn("CLAUDE_CONFIG_DIR", env)
            self.assertNotIn("CODEX_HOME", env)
            self.assertNotIn("USER", environment(self.root))

    def test_killpg_does_not_contain_setsid_child(self):
        read_fd, write_fd = os.pipe()
        process = subprocess.Popen(
            [sys.executable, "-B", CANARY, "detached", str(read_fd), str(self.root)],
            cwd=self.root, env=environment(self.root), pass_fds=(read_fd,),
            start_new_session=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        os.close(read_fd)

        def await_condition(condition):
            deadline = time.monotonic() + 2
            while time.monotonic() < deadline:
                if condition():
                    return
                time.sleep(0.01)
            self.fail("fixture condition not observed before deadline")

        heartbeat = self.root / "escaped-heartbeat"
        try:
            await_condition(heartbeat.exists)
            os.killpg(process.pid, signal.SIGKILL)
            process.wait(timeout=2)
            before = heartbeat.read_text()
            await_condition(lambda: heartbeat.read_text() not in ("", before))
            self.assertFalse((self.root / "escaped-finished").exists())
        finally:
            # The live child owns only the read end. Close our lease, not a
            # guessed PID. A five-second child deadline is an extra backstop.
            os.close(write_fd)
            if process.poll() is None:
                os.killpg(process.pid, signal.SIGKILL)
                process.wait(timeout=2)
        await_condition(lambda: (self.root / "escaped-finished").exists())


if __name__ == "__main__":
    unittest.main()
