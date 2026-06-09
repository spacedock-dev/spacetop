---
commissioned-by: spacedock@0.19.8
entity-type: writing_doc
entity-label: doc
entity-label-plural: docs
id-style: slug
stages:
  defaults:
    worktree: false
  states:
    - name: brief
      initial: true
    - name: done
      terminal: true
---

# Slug Workflow Fixture

Minimal `id-style: slug` workflow used by the parser tests. Entities carry a
blank `id:` field; identity comes from the filename slug instead.

## Stages

### brief

Initial stage.

### done

Terminal stage.
