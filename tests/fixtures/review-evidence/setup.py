#!/usr/bin/env python3
"""Build disposable review evidence from the repository's compatibility fixtures."""
import hashlib
import json
from pathlib import Path
import shutil
import sys

fixtures = Path(__file__).resolve().parent.parent
root = Path(sys.argv[1]).resolve()
if root.exists():
    raise SystemExit("destination must not exist")
shutil.copytree(fixtures / "correction_history/workflow", root)
manifest = json.loads((fixtures / "gate_rooms/index.json").read_text())
for case in ["complete", "missing-report", "tampered-report", "missing-room", "tampered-room", "no-report", "blank-question", "unsupported"]:
    entity = root / case
    room = entity / "room"
    shutil.copytree(fixtures / "gate_rooms", room)
    data = json.loads(json.dumps(manifest))
    data["id"] = f"briefing:review:{case}:validation:attempt-1:revision-1"
    data["question"] = f"Review {case}: should this exact candidate proceed?"
    if case == "no-report":
        data["artifacts"][0]["id"] = "artifact:supporting-report"
    if case == "blank-question":
        data["question"] = " "
    if case == "unsupported":
        data["artifacts"][0]["uri"] = "https://example.invalid/report"
    # This fixture's JSON contains only ASCII keys/strings and integer version text.
    # Compact sorted-key JSON therefore equals RFC 8785 for these generated objects.
    canonical = json.dumps(data, sort_keys=True, separators=(",", ":")).encode()
    digest = "sha256:" + hashlib.sha256(canonical).hexdigest()
    (room / "index.json").write_bytes(canonical)
    if case == "missing-report":
        (room / "gate-review.md").unlink()
    if case == "tampered-report":
        (room / "gate-review.md").write_text("Changed bytes, not the recorded report.\n")
    if case == "missing-room":
        shutil.rmtree(room)
    if case == "tampered-room":
        (room / "index.json").write_text("{}")
    (entity / "index.md").write_text(f"""---
id: {case}
title: Review evidence {case}
status: validation
gates:
  version: 1
  records:
    - id: gate:{case}:validation
      stage: validation
      attempts:
        - id: attempt-1
          briefing:
            id: {data['id']}
            digest: {digest}
            room-ref: ./room
          resolution:
            type: Resolution
            id: resolution:{case}
            briefing: {data['id']}
            by: person:fixture-reviewer
            at: '2026-10-23T00:00:00Z'
            decision: approve
            reason: Exact recorded decision; not execution permission.
---
Read-only review evidence fixture.
""")
# Extend the upstream-derived history with a dangling edge; preserve the oracle round.
round2 = root / "sample/review/implementation/round-2/briefing.review.jsonl"
rows = [json.loads(line) for line in round2.read_text().splitlines()]
bad = dict(rows[3], id="annotation:dangling", includes=["annotation:absent-in-this-briefing"])
validation = root / "sample/review/validation/round-1/briefing.review.jsonl"
validation_rows = [json.loads(line) for line in validation.read_text().splitlines()]
bad["briefing"] = validation_rows[0]["briefing"]
with validation.open("a") as stream:
    stream.write(json.dumps(bad) + "\n")
(root / "_archive").mkdir(exist_ok=True)
(root / "_archive/archived.md").write_text("---\nid: archived\ntitle: Archived inventory sentinel\nstatus: validation\n---\nDo not mutate.\n")
(root / "untracked-sentinel.txt").write_text("Include untracked files in no-write proof.\n")
print(root)
