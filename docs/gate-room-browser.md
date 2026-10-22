# Gate room browser

`B` opens the selected entity's recorded gate attempts. It defaults to the
current stage's selected attempt, or the last recorded attempt when the current
stage has no record. History is keyed by gate and attempt ids; a reload retains
the selected identity and focus, and reports when the identity disappears.
The selected question is first in the content, including item previews. Missing
or unverified question data is explicit. `r` opens the canonical main report:
the Artifact with exact ID `artifact:gate-review`. A Reference with that ID, a
filename or the first inventory position cannot designate it. No designation
says “Main report not identified”; inventory remains browseable with Tab/Enter.
The shortcut preserves attempt, focus and supporting-item selection on Back.
Question/inventory bytes load at open or attempt selection. Item bytes load only
on Enter/r and reverify on preview refresh by the opened identity, not list
position. File content never comes from the render loop.

Format precedence follows the bound attempt rather than nearby files:

1. A request digest requires a matching request and its clean relative locator.
2. A request-less directory with briefing.json uses that retained Briefing.
3. Other directories use index.json; gate-briefing.json is a fallback only when
   index.json is absent. An unreadable or invalid reserved file fails closed.
4. A regular-file binding reads exactly that file.

Every format requires Briefing v1, its bound id and JCS SHA-256, a nonblank
question, at least one Artifact, and unique complete inventory bindings.
References in nested context children retain source order. Reference summaries
are invalid. Artifact summaries are shown verbatim. Stage identities are
checked against the historical gate record, independently of current status.
The identity prefix is nonempty and excludes newline, matching the upstream
identity grammar; attempt/revision ordinals use ASCII
`[1-9][0-9]*` with no numeric upper bound. Absent or null context and nested
children are empty inventories; other non-array values are invalid.

Recorded decisions are frontmatter facts. A valid room binding authenticates
neither the recorded actor nor a conn quote. Included annotations in gate
attempts remain recorded ids; `h` opens the separate correction-round log browser.

The task preview explains gate readiness in plain language alongside the
canonical label and recorded details. Waiting for your decision is different
from approval awaiting merge. `B` only inspects evidence; it does not approve
requests or authenticate their recorded actors. Make decisions in your
Spacedock workflow session. Spacetop does not know the action destination from a
room reference, actor string or recorded target stage; ask your workflow
coordinator when the destination is unknown.

Safe local evidence roots are the workflow's canonical entity root, including
archives, contained within its canonical definition root. No root is inferred
from worktrees, HOME, the current directory, events or URI text. Symlink
components, lexical traversal, encoded local paths and nonregular files fail
closed. Reads use descriptor-relative NOFOLLOW opens and post-read identity
checks. Unix platforms support this boundary; other platforms return a typed
diagnostic. Per-document/item size is capped at 2 MiB, JSON recursion is bounded,
and context traversal is capped at 64 levels. Oversized/truncated content is not
presented as verified evidence.

Git-root links use only verified local workflow history. They require full
commit ids, exact canonical URI encoding, regular blob mode, and raw SHA-256
revisions. A linked/detached checkout can provide history without sync authority.
A distinct state checkout is required for state links. Missing objects stay
missing; no lazy fetch, checkout repair, ref retention or current-file fallback
is permitted. Unsupported links are visible but have no launcher action.

The watcher registers contained state roots and keeps selected, safely resolved
local inventory coordinates for arbitrary artifact filenames. Events only
invalidate; they never authorize reading their paths. Canonical room changes,
entity rebinding, artifact changes/removal and state directory replacement cause
reverification. An open item is cleared on failure. Failed Git objects can be
retried via Enter without fetching.

Failures explain both cause and effect, followed by the exact typed diagnostic.
Missing files, malformed/mismatched Briefings, unsafe paths, size limits,
digest mismatches and absent exact Git objects never produce a verified preview.
A bound Briefing is not a claim that all its items were read or verified.
Verified non-UTF-8 bytes cannot be shown as a text report. Recorded frontmatter
decisions remain inspectable independently, including when the room is rejected.

Reproducible proof lives in `spacetop-core/tests/gate_rooms.rs`, core decoder and
safe-I/O tests, app gate-room tests and Ratatui TestBackend tests. The unchanged
Spacedock v0.27.3 fixture pins JCS interoperability. Run `cargo test`, `make lint`
and `cargo test -- --ignored` for real notify refresh coverage. Test repositories
are temporary and do not require user configuration or upstream checkout paths.

## Correction-round history

Inside `B`, `h` toggles gates/history, including entities without gate attempts.
History is an owned bounded core snapshot, loaded at browser open and refresh,
never during drawing. `Tab` switches rounds/records; arrows or `j/k` select.
`Enter` opens record detail, then follows the selected outgoing or incoming
includes edge. `Esc`/`q` returns one record at a time to the source selection,
then closes the browser. Artifact entries use Enter for digest-verified preview;
label-only context References remain visible and non-openable. Paging and `w`
work in both modes. The browser has no mutation or approval controls.

