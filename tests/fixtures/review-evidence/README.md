# Review evidence scenarios

`setup.py` constructs a disposable workflow from the committed `gate_rooms` and
`correction_history/workflow` compatibility fixtures. It needs Python 3, not an
upstream checkout or network. The upstream oracle files remain unchanged in their
source directories. Generated gate objects use ASCII-only JSON whose sorted,
compact encoding matches JCS; the bound SHA-256 is computed from those bytes.

```sh
proof_root=$(mktemp -d)
proof_root=$(cd "$proof_root" && pwd -P)
python3 tests/fixtures/review-evidence/setup.py "$proof_root/workflow"
git -C "$proof_root/workflow" init -q
git -C "$proof_root/workflow" add .
git -C "$proof_root/workflow" -c user.name=Fixture -c user.email=fixture@example.invalid commit -qm fixture
export XDG_CONFIG_HOME="$proof_root/config"
export XDG_STATE_HOME="$proof_root/state"
cargo run -p spacetop -- --workflow-dir "$proof_root/workflow"
```

Use the canonical path above: a symlinked temporary-root component (for example
macOS `/var` rather than `/private/var`) is deliberately refused by safe readers.

Select an entity with arrows, `B` opens its browser, `r` opens its canonical
report without changing the supporting-item selection. `Esc`, `Tab`, arrows,
`Enter` open supporting evidence. `PgUp/PgDn` scroll; `w` toggles wrapping.

| Entity | Expected question/report result |
| --- | --- |
| complete | Question first; r opens Exact candidate review; Back retains context |
| missing-report | Bound question; missing-file explanation; recorded Approve remains |
| tampered-report | Bound question; digest mismatch; no verified preview |
| missing-room | Explicit unavailable question/inventory; recorded Approve remains |
| tampered-room | Rejected Briefing; explicit unavailable question and diagnostic detail |
| no-report | Main report not identified; supporting inventory remains available |
| blank-question | Strict schema failure; no invented question |
| unsupported | Display-only URL; no fetch or launcher |

On Advisory history press `B`, `h`, `Tab`, Enter, `f`: only the typed incoming
FollowUp responses remain. Enter follows the disposition; Enter follows its
closing Resolution. Neither means fixed or approved. Esc twice restores the
source and selected edge. The current implementation cycle 1 contains two
findings with an explicit response. Cycle 2 repeats prose and IDs but has no
response edge: it must say no recorded response, not link another round.
Cycle 10 has a partial tail: known prefix links survive but additional responses
are unknown. Validation cycle 1 has a dangling includes target shown as invalid,
not replaced by a similarly named record. Current/retained labels come from the
pointer, not cycle order. The round artifact uses the upstream contained
`../../../candidate.patch` URI and the correction reader, not the gate reader.

Exercise complete and failure cases at 80x24, 40x12 and 20x6 (and Unicode via the
Ratatui regression). Keep operational screen captures and logs outside product
source. Inventory every workflow file and byte digest before/after inspection,
excluding only `.git`, including archived and untracked files. Do not press `Y`.
Run deliberate report replacement/refresh in a separate pass: the old preview
must disappear, and Enter/r must show the digest failure. Exact Git links and
escaping paths are covered by the core gate-room/correction-history safety tests;
missing objects must never fetch or use dirty current files.
