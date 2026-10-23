"""Synthetic OS canary. Never receives real credentials or task data."""

import errno
import json
import os
from pathlib import Path
import select
import socket
import sys
import time


def denied(action):
    try:
        action()
        return "allowed"
    except OSError as error:
        return errno.errorcode.get(error.errno, str(error.errno))


def fork_once():
    pid = os.fork()
    if pid == 0:
        os._exit(0)
    os.waitpid(pid, 0)


def boundary(scratch, outside):
    # exec is attempted in a fresh invocation, so a successful exec cannot
    # suppress the other results. These are synthetic, non-secret paths.
    results = {
        "fork": denied(fork_once),
        "outside_read": denied(lambda: (outside / "sentinel").read_bytes()),
        "outside_write": denied(lambda: (outside / "changed").write_text("x")),
        "network": denied(lambda: socket.create_connection(("127.0.0.1", 9), 1)),
        "scratch_write": denied(lambda: (scratch / "written").write_text("ok")),
    }
    print(json.dumps(results), flush=True)


def detached(lease_fd, scratch):
    pid = os.fork()
    if pid == 0:
        os.setsid()
        (scratch / "escaped-ready").write_text(str(os.getpid()))
        # The supervisor owns the only write end. EOF always ends this fixture,
        # even when the supervisor crashes. Never kill a PID read from a file.
        deadline = time.monotonic() + 5
        counter = 0
        while time.monotonic() < deadline:
            if select.select([lease_fd], [], [], 0.01)[0]:
                os.read(lease_fd, 1)
                break
            counter += 1
            (scratch / "escaped-heartbeat").write_text(str(counter))
        (scratch / "escaped-finished").write_text("lease closed")
        os._exit(0)
    (scratch / "parent-ready").write_text(str(os.getpid()))
    while True:
        time.sleep(0.01)


def session_syscalls():
    for name, action in (("setsid", os.setsid), ("setpgid", lambda: os.setpgid(0, 0))):
        pid = os.fork()
        if pid == 0:
            result = denied(action)
            print(json.dumps({"operation": name, "result": result}), flush=True)
            os._exit(0)
        os.waitpid(pid, 0)
    for name, options in (("spawn_setsid", {"setsid": True}), ("spawn_setpgroup", {"setpgroup": 0})):
        try:
            pid = os.posix_spawn(sys.executable, [sys.executable, "-B", __file__, "identity", name],
                                 os.environ, **options)
            os.waitpid(pid, 0)
        except OSError as error:
            print(json.dumps({"operation": name, "result": errno.errorcode.get(error.errno)}), flush=True)


if __name__ == "__main__":
    mode = sys.argv[1]
    if mode == "boundary":
        boundary(Path(sys.argv[2]), Path(sys.argv[3]))
    elif mode == "exec":
        print(denied(lambda: os.execv("/bin/sh", ["/bin/sh", "-c", "printf EXEC_ALLOWED"])))
    elif mode == "detached":
        detached(int(sys.argv[2]), Path(sys.argv[3]))
    elif mode == "echo":
        sys.stdout.buffer.write(sys.stdin.buffer.read())
    elif mode == "flood":
        while True:
            os.write(1, b"o" * 4096)
            os.write(2, b"e" * 4096)
    elif mode == "blocked-stdin":
        time.sleep(30)
    elif mode == "error":
        print('{"subtype":"success","is_error":true}')
        sys.exit(1)
    elif mode == "session-syscalls":
        session_syscalls()
    elif mode == "identity":
        print(json.dumps({"operation": sys.argv[2], "result": "allowed", "pid": os.getpid(),
                          "sid": os.getsid(0), "pgrp": os.getpgrp()}), flush=True)
