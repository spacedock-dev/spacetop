#!/usr/bin/env python3
"""Controlled Herdr command boundary; never used by the product."""
import json, os, sys, time
from pathlib import Path

root = Path(os.environ["TEST_HERDR_ROOT"])
a = sys.argv[1:]
with (root / "argv.jsonl").open("a") as f:
    f.write(json.dumps(a) + "\n")
mode = os.environ.get("TEST_HERDR_MODE", "")
if mode == "api-fail":
    print("socket unavailable", file=sys.stderr)
    sys.exit(1)
if mode == "malformed":
    print("{")
    sys.exit(0)
panes = json.loads((root / "panes.json").read_text())


def option(k):
    return a[a.index(k) + 1]


if a[:2] == ["pane", "list"]:
    if mode == "slow":
        time.sleep(0.4)
    result = {"panes": panes}
elif a[:3] == ["plugin", "pane", "open"]:
    pane = dict(
        pane_id=panes[0]["workspace_id"] + ":p2",
        workspace_id=panes[0]["workspace_id"],
        tab_id=panes[0]["tab_id"],
        cwd=option("--cwd"),
        focused=False,
        tokens={},
    )
    if mode == "wrong-open":
        pane["focused"] = True
    panes.append(pane)
    result = {
        "plugin_pane": dict(
            plugin_id="spacetop.sidecar", entrypoint="inspector", pane=pane
        )
    }
elif a[:2] == ["pane", "report-metadata"]:
    pane = next(p for p in panes if p["pane_id"] == a[2])
    for i, x in enumerate(a):
        if x == "--token":
            k, v = a[i + 1].split("=", 1)
            pane["tokens"][k] = v[:80]
    result = {}
elif a[:3] == ["plugin", "pane", "focus"]:
    if mode == "focus-fail":
        print("not a plugin pane", file=sys.stderr)
        sys.exit(1)
    pane = next(p for p in panes if p["pane_id"] == a[3])
    pane["focused"] = True
    result = {
        "plugin_pane": dict(
            plugin_id="other" if mode == "wrong-owner" else "spacetop.sidecar",
            entrypoint="inspector",
            pane=pane,
        )
    }
else:
    raise RuntimeError(a)
(root / "panes.json").write_text(json.dumps(panes))
if a[:2] != ["pane", "report-metadata"]:
    print(json.dumps({"result": result}))
