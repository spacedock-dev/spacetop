# Correction history compatibility and terminal fixture

`upstream/` contains unchanged files from Spacedock **v0.27.3**,
[`internal/gates/testdata/advisory-round`](https://github.com/spacedock-dev/spacedock/tree/v0.27.3/internal/gates/testdata/advisory-round).
The oracle Briefing's canonical digest is
`sha256:006c419cc260eac142a2a038037d4d8a8d80156f7b1f7224b3be6eab682a0698`.
Its unchanged `recorded-entity.md` is a compatibility oracle: entity `task` binds
Briefing `briefing:3j:implementation:round-1`. The parser must preserve that exact
identity rather than derive it from entity, stage or cycle. The browsing fixture
`workflow/sample/index.md` uses entity 3j with the same unchanged oracle room.
Core regressions also cover a verify room binding an implement Briefing, opaque
IDs, exact pointer mismatches and ambiguous unbound flat-entity rooms.
No installed Spacedock source or private path is required to run these tests.

`workflow/` is disposable test data. Implementation cycles 1, 2, 10 and validation
cycle 1 demonstrate stage/numeric order. The pointer deliberately remains at
implementation cycle 1. Cycle 2 repeats IDs/prose but removes the disposition's
includes and gives it an offset timestamp; that must not invent a response.
Cycle 10 has a partial final JSONL line after five complete records. The missing
entity has a pointer to a nonexistent room. The legacy entity has neither pointer
nor history. The five-record oracle chain is two findings -> reviewer revise ->
disposition Annotation -> closing revise; closure is not gate approval.

## Reproduce in a real terminal

From the product checkout, with Rust/Cargo, Git and a POSIX shell:

```sh
proof_root=$(mktemp -d)
cp -R tests/fixtures/correction_history/workflow "$proof_root/workflow"
mkdir -p "$proof_root/config" "$proof_root/state"
git -C "$proof_root/workflow" init -q
git -C "$proof_root/workflow" add .
git -C "$proof_root/workflow" -c user.name=Fixture -c user.email=fixture@example.invalid commit -qm fixture
export XDG_CONFIG_HOME="$proof_root/config"
export XDG_STATE_HOME="$proof_root/state"
printf 'Disposable fixture: %s\n' "$proof_root/workflow"
cargo run -p spacetop -- --workflow-dir "$proof_root/workflow"
```

Use a 150x50 terminal first. On Advisory history press `B`, `h`: CURRENT bound,
implementation cycle 1, reviewer Revise, Answered. Round `j`, `j` visits cycles 2
then 10, not lexical order; another `j` visits validation. `k` three times returns
to current cycle 1. `Tab`, Enter opens job-592; `j`, Enter follows the actual
incoming FollowUp to decline-duplicate-member. `j`, `j`, Enter follows its
incoming edge to the ensign Resolution. Esc twice returns to job-592, Esc once
to its list selection. No transitive shortcut or cross-round edge exists.

On cycle 2, job-592 has reviewer evidence but no recorded follow-up, despite
matching prose elsewhere. On cycle 10, inspect the explicit partial-line error
and five-record valid prefix. On the missing entity inspect CURRENT UNVERIFIED;
on legacy inspect No current pointer / no history. Resize to 40x12 and 20x6;
CURRENT/RETAINED/UNVERIFIED and Esc remain visible. Use PgDown/PgUp for detail.
Repeat with `SPACETOP_ASCII=1 cargo run -p spacetop -- --workflow-dir "$proof_root/workflow"`.

Before deliberately changing any fixture, close the app and verify no fixture
writes, including untracked files:

```sh
git -C "$proof_root/workflow" diff --exit-code
git -C "$proof_root/workflow" status --porcelain --untracked-files=all
```

In a second terminal, modify **only the disposable copy** while browsing:
append `{"type":"Annotation"` without newline to a complete log; observe the
explicit partial-tail error. Finish it with a unique ID, the same Briefing ID,
actor, RFC3339 time and newline; observe recovery to an incomplete Annotation
ending. Atomically rename a replacement log, rename/delete a selected retained
round, and add a new correctly bound round. A removed selection must not open an
adjacent record. Open artifact preview, modify/delete candidate.patch and observe
preview invalidation; missing artifact bytes must not erase the reviewer verdict.
Capture raw terminal output, actions, sizes, exit status and the before/after
file inventory in local operational evidence, not in this product fixture.

Automated proofs: core `tests/correction_history.rs`, app correction-history
unit tests, UI gate-room TestBackend tests, existing gate-room safety tests, and
ignored `watcher_fs` real-notify smoke tests. `cargo test`, `cargo test -p
spacetop-core -- --ignored`, and `make lint` cover the final verification gates.
