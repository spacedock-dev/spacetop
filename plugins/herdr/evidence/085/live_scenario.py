import os, sys, json, time, subprocess, pathlib, pty, fcntl, termios, struct, select, shutil, hashlib, argparse, tempfile

parser = argparse.ArgumentParser(
    description="Isolated Herdr shortcut scenario; never sends a model prompt"
)
parser.add_argument(
    "--root", type=pathlib.Path, help="Fresh temporary artifact directory"
)
parser.add_argument(
    "--codex-auth",
    type=pathlib.Path,
    help="Optional auth.json copied only to temporary Codex home and removed afterward",
)
parser.add_argument(
    "--bin-dir",
    type=pathlib.Path,
    help="Directory containing installed spacetop and spacetop-herdr",
)
args = parser.parse_args()
code = pathlib.Path(__file__).resolve().parents[4]
root = (args.root or pathlib.Path(tempfile.mkdtemp(prefix="spacetop-herdr-"))).resolve()
if not any(
    root.is_relative_to(path.resolve())
    for path in [pathlib.Path(tempfile.gettempdir()), pathlib.Path("/tmp")]
):
    raise RuntimeError("artifact root must be under the system temporary directory")
root.mkdir(parents=True, exist_ok=True)
if (root / "config").exists() or (root / "home").exists():
    raise RuntimeError("use a fresh root to isolate registry/session")
shutil.copyfile(pathlib.Path(__file__).with_name("config.toml"), root / "config.toml")
bin_dir = (args.bin_dir or code / "target/debug").resolve()
for name in ["spacetop", "spacetop-herdr"]:
    if not (bin_dir / name).is_file():
        raise RuntimeError("Build/install both binaries before running the scenario")
herdr = shutil.which("herdr")
if not herdr:
    raise RuntimeError("Install Herdr 0.9.3 or later")
print("Artifacts:", root, flush=True)
e = os.environ.copy()
for k in list(e):
    if k.startswith("HERDR_") or k.startswith("CODEX_"):
        e.pop(k, None)
e.update(
    HOME=str(root / "home"),
    XDG_CONFIG_HOME=str(root / "config"),
    XDG_STATE_HOME=str(root / "state"),
    CODEX_HOME=str(root / "codex"),
    HERDR_CONFIG_PATH=str(root / "config.toml"),
    PATH=str(bin_dir) + ":" + e["PATH"],
    TERM="xterm-256color",
)
for k in ["HOME", "XDG_CONFIG_HOME", "XDG_STATE_HOME", "CODEX_HOME"]:
    pathlib.Path(e[k]).mkdir(parents=True, exist_ok=True)
# Only auth is copied into temporary agent home; no source config/layout is shared and no model prompt is sent.
auth = args.codex_auth

session = "spacetop-sidecar-proof"
base = [herdr, "--session", session]
proof = {}


def call(k, a, check=True):
    p = subprocess.run(base + a, env=e, capture_output=True, text=True, timeout=45)
    proof[k] = {"argv": a, "exit": p.returncode, "stdout": p.stdout, "stderr": p.stderr}
    (root / "proof.json").write_text(json.dumps(proof, indent=2))
    print(k, p.returncode, p.stdout[:200], p.stderr[:200], flush=True)
    if check and p.returncode:
        raise RuntimeError(k)
    try:
        return json.loads(p.stdout)["result"]
    except:
        return p.stdout


def git(a, cwd):
    subprocess.run(["git"] + a, cwd=cwd, check=True, capture_output=True)


project = root / "main project"
worktree = root / "invoking worktree spaces ; $(touch NEVER)"
project.mkdir(exist_ok=True)
git(["init", "-q"], project)
for folder, label in [
    ("workflow-a", "WORKTREE-085-A"),
    ("workflow-b", "WORKTREE-085-B"),
]:
    d = project / folder
    d.mkdir(exist_ok=True)
    (d / "README.md").write_text(
        "---\ncommissioned-by: spacedock@0.27.3\nname: "
        + label
        + "\nstages:\n  states:\n    - name: implement\n      initial: true\n    - name: done\n      terminal: true\n---\n"
    )
    (d / "001.md").write_text(
        "---\nid: 001\ntitle: "
        + label
        + " recognizable\nstatus: implement\n---\nRead-only marker\n"
    )
git(["add", "."], project)
if subprocess.check_output(["git", "status", "--porcelain"], cwd=project).strip():
    git(
        [
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.com",
            "commit",
            "-qm",
            "fixture",
        ],
        project,
    )
if not worktree.exists():
    git(["worktree", "add", "-qb", "live-sidecar", str(worktree)], project)
# Make the worktree's content distinguishable without mutating it during open/reuse.
(worktree / "workflow-a/001.md").write_text(
    (worktree / "workflow-a/001.md")
    .read_text()
    .replace("recognizable", "INVOKING-WORKTREE")
)


def snapshot():
    return {
        "files": {
            str(p.relative_to(worktree)): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in worktree.rglob("*.md")
        },
        "head": subprocess.check_output(
            ["git", "rev-parse", "HEAD"], cwd=worktree, text=True
        ),
        "status": subprocess.check_output(
            ["git", "status", "--porcelain"], cwd=worktree, text=True
        ),
        "index": hashlib.sha256(
            pathlib.Path(
                subprocess.check_output(
                    ["git", "rev-parse", "--git-path", "index"], cwd=worktree, text=True
                ).strip()
            ).read_bytes()
        ).hexdigest(),
    }


