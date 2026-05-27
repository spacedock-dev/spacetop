---
id: 047
title: Color the sync result pill so the Y-sync outcome is readable at a glance
status: plan
source: captain
started: 2026-05-27T06:56:37Z
completed:
verdict:
score:
worktree:
issue:
pr:
---

When the user presses `Y` to sync (`git pull --ff-only`), the result already
renders as a pill in the status footer (`Synced (already up to date)`,
`Synced (N new commits)`, `⚠ Sync failed: …`, `Sync unavailable: …`). The
captain reports the result is easy to miss: a **successful** sync pill is
styled identically to the neutral key hints beside it (white text on the same
dark pill background), so it reads as just another hint rather than the
outcome of their action. Only the failure case is visually distinct (red).

The captain wants the sync result to be obvious at a glance — at minimum,
whether it succeeded or not.

## Captain-approved direction

Keep the result in the footer, but color the sync pill by outcome and make
success distinguishable without relying on color alone:

- `Succeeded` → green, with a leading `✓ ` glyph (mirrors the existing `⚠ ` on failure)
- `Unavailable` → yellow
- `Failed` → red (unchanged)
- `InFlight` ("Syncing…") → cyan, so it reads as in-progress rather than done

Derive the pill color from the `SyncStatus` value, replacing the current
fragile `starts_with('⚠')` string-sniffing in the footer renderer. The key
hints stay neutral gray. Scope is footer-only: no changes to the sync logic
(`src/git_sync.rs`), the `Y` key path, or the in-flight→result flow.

## Acceptance criteria

- **AC-1** A `Succeeded` sync pill renders green and its label begins with `✓ ` (e.g. `✓ Synced (already up to date)`, `✓ Synced (3 new commits)`).
- **AC-2** A `Failed` sync pill renders red and keeps the `⚠ Sync failed: {message}` label.
- **AC-3** An `Unavailable` sync pill renders yellow with the `Sync unavailable: {hint}` label.
- **AC-4** An `InFlight` sync pill renders cyan with the `Syncing…` label.
- **AC-5** The pill color is derived from the `SyncStatus` variant, not by inspecting the label string; the neutral key hints remain unchanged in style.
- **AC-6** The pinned sync-pill label strings (and their tests) are updated together for the new `✓ ` success prefix; all other labels are unchanged.
- **AC-7** A render test asserts the success pill's foreground is green and the failed pill's foreground is red.
- **AC-8** `make lint` is clean and the full test suite (`cargo test`) passes; `src/git_sync.rs` behavior is untouched.

## Out of scope

`Y` only triggers a sync when the preview pane is closed. Making `Y` work with
the preview open is a separate concern and is not part of this task.
