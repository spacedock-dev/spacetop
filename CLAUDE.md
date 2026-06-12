# Spacetop - Claude Code

@AGENTS.md

## Claude Code

This file is a Claude Code adapter. `AGENTS.md` is the canonical source of repo
policy for Spacetop; keep shared safety, architecture, test, lint, and workflow
rules there.

Claude-specific notes may live below this section, but they must not override or
duplicate repo policy from `AGENTS.md`.

## Claude-Specific Tools

Claude Code skills or local helper tools may be useful, but they are not repo
policy. Use them only when they support the current request and do not conflict
with `AGENTS.md`.

If a user explicitly requests a gstack workflow, verify it before use:

```bash
test -d ~/.claude/skills/gstack/bin && echo "GSTACK_OK" || echo "GSTACK_MISSING"
```

If gstack is missing for a requested gstack workflow, stop and ask the user to
install it:

```bash
git clone --depth 1 https://github.com/garrytan/gstack.git ~/.claude/skills/gstack
cd ~/.claude/skills/gstack && ./setup --team
```

Do not use gstack availability as a general blocker for ordinary Spacetop work.
