# TODOS

## Preview scroll: Ctrl-modifier keys (half-page + single-line)

**What:** Add `Ctrl-d`/`Ctrl-u` (half-page down/up) and optionally `Ctrl-e`/`Ctrl-y`
(single-line down/up) to the preview-pane scroll vocabulary.

**Why:** Half-page jumps are vim/less muscle memory. The first scroll PR ships the
modifier-free set (`Space`/`b`, `PgUp`/`PgDn`, `g`/`G`) to avoid touching key-modifier
handling; these are the deliberate fast-follow.

**Pros:** Completes the `less`/vim keymap; the scroll methods already exist
(`scroll_preview_vertical(delta)` + `half_*` wrappers), so the keys themselves are
trivial.

**Cons:** Requires teaching `handle_overview_key` to inspect `key.modifiers`
(`src/app/keys.rs:25` matches `key.code` only today). That adds a small modifier-match
surface with its own edge cases.

**Context:** Use exact-equality matching (`key.modifiers == KeyModifiers::CONTROL`),
not `.contains(CONTROL)`, so `Ctrl+Shift+d` does not alias. Place the Ctrl arms before
any plain `Char(_)` arm so the modified form is not shadowed. **`handle_overview_key`
is currently modifier-blind** (it matches on `key.code` only), and the existing
preview-open `Char('b')` arm pages up — so when `Ctrl-b` half-page lands it WILL be
swallowed by the plain `'b'` arm unless that arm is tightened in the same change to
reject CONTROL/ALT (keep NONE and SHIFT so `Shift+g`→`G` still works). Do the same for
`Space`/`g`/`G`. Add a test that `Ctrl+Shift+d` does NOT trigger half-page and that
`Ctrl+b` reaches the half-page arm, not the page arm. `Ctrl-e`/`Ctrl-y` are lowest
value (`↑`/`↓` are task-nav, not free) — include only if the single-line gap is felt.

**Depends on:** the preview keyboard-scroll PR landing first (the helper + wrappers).
