---
id: feedback
title: feedback
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
          resolution: {type: Resolution, id: resolution-one, briefing: briefing-one, by: person:captain, at: now, decision: revise, reason: retry}
---

Fixture body.
