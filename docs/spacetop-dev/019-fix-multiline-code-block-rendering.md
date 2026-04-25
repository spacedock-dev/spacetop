---
id: "019"
title: Fix multi-line code block rendering in preview pane
status: design
source: captain (bug report — follows task 016)
started:
completed:
verdict:
score:
worktree:
issue:
pr:
---

Task 016 implemented fenced code block rendering in the preview pane using `pulldown-cmark`. Multi-line code blocks do not render correctly — the current `Text` event handling inside a `CodeBlock` emits each line as a separate `Line`, but the pulldown-cmark parser delivers the entire code block body as a single `Text` event containing embedded newlines. The result is that multi-line code blocks appear as one long collapsed line rather than multiple properly-separated lines.

## Example content (use this to verify the fix)

### Multi-line Rust block

```rust
fn main() {
    let x = 42;
    println!("hello {}", x);
}
```

### Multi-line shell block

```bash
cargo build --release
cargo test --lib
cargo run -- --workflow-dir docs/spacetop-dev
```

### Single-line block (must still work after the fix)

```rust
let x = 42;
```

## Acceptance criteria

**AC-1 -- Multi-line fenced code blocks render each source line as a distinct styled line.**
The example Rust block above (4 lines) renders as 4 separate lines in the preview pane, each with the code block style (`fg(Cyan).bg(DarkGray)`). No lines are merged or collapsed.
Verified by: unit test asserting the rendered `Vec<Line>` count matches the number of source lines in the block.

**AC-2 -- Single-line code blocks are unaffected.**
A fenced block with a single line of content still renders as exactly one styled line.
Verified by: existing test `preview_renders_fenced_code_block_without_backtick_fences` continues to pass.

**AC-3 -- Backtick fences are not visible in any code block.**
Neither opening nor closing fence lines appear in the rendered output.
Verified by: assert no line contains ` ``` ` in the rendered output for all blocks above.

**AC-4 -- Non-code markdown content is unaffected.**
Prose, headings, inline code, and lists continue to render correctly.
Verified by: full `cargo test --lib` suite passes with no regressions.
