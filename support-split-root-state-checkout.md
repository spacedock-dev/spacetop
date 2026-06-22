---
title: Support split-root workflows (read entities from the state checkout)
status: shape
source: captain report (session) — spacetop-dev migrated to split-root; task list renders empty
kind: feature
risk: medium
id: 073
---

Spacetop renders an empty task list for the spacetop-dev workflow after it was
migrated to a split-root state backend. The workflow README declares
`state: .spacedock-state`, and the entity files (active + `_archive/`) now live
in the state checkout (`docs/spacetop-dev/.spacedock-state/`), on the
`spacedock-state/spacetop-dev` orphan branch — NOT beside the README.

Spacetop's discovery/parser assumes single-root: it scans the workflow directory
for `*.md` entities, finds only `README.md` (entities were git-rm'd from the code
branch), and shows nothing. It does not read the README `state:` field, so it
never looks in the state checkout.

Shape should pin down:
- How spacetop currently resolves the entity directory vs the definition (README)
  directory, and where the single-root assumption is baked in (parser/discovery).
- The contract for the `state:` field: `.spacedock-state` (a relative path → state
  checkout dir), `$inline`/absent (single-root, entities beside README). Resolve
  the state checkout relative to the workflow/definition dir.
- Whether the worktree-merge scan (`.worktrees/*/<workflow>`) interacts with the
  state checkout, and how `_archive/` under the state checkout is loaded.
- Acceptance criteria including the concrete regression: a split-root workflow
  fixture must render its active + archived entities (today it renders empty).