proof["before"] = snapshot()
serverlog = (root / "server.log").open("wb")
server = subprocess.Popen(
    base + ["server"], env=e, stdout=serverlog, stderr=subprocess.STDOUT, cwd=worktree
)
client = None
fd = None
try:
    if auth is not None:
        shutil.copyfile(auth, root / "codex/auth.json")
    for _ in range(100):
        if (root / f"config/herdr/sessions/{session}/herdr.sock").exists():
            break
        time.sleep(0.05)
    call("initial-plugins", ["plugin", "list"])
    w = call(
        "workspace",
        [
            "workspace",
            "create",
            "--cwd",
            str(worktree),
            "--label",
            "agent-worktree",
            "--focus",
        ],
    )
    wid = w["workspace"]["workspace_id"]
    pid = w["root_pane"]["pane_id"]
    call("link", ["plugin", "link", str(code / "plugins/herdr")])
    master, slave = pty.openpty()
    fd = master
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 48, 180, 0, 0))
    client = subprocess.Popen(
        base,
        env=e,
        stdin=slave,
        stdout=slave,
        stderr=slave,
        cwd=worktree,
        start_new_session=True,
    )
    os.close(slave)
    os.set_blocking(master, False)
    screen = open(root / "client.ansi", "wb")

    def drain(seconds=0.5):
        stop = time.time() + seconds
        while time.time() < stop:
            r, _, _ = select.select([master], [], [], 0.05)
            if r:
                try:
                    screen.write(os.read(master, 65536))
                    screen.flush()
                except (BlockingIOError, OSError):
                    pass

    drain(2)
    call(
        "agent-start",
        [
            "agent",
            "start",
            "sidecar-proof",
            "--kind",
            "codex",
            "--pane",
            pid,
            "--timeout",
            "15000",
            "--",
            "--no-alt-screen",
        ],
    )
    call("agent-before", ["agent", "get", "sidecar-proof"], False)
    call("agent-screen", ["pane", "read", pid, "--source", "visible"])

    def shortcut(k):
        os.write(master, b"\x02")
        drain(0.15)
        os.write(master, b"v")
        drain(1)
        proof[k + "-input"] = {
            "hex": "0276",
            "meaning": "Ctrl+B then v to Herdr client PTY",
        }
        call(k + "-log", ["plugin", "log"])
        return call(k + "-panes", ["pane", "list", "--workspace", wid])["panes"]

    panes = shortcut("first")
    side = [p for p in panes if p["pane_id"] != pid]
    if len(side) != 1:
        raise RuntimeError("shortcut did not create exactly one sidecar")
    sid = side[0]["pane_id"]
    call("first-layout", ["pane", "layout", "--pane", pid])
    call("first-process", ["pane", "process-info", "--pane", sid])
    call("first-screen", ["pane", "read", sid, "--source", "visible"])
    shortcut("reuse")
    call("reuse-layout", ["pane", "layout", "--pane", pid])
    call("close", ["plugin", "pane", "close", sid])
    call("return-agent", ["pane", "focus", "--direction", "left", "--pane", pid], False)
    panes = shortcut("reopen")
    sid2 = [p["pane_id"] for p in panes if p["pane_id"] != pid][0]
    other = call(
        "workspace-two",
        [
            "workspace",
            "create",
            "--cwd",
            str(project),
            "--label",
            "other-project",
            "--focus",
        ],
    )
    wid2 = other["workspace"]["workspace_id"]
    pid2 = other["root_pane"]["pane_id"]
    os.write(master, b"\x02")
    drain(0.15)
    os.write(master, b"v")
    drain(1)
    call("workspace-two-log", ["plugin", "log"])
    call("workspace-two-panes", ["pane", "list", "--workspace", wid2])
    call("workspace-one-final", ["pane", "list", "--workspace", wid])
    proof["after"] = snapshot()
    proof["readonly_equal"] = proof["before"] == proof["after"]
    proof["crafted_path_executed"] = (worktree / "NEVER").exists()
    proof["version"] = subprocess.check_output(
        base + ["--version"], env=e, text=True
    ).strip()
    assert proof["readonly_equal"] and not proof["crafted_path_executed"]
    first_layout = json.loads(proof["first-layout"]["stdout"])["result"]["layout"]
    reuse_layout = json.loads(proof["reuse-layout"]["stdout"])["result"]["layout"]
    assert (
        first_layout["focused_pane_id"] == pid
        and first_layout["splits"][0]["direction"] == "right"
    )
    assert reuse_layout["focused_pane_id"] == sid and len(reuse_layout["panes"]) == 2
    assert sid2 != sid
    p2 = json.loads(proof["workspace-two-panes"]["stdout"])["result"]["panes"]
    assert len(p2) == 2 and all(p["workspace_id"] == wid2 for p in p2)
    assert (
        "INVOKING-WORKTREE" in proof["first-screen"]["stdout"]
        and "workflow-b" in proof["first-screen"]["stdout"]
    )
    call("unlink", ["plugin", "unlink", "spacetop.sidecar"])
finally:
    (root / "codex/auth.json").unlink(missing_ok=True)
    if client is not None:
        client.terminate()
        client.wait(timeout=5)
    call("stop", ["session", "stop", session], False)
    try:
        server.wait(timeout=5)
    except:
        server.terminate()
    (root / "proof.json").write_text(json.dumps(proof, indent=2))
