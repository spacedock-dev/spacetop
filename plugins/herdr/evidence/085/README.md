# Task 085 evidence

`live-proof.json` contains the final real Herdr 0.9.3 / Codex 0.160.0 scenario's
raw command results. The client received actual Ctrl+B,v bytes (`0276`) through
a PTY. No model prompt was sent. Isolated HOME, XDG config/state, Codex home and
named session were used; the temporary auth copy was removed and the owned
session stopped. No captain layout/config was changed.

- AC-1: `first-input`, `agent-start`, `agent-screen`, `first-layout` and
  `first-process`: agent w1:p1 remains focused; w1:p2 is at x=77 to its right.
- AC-2: `first-process` retains the single argv path with spaces/metacharacters;
  `first-screen` shows two workflows and the INVOKING-WORKTREE marker.
- AC-3: `reuse-panes/layout` retain two panes and focus w1:p2; `close` and
  `reopen-panes` replace it with w1:p3; workspace two gets w2:p2 while w1:p3
  remains in workspace one.
- AC-4: `before`/`after` include Markdown hashes, Git HEAD/index hash/status;
  `readonly_equal=true`, `crafted_path_executed=false`. No sync key was used.
- AC-5: `initial-plugins` starts empty, `link` uses the final manifest, all final
  action log entries exit zero, `unlink` and `stop` show cleanup. Config syntax
  is retained. Focused tests exercise actionable failures.
- AC-6: test/check output is in `checks.txt`. Independent acceptance review is
  owed by the workflow's verify stage; implementation evidence does not replace
  that review.

`plan-jev-request.json` and `plan-jev-response.json` retain the FO-owned plan
judgment supplied to the implementation worker. No extra JEV call was made.

Reproduce on supported macOS with Git, Python 3, Herdr and Codex on PATH:

```sh
cargo build -p spacetop --bins
python3 plugins/herdr/evidence/085/live_scenario.py \
  --codex-auth /absolute/path/to/your/auth.json
```

The optional auth argument copies only that file into the temporary agent home
and removes the copy in cleanup. Never add credentials to this evidence folder.
No prompts are sent to Codex. `--bin-dir /absolute/installed/bin` can exercise
installed binaries. `--root /absolute/fresh/tmp/path` keeps a chosen artifact
home; reusing a populated root is rejected. This runner controls only its own
named session, uses a clean non-login zsh, registers/removes the plugin in its
isolated registry, and stops its own session. Socket/PTY access may require
execution outside a restrictive sandbox. Product use needs neither Python nor
this runner.

Two host details learned from exercising the APIs: split/right accepts
`--target-pane` and rejects a simultaneous `--workspace`; `pane report-metadata`
returns empty stdout on success. Tests and the final helper follow both facts.
Display tokens truncate long values, so reuse checks full pane cwd rather than
storing a project path in a token. An early failed scenario also caught malformed test fixture YAML; the retained
final fixtures use `stages.states` and the TUI showed their actual content.
