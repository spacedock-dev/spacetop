---
id: 002
title: Parse Spacedock Workflow Files
status: plan
source: commission seed
started: 2026-04-24T14:30:53Z
completed:
verdict:
score: 1.0
worktree:
issue:
pr:
---

Read Spacedock workflow `README.md` metadata and work item markdown frontmatter into a typed model that SpaceTop can use for status summaries and workflow structure views.

## Acceptance criteria

**AC-1 -- Workflow README frontmatter is parsed into typed stage metadata.**
Verified by: parser tests using `docs/spacetop-dev/README.md` or fixture copies assert stage names, initial state, terminal state, and review gate metadata.

**AC-2 -- Work item markdown frontmatter is parsed into typed task records.**
Verified by: parser tests assert IDs, titles, statuses, sources, scores, and body text for the seed task files.

**AC-3 -- Invalid or incomplete workflow files produce actionable errors.**
Verified by: tests cover missing frontmatter, unknown status, and malformed YAML with readable error messages.
