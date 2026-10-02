# Gate room browser

`B` opens the selected entity's recorded gate attempts. It defaults to the
current stage's selected attempt, or the last recorded attempt when the current
stage has no record. History is keyed by gate and attempt ids; a reload retains
the selected identity and focus, and reports when the identity disappears.
Question/inventory bytes are loaded at open or attempt selection. Artifact and
Reference bytes are loaded only by Enter in inventory focus, then reverified
when an open preview refreshes. File content never comes from the render loop.

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
neither the recorded actor nor a conn quote. Included annotations remain
recorded ids; correction-round log resolution is outside this browser.

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

Reproducible proof lives in `spacetop-core/tests/gate_rooms.rs`, core decoder and
safe-I/O tests, app gate-room tests and Ratatui TestBackend tests. The unchanged
Spacedock v0.27.3 fixture pins JCS interoperability. Run `cargo test`, `make lint`
and `cargo test -- --ignored` for real notify refresh coverage. Test repositories
are temporary and do not require user configuration or upstream checkout paths.
