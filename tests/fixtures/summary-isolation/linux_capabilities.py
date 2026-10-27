"""Read Linux capabilities and attempt one private user/PID namespace, no CLI."""

import ctypes
import errno
import json
import os
from pathlib import Path
import platform

libc = ctypes.CDLL(None, use_errno=True)
report = {"machine": platform.machine(), "kernel": platform.release(), "pid": os.getpid()}
result = libc.unshare(0x10000000 | 0x20000000)  # CLONE_NEWUSER | CLONE_NEWPID
report["unshare_user_pid"] = "allowed" if result == 0 else errno.errorcode.get(ctypes.get_errno())
report["cgroup_kill_present"] = Path("/sys/fs/cgroup/cgroup.kill").exists()
report["cgroup_root_writable"] = os.access("/sys/fs/cgroup", os.W_OK)
report["subreaper_set"] = libc.prctl(36, 1, 0, 0, 0)  # PR_SET_CHILD_SUBREAPER
report["native_cli_auth_proved"] = False
print(json.dumps(report, indent=2))
