---
id: "040"
title: Worktree-aware diff orientation and 'o' open target
status: design
source: captain
started:
completed:
verdict:
score:
worktree:
issue:
pr:
---

The preview-pane diff and the `o` (open in editor) action both need to treat the worktree copy as the active version of an entity when one exists.

Two related behaviors to align:

1. **Diff orientation in the preview body.** When an entity has a divergent worktree copy, the preview renders a unified diff between the main body and the worktree body. The worktree content should appear as the *added* side (`+` green). Verify the current call site in `src/ui/mod.rs` (around the `render_diff_lines(main, &item.body)` invocation) and confirm — or fix — the argument order so worktree content reads as additions and main content as removals.

2. **`o` opens the worktree copy when present.** Pressing `o` in the overview currently records `pending_open_file` from the item's path. When the entity has a `worktree_source` (i.e. there is a worktree copy on disk), `o` should open the worktree-resident markdown file in `$EDITOR` (nvim), not the main-branch copy. When no worktree copy exists, behavior is unchanged.

Both changes should be testable without a terminal backend: extend `src/ui/diff.rs` tests for orientation, and extend the `App` keymap / `pending_open_file` tests in `src/app.rs` to assert the chosen path when `worktree_source` is `Some(_)` vs `None`.

## Acceptance criteria

**AC-1 -- Preview diff shows worktree content as `+` and main content as `-`.**
Verified by: a unit test in `src/ui/diff.rs` (or the existing preview render tests in `src/ui/mod.rs`) that constructs a `WorkItem` with distinct `main_body` and `body`, renders the bottom preview, and asserts a line beginning with `+` carries text unique to the worktree body and a line beginning with `-` carries text unique to the main body.

**AC-2 -- Pressing `o` on an entity with a worktree copy queues the worktree-resident markdown path for $EDITOR.**
Verified by: a unit test against `App` (mirroring the existing `pending_open_file` tests) that sets `worktree_source = Some(<worktree path>)` on the selected item, sends the `o` key, and asserts `take_pending_open_file()` returns the worktree path — not the main-branch path. A second test with `worktree_source = None` asserts the main path is still returned.

**AC-3 -- `make lint` and `cargo test` pass.**
Verified by: `make lint` (clippy with `-D warnings`) and `cargo test` from the repo root, both green.
