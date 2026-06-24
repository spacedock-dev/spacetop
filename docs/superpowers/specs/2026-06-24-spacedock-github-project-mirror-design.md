# Spacedock GitHub Project Mirror Design

## Goal

Mirror Spacedock split-root workflow entities into GitHub Projects so remote
readers can see workflow status without checking out state branches manually.
Spacedock markdown remains the single source of truth.

## Scope

- Trigger on pushes to `spacedock-state/**`.
- Allow manual full sync with `workflow_dispatch` and a `workflow_id` input.
- Map `spacedock-state/<workflow-id>` to a workflow README whose frontmatter has
  `id: <workflow-id>`.
- Read the target Project from that README frontmatter:
  `github-project.owner` and `github-project.number`.
- Process only markdown entity files changed in the pushed commit range.
- For manual runs, process all tracked markdown entity files on
  `spacedock-state/<workflow-id>`.
- Upsert GitHub Project draft items by `Entity ID = <workflow-id>:<entity-id>`.
- Mirror fields: `Status`, `Kind`, `Score`, `Source`, `PR`, `Updated At`, and
  `Archived`.
- Treat files under `_archive/` as `Status = Done` and `Archived = true`.

## Non-Goals

- No GitHub Issues.
- No two-way sync from GitHub Project edits back to markdown.
- No full Project rebuild on every push.
- No cache committed to state branches.
- No Project creation. Missing mirror-owned fields are created automatically;
  existing fields are still type-checked.

## Operational Contract

The GitHub Project must already exist. The Action creates missing mirror-owned
fields:

- `Entity ID`
- `Kind`
- `Score`
- `Source`
- `PR`
- `Updated At`
- `Archived`

The Project may contain `Status`, which usually exists on GitHub Projects by
default. If `Status` is a single-select field and lacks a workflow stage option,
the Action skips that field instead of failing the whole mirror. Other existing
single-select fields still fail loudly when they lack a needed value.

The Action uses `SPACEDOCK_PROJECT_TOKEN`. The built-in `GITHUB_TOKEN` is
repository-scoped and does not expose a Project V2 write permission, so it
should not be used for this mirror.

## Data Flow

```text
push to spacedock-state/<workflow-id>
  -> checkout state branch
  -> checkout default branch
  -> find README with id: <workflow-id>
  -> read github-project mapping
  -> diff before..after for changed markdown files
  -> parse entity frontmatter
  -> find Project item by Entity ID
  -> create or update draft item and fields
```

```text
manual workflow_dispatch with workflow_id
  -> checkout spacedock-state/<workflow-id>
  -> checkout default branch
  -> process every tracked markdown entity file
  -> create or update draft item and fields
```

Deleted files are ignored. Archived entities should be represented by a move or
add under `_archive/`, which gives the Action current markdown content to parse.
