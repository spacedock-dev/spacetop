---
commissioned-by: spacedock@0.27.3
stages:
  states:
    - name: seed
      initial: true
      gate: true
    - name: review
      gate: true
    - name: implement
    - name: done
      terminal: true
---
# Durable gates fixtures

Adapted from Spacedock v0.27.3 (29da151096c1f2b7291a3adafa0f12a762bad78f),
internal/gates/model.go and gates_test.go. Synthetic IDs, digests, and opaque
room references exercise recorded frontmatter only. No retained room or session
is needed or authenticated. Invalid fixtures remain readable entities.
