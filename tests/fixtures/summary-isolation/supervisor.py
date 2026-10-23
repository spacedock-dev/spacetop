"""Bounded LOCAL PROBE runner, not a production containment implementation.

Process groups cannot contain setsid descendants. Only trusted fixtures, or
commands under a separately established no-fork OS boundary, belong here.
"""

from dataclasses import dataclass
import os
from pathlib import Path
import selectors
import signal
import subprocess
import time


@dataclass(frozen=True)
class Limits:
    input_bytes: int = 8192
    stdout_bytes: int = 65536
    stderr_bytes: int = 16384
    seconds: float = 6


@dataclass(frozen=True)
class Result:
    outcome: str
    exit_code: int | None
    stdout: bytes
    stderr: bytes


def stop_group(process):
    try:
        os.killpg(process.pid, signal.SIGKILL)
    except ProcessLookupError:
        pass
    process.wait()


def run(argv: list[str], cwd: Path, env: dict[str, str], source: bytes = b"",
        limits: Limits = Limits(), cancel_after: float | None = None) -> Result:
    """Drain both pipes while writing stdin; never collect unbounded output."""
    if len(source) > limits.input_bytes:
        return Result("input_limit", None, b"", b"")
    try:
        process = subprocess.Popen(argv, cwd=cwd, env=env, stdin=subprocess.PIPE,
                                   stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                   start_new_session=True)
    except FileNotFoundError:
        return Result("missing_executable", None, b"", b"")
    buffers = {"stdout": bytearray(), "stderr": bytearray()}
    caps = {"stdout": limits.stdout_bytes, "stderr": limits.stderr_bytes}
    started = time.monotonic()
    outcome = "exited"
    sent = 0
    try:
        with selectors.DefaultSelector() as selector:
            for name in ("stdout", "stderr"):
                pipe = getattr(process, name)
                os.set_blocking(pipe.fileno(), False)
                selector.register(pipe, selectors.EVENT_READ, name)
            if source:
                os.set_blocking(process.stdin.fileno(), False)
                selector.register(process.stdin, selectors.EVENT_WRITE, "stdin")
            else:
                process.stdin.close()
            while selector.get_map() or process.poll() is None:
                elapsed = time.monotonic() - started
                if cancel_after is not None and elapsed >= cancel_after:
                    outcome = "cancelled"
                    break
                if elapsed >= limits.seconds:
                    outcome = "timeout"
                    break
                for key, _ in selector.select(timeout=0.01):
                    pipe, name = key.fileobj, key.data
                    if name == "stdin":
                        try:
                            sent += os.write(pipe.fileno(), source[sent:sent + 4096])
                        except BrokenPipeError:
                            sent = len(source)
                        if sent == len(source):
                            selector.unregister(pipe)
                            pipe.close()
                        continue
                    chunk = os.read(pipe.fileno(), 4096)
                    if not chunk:
                        selector.unregister(pipe)
                        pipe.close()
                        continue
                    remaining = caps[name] - len(buffers[name])
                    buffers[name].extend(chunk[:remaining])
                    if len(chunk) > remaining:
                        outcome = name + "_limit"
                        break
                if outcome != "exited":
                    break
    finally:
        # This reaps the direct process, but is NOT a detached-child guarantee.
        if outcome != "exited" or process.poll() is None:
            stop_group(process)
        else:
            process.wait()
        for pipe in (process.stdin, process.stdout, process.stderr):
            pipe.close()
    return Result(outcome, process.returncode, bytes(buffers["stdout"]),
                  bytes(buffers["stderr"]))