Stages follow workflow definition order, unknown stages lexically afterward,
and cycles sort numerically. Records retain physical JSONL order, not timestamp
order. A valid current pointer is selected even if newer rounds exist; an absent
or invalid pointer never marks a retained round current. Missing pointer targets
remain visible UNVERIFIED rows. Main/state evidence origin remains separate from
a displayed worktree body, and archived entities use their physical archive parent.

Current rooms require the pointer's exact Briefing ID and JCS digest. Briefing
IDs are opaque recorded identities, not derived from the entity, room stage or
cycle. Folder-form retained rooms take the manifest ID within their owning
entity's origin/stage/cycle namespace without inventing a historical pointer
digest. Flat entities share a sibling review directory: only an exact current
pointer binding attributes a room; other candidates report ambiguous ownership
and expose no records. Discovery visits only immediate review/stage/round-N
entries, capped at 4096 entries and 128 rounds; truncation is a visible diagnostic. Positive cycles
are normalized u32 values. Each file remains limited to 2 MiB. Round artifacts
may use `..` only while remaining within the entity parent (the real upstream
fixture uses `../../../candidate.patch`); absolute paths, symlinks, encoded paths,
mutable `index.md` and nonregular files remain forbidden. Gate URI rules are
unchanged. Exact local Git links share the gate reader and never fetch.

Review JSONL requires complete newline-terminated UTF-8 records, unique IDs,
nonempty actors, RFC3339 timestamps (offsets supported), and same-Briefing
binding. A malformed line stops decoding: the valid prefix stays visible, but
later lines and partial tails cannot establish closure or relationships.
Annotation body/target/kind and Resolution reason/includes are shown verbatim.
Missing bodies are explicit. The first Resolution is the reviewer verdict.
Answered means a later ending Resolution, or a first/last approve Resolution;
it is neither gate approval nor proof that every finding received a response.

Relationships expose full source/target identity, recorded includes direction,
ordinal, validation and reviewer-evidence/follow-up role. They resolve only to
earlier records in the same entity-origin/stage/cycle/Briefing log. Inverse edges
allow finding -> disposition Annotation -> closing Resolution navigation with
each intermediate record intact. No cross-round edge is inferred from matching
IDs, prose, actors or timestamps. Dangling/forward edges are invalid and cannot
be opened. In record detail, `f` filters to recorded responses (incoming FollowUp
edges); press it again for all evidence relationships. Enter follows the selected
edge; Back restores the source, filter and exact edge, not the first nearby row.
No recorded response means unresolved in this log, not proof of an unfixed
correction. Invalid/partial evidence instead reports response unknown, while
retaining any known prefix responses. A response never implies fixed/approved.
A reviewer including a finding is not a follow-up response.

Refresh watches review JSONL and directory creation/rename/deletion as well as
Briefings and safe artifact coordinates. It retains surviving full identities,
including a relationship's source, target and direction rather than its list
position. A removed relationship clears selection with an explicit notice;
Enter cannot open a replacement until you select another relationship with
j/k or the arrows. Refresh clears artifact previews, revalidates record details,
and reports removed selections rather than opening an adjacent row. Explicit
opens revalidate both Briefing digest and selected record/item.
Artifact previews retain a fixed header with current/retained provenance,
stage/cycle and read-only status separately from item SHA-256 verification.
These labels remain visible while scrolling or toggling wrap on narrow
terminals and use ASCII-compatible text.

The question remains first in round detail. Full entity-origin, stage, cycle and
Briefing provenance accompany records; current/retained preview headers stay
separate from item verification. The reusable
`tests/fixtures/review-evidence/setup.py` combines complete/missing/tampered report
scenarios with the upstream-derived multi-round history. Its README describes
navigation and the full-file no-write inventory procedure.


## Finding work that needs attention

Open the command palette (`:` by default) and select `needs-attention`.
The focused list uses recorded gate facts and exact verification diagnostics,
not runtime silence, timestamps or guessed priority. A hold is a recorded blocker;
requested changes, unprepared requests, pending application of an approval and
missing or unverified information have different explanations.

Use Enter for the task's report/details and `B` for the selected gate attempt.
Tab switches to inventory; Up/Down or j/k selects evidence; Enter reads verified
bytes. Esc returns to the browser and then the same focused task. Missing rooms,
artifacts, unsupported references or invalid bindings stay explicit failures.
They never remove a known request or fall back to unverified report bytes.
Historical fallback is labeled historical, not current evidence.

`all-tasks` in the palette restores the ordinary active list. `a` exits focus
and opens archives. Broken rows expose their parse diagnostic, not another
task's gate browser. Decisions still happen in your Spacedock workflow session.

The `tests/fixtures/attention` workflow includes a correctly bound local report,
missing-room and missing-artifact cases, distinct recorded reasons, negative
controls and a malformed task. Copy it into a disposable Git repository and
commit it for deterministic preparation proof. Keep config/session roots
outside the fixture; inventory every workflow file before and after an
inspection-only walkthrough, including untracked and archive files. Do not
include deliberate fixture mutation or the explicit `Y` sync action in that run.
