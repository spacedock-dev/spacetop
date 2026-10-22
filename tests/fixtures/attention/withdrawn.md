---
id: withdrawn
title: withdrawn
status: review
gates:
  version: 1
  records:
    - id: gate-review
      stage: review
      attempts:
        - id: attempt-one
          briefing:
            id: briefing-one
            digest: sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
            room-ref: opaque/room
          withdrawal: {by: agent:first-officer, at: '2026-10-01T00:00:00Z', reason: retry}
---

Fixture body.
