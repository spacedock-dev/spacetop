---
commissioned-by: spacedock@0.27.3
stages:
  states:
    - name: seed
      initial: true
      gate: true
    - name: review
      gate: true
    - name: validation
      gate: true
    - name: implement
    - name: done
      terminal: true
---
# Attention walkthrough fixture

Open this workflow with `--workflow-dir tests/fixtures/attention`, then use the
command palette's `needs-attention`. The `3k` task has a locally bound Briefing
v1 inventory and verified report bytes. `missing-room` has no room;
`missing-artifact` has a valid bound inventory but absent artifact bytes.

The other gate facts reuse the durable-gates fixtures; their opaque references
are deliberately unavailable, not verified evidence. Ordinary and old tasks
must remain excluded. The malformed row represents invalid task information.

For deterministic preparation proof, copy this directory to a disposable Git
repository and commit it: `preparation` has a completed review report, whereas
`unverified` has no report. Keep HOME/XDG config and state outside the workflow.
Do not change the committed fixture during a read-only inventory comparison.
