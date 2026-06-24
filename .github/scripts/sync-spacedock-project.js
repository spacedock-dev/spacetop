const assert = require("assert");
const fs = require("fs");
const path = require("path");
const { execFileSync } = require("child_process");

const STATE_PREFIX = "spacedock-state/";
const REQUIRED_FIELDS = [
  "Entity ID",
  "Status",
  "Kind",
  "Score",
  "Source",
  "PR",
  "Updated At",
  "Archived",
];
const FIELD_TYPES = {
  "Entity ID": "TEXT",
  Kind: "TEXT",
  Score: "NUMBER",
  Source: "TEXT",
  PR: "TEXT",
  "Updated At": "DATE",
  Archived: "TEXT",
};

module.exports = async function sync({ github, context, core }) {
  const workspace = process.env.GITHUB_WORKSPACE || process.cwd();
  const stateDir = path.join(workspace, "state");
  const definitionDir = path.join(workspace, "definition");
  const branch = stateBranch(context);
  const workflowId = workflowIdFromBranch(branch);
  const definition = findWorkflowDefinition(definitionDir, workflowId);
  const projectConfig = definition.frontmatter["github-project"];
  if (!projectConfig || !projectConfig.owner || !projectConfig.number) {
    throw new Error(`workflow ${workflowId} is missing github-project.owner/number`);
  }
  const projectNumber = Number(projectConfig.number);
  if (!Number.isInteger(projectNumber) || projectNumber < 1) {
    throw new Error(`workflow ${workflowId} has invalid github-project.number: ${projectConfig.number}`);
  }

  const changedFiles = filesToSync(stateDir, context);
  if (changedFiles.length === 0) {
    core.info("No changed markdown entity files.");
    return;
  }

  const project = await ensureProjectFields(
    github,
    await loadProject(github, projectConfig.owner, projectNumber),
    core
  );
  requireFields(project.fields);

  for (const file of changedFiles) {
    const fullPath = path.join(stateDir, file);
    if (!fs.existsSync(fullPath) || fs.statSync(fullPath).isDirectory()) {
      core.info(`Skipping deleted or directory path: ${file}`);
      continue;
    }
    const entity = parseMarkdownEntity(fullPath);
    if (!entity.frontmatter.id) {
      core.info(`Skipping markdown without entity id: ${file}`);
      continue;
    }
    const archived = file.split(path.sep).includes("_archive") || file.split("/").includes("_archive");
    const values = entityProjectValues(workflowId, file, entity, archived, context.payload.head_commit);
    await upsertProjectItem(github, project, values);
    core.info(`Synced ${values.entityId}`);
  }
};

function stateBranch(context) {
  if (context.eventName === "workflow_dispatch") {
    return `${STATE_PREFIX}${context.payload.inputs.workflow_id}`;
  }
  return context.ref.replace("refs/heads/", "");
}

function workflowIdFromBranch(branch) {
  if (!branch.startsWith(STATE_PREFIX)) {
    throw new Error(`expected branch ${STATE_PREFIX}<workflow-id>, got ${branch}`);
  }
  const id = branch.slice(STATE_PREFIX.length);
  if (!id || id.includes("/")) {
    throw new Error(`expected exactly one workflow id after ${STATE_PREFIX}, got ${branch}`);
  }
  return id;
}

function findWorkflowDefinition(root, workflowId) {
  const readmes = [];
  walk(root, (file) => {
    if (path.basename(file) === "README.md") readmes.push(file);
  });
  for (const readme of readmes) {
    const parsed = parseMarkdownEntity(readme);
    if (parsed.frontmatter.id === workflowId) {
      return { path: readme, frontmatter: parsed.frontmatter };
    }
  }
  throw new Error(`no workflow README found with id: ${workflowId}`);
}

function filesToSync(cwd, context) {
  if (context.eventName === "workflow_dispatch") {
    return trackedMarkdownFiles(cwd);
  }
  return changedMarkdownFiles(cwd, context.payload.before, context.sha);
}

function trackedMarkdownFiles(cwd) {
  return git(cwd, ["ls-files", "*.md"])
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean)
    .filter((file) => !path.basename(file).startsWith("."));
}

function changedMarkdownFiles(cwd, before, after) {
  const zero = /^0+$/.test(before || "");
  const output = zero
    ? trackedMarkdownFiles(cwd).join("\n")
    : git(cwd, ["diff", "--name-only", "--diff-filter=ACMR", `${before}..${after}`, "--", "*.md"]);
  return output
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean)
    .filter((file) => !path.basename(file).startsWith("."));
}

function git(cwd, args) {
  return execFileSync("git", args, { cwd, encoding: "utf8" });
}

function parseMarkdownEntity(file) {
  const text = fs.readFileSync(file, "utf8");
  const match = text.match(/^---\r?\n([\s\S]*?)\r?\n---\r?\n?([\s\S]*)$/);
  if (!match) return { frontmatter: {}, body: text };
  return { frontmatter: parseSimpleYaml(match[1]), body: match[2].trim() };
}

function parseSimpleYaml(text) {
  const root = {};
  const stack = [{ indent: -1, value: root }];
  for (const rawLine of text.split(/\r?\n/)) {
    if (!rawLine.trim() || rawLine.trimStart().startsWith("#")) continue;
    const indent = rawLine.match(/^\s*/)[0].length;
    const trimmed = rawLine.trim();
    if (trimmed.startsWith("- ")) continue;
    const colon = trimmed.indexOf(":");
    if (colon === -1) continue;
    const key = trimmed.slice(0, colon).trim();
    const rawValue = trimmed.slice(colon + 1).trim();
    while (stack.length > 1 && indent <= stack[stack.length - 1].indent) stack.pop();
    const parent = stack[stack.length - 1].value;
    if (rawValue === "") {
      parent[key] = {};
      stack.push({ indent, value: parent[key] });
    } else {
      parent[key] = parseScalar(rawValue);
    }
  }
  return root;
}

function parseScalar(value) {
  const unquoted = value.replace(/^['"]|['"]$/g, "");
  if (unquoted === "true") return true;
  if (unquoted === "false") return false;
  if (/^-?\d+(\.\d+)?$/.test(unquoted)) return Number(unquoted);
  return unquoted;
}

function walk(root, visit) {
  for (const entry of fs.readdirSync(root, { withFileTypes: true })) {
    const file = path.join(root, entry.name);
    if (entry.isDirectory()) {
      if ([".git", "node_modules", "target"].includes(entry.name)) continue;
      walk(file, visit);
    } else {
      visit(file);
    }
  }
}

function entityProjectValues(workflowId, file, entity, archived, commit) {
  const frontmatter = entity.frontmatter;
  const status = archived ? "Done" : String(frontmatter.status || "");
  return {
    entityId: `${workflowId}:${frontmatter.id}`,
    title: String(frontmatter.title || frontmatter.id),
    body: draftBody(workflowId, file, entity),
    status,
    kind: String(frontmatter.kind || ""),
    score: frontmatter.score === undefined || frontmatter.score === "" ? null : Number(frontmatter.score),
    source: String(frontmatter.source || ""),
    pr: String(frontmatter.pr || ""),
    updatedAt: commit && commit.timestamp ? commit.timestamp : new Date().toISOString(),
    archived,
  };
}

function draftBody(workflowId, file, entity) {
  return [
    `Mirrored from Spacedock entity \`${workflowId}:${entity.frontmatter.id}\`.`,
    "",
    `State path: \`${file}\``,
    "",
    entity.body,
  ].join("\n");
}

async function loadProject(github, owner, number) {
  const projectQuery = `
    query($owner: String!, $number: Int!, $after: String) {
      organization(login: $owner) { projectV2(number: $number) { ...ProjectParts } }
    }
    fragment ProjectParts on ProjectV2 {
      id
      fields(first: 100) {
        nodes {
          ... on ProjectV2Field { id name dataType }
          ... on ProjectV2SingleSelectField { id name dataType options { id name } }
        }
      }
      items(first: 100, after: $after) {
        pageInfo { hasNextPage endCursor }
        nodes {
          id
          content { ... on DraftIssue { id title body } }
          fieldValues(first: 100) {
            nodes {
              ... on ProjectV2ItemFieldTextValue { text field { ...FieldName } }
              ... on ProjectV2ItemFieldNumberValue { number field { ...FieldName } }
              ... on ProjectV2ItemFieldDateValue { date field { ...FieldName } }
              ... on ProjectV2ItemFieldSingleSelectValue { name field { ...FieldName } }
            }
          }
        }
      }
    }
    fragment FieldName on ProjectV2FieldConfiguration {
      ... on ProjectV2Field { id name }
      ... on ProjectV2SingleSelectField { id name }
    }
  `;
  let after = null;
  let project = null;
  const items = [];
  do {
    const data = await github.graphql(projectQuery, { owner, number, after });
    project = data.organization?.projectV2;
    if (!project) throw new Error(`GitHub Project not found: ${owner}/${number}`);
    items.push(...project.items.nodes);
    after = project.items.pageInfo.hasNextPage ? project.items.pageInfo.endCursor : null;
  } while (after);
  return {
    id: project.id,
    fields: Object.fromEntries(project.fields.nodes.filter(Boolean).map((field) => [field.name, field])),
    items,
    itemValues: itemValuesById(items),
  };
}

function itemValuesById(items) {
  return new Map(items.map((item) => [
    item.id,
    new Map(item.fieldValues.nodes
      .filter((value) => value.field?.name)
      .map((value) => [value.field.name, itemFieldValue(value)])),
  ]));
}

function itemFieldValue(value) {
  if (value.text !== undefined) return value.text;
  if (value.number !== undefined) return String(value.number);
  if (value.date !== undefined) return value.date;
  if (value.name !== undefined) return value.name;
  return "";
}

async function ensureProjectFields(github, project, core) {
  for (const [name, dataType] of Object.entries(FIELD_TYPES)) {
    if (project.fields[name]) continue;
    core.info(`Creating GitHub Project field: ${name}`);
    const result = await github.graphql(`
      mutation($projectId: ID!, $name: String!, $dataType: ProjectV2CustomFieldType!) {
        createProjectV2Field(input: {projectId: $projectId, name: $name, dataType: $dataType}) {
          projectV2Field {
            ... on ProjectV2Field { id name dataType }
            ... on ProjectV2SingleSelectField { id name dataType options { id name } }
          }
        }
      }
    `, { projectId: project.id, name, dataType });
    project.fields[name] = result.createProjectV2Field.projectV2Field;
  }
  return project;
}

function requireFields(fields) {
  const missing = REQUIRED_FIELDS.filter((name) => !fields[name]);
  if (missing.length > 0) {
    throw new Error(`GitHub Project is missing required fields: ${missing.join(", ")}`);
  }
  requireFieldType(fields, "Entity ID", ["TEXT"]);
  requireFieldType(fields, "Status", ["SINGLE_SELECT", "TEXT"]);
  requireFieldType(fields, "Kind", ["SINGLE_SELECT", "TEXT"]);
  requireFieldType(fields, "Score", ["NUMBER", "TEXT"]);
  requireFieldType(fields, "Source", ["TEXT"]);
  requireFieldType(fields, "PR", ["TEXT"]);
  requireFieldType(fields, "Updated At", ["DATE", "TEXT"]);
  requireFieldType(fields, "Archived", ["SINGLE_SELECT", "TEXT"]);
}

function requireFieldType(fields, name, types) {
  if (!types.includes(fields[name].dataType)) {
    throw new Error(`GitHub Project field ${name} must be one of: ${types.join(", ")}`);
  }
}

async function upsertProjectItem(github, project, values) {
  const item = findItemByEntityId(project, values.entityId);
  const syncedItem = item || await createDraftItem(github, project.id, values);
  if (item && item.content?.id && (item.content.title !== values.title || item.content.body !== values.body)) {
    await updateDraftItem(github, item.content.id, values);
  }
  await updateFields(github, project, syncedItem.id, values);
}

function findItemByEntityId(project, entityId) {
  return project.items.find((item) =>
    item.fieldValues.nodes.some((value) => value.field?.name === "Entity ID" && value.text === entityId)
  );
}

async function createDraftItem(github, projectId, values) {
  const result = await github.graphql(`
    mutation($projectId: ID!, $title: String!, $body: String!) {
      addProjectV2DraftIssue(input: {projectId: $projectId, title: $title, body: $body}) {
        projectItem { id content { ... on DraftIssue { id } } }
      }
    }
  `, { projectId, title: values.title, body: values.body });
  return result.addProjectV2DraftIssue.projectItem;
}

async function updateDraftItem(github, draftIssueId, values) {
  await github.graphql(`
    mutation($draftIssueId: ID!, $title: String!, $body: String!) {
      updateProjectV2DraftIssue(input: {draftIssueId: $draftIssueId, title: $title, body: $body}) {
        draftIssue { id }
      }
    }
  `, { draftIssueId, title: values.title, body: values.body });
}

async function updateFields(github, project, itemId, values) {
  const currentValues = project.itemValues.get(itemId) || new Map();
  await setField(github, project, itemId, currentValues, "Entity ID", values.entityId);
  await setField(github, project, itemId, currentValues, "Status", values.status, { skipMissingSingleSelect: true });
  await setField(github, project, itemId, currentValues, "Kind", values.kind);
  await setField(github, project, itemId, currentValues, "Score", values.score);
  await setField(github, project, itemId, currentValues, "Source", values.source);
  await setField(github, project, itemId, currentValues, "PR", values.pr);
  await setField(github, project, itemId, currentValues, "Updated At", values.updatedAt);
  await setField(github, project, itemId, currentValues, "Archived", String(values.archived));
}

async function setField(github, project, itemId, currentValues, name, value, options = {}) {
  const field = project.fields[name];
  const expected = comparableFieldValue(field, value);
  if (currentValues.get(name) === expected) return;
  if (value === null || value === undefined || value === "") {
    if (!currentValues.has(name)) return;
    await clearField(github, project.id, itemId, field.id);
    return;
  }
  let fieldValue;
  if (field.dataType === "NUMBER") {
    fieldValue = { number: Number(value) };
  } else if (field.dataType === "DATE") {
    fieldValue = { date: String(value).slice(0, 10) };
  } else if (field.dataType === "SINGLE_SELECT") {
    const option = field.options.find((candidate) => candidate.name.toLowerCase() === String(value).toLowerCase());
    if (!option && options.skipMissingSingleSelect) return;
    if (!option) throw new Error(`field ${name} is missing option: ${value}`);
    fieldValue = { singleSelectOptionId: option.id };
  } else {
    fieldValue = { text: String(value) };
  }
  await github.graphql(`
    mutation($projectId: ID!, $itemId: ID!, $fieldId: ID!, $value: ProjectV2FieldValue!) {
      updateProjectV2ItemFieldValue(input: {projectId: $projectId, itemId: $itemId, fieldId: $fieldId, value: $value}) {
        projectV2Item { id }
      }
    }
  `, { projectId: project.id, itemId, fieldId: field.id, value: fieldValue });
}

function comparableFieldValue(field, value) {
  if (value === null || value === undefined || value === "") return "";
  if (field.dataType === "NUMBER") return String(Number(value));
  if (field.dataType === "DATE") return String(value).slice(0, 10);
  return String(value);
}

async function clearField(github, projectId, itemId, fieldId) {
  await github.graphql(`
    mutation($projectId: ID!, $itemId: ID!, $fieldId: ID!) {
      clearProjectV2ItemFieldValue(input: {projectId: $projectId, itemId: $itemId, fieldId: $fieldId}) {
        projectV2Item { id }
      }
    }
  `, { projectId, itemId, fieldId });
}

function selfTest() {
  assert.equal(workflowIdFromBranch("spacedock-state/spacetop-dev"), "spacetop-dev");
  assert.throws(() => workflowIdFromBranch("main"));
  assert.equal(stateBranch({
    eventName: "workflow_dispatch",
    payload: { inputs: { workflow_id: "spacetop-dev" } },
  }), "spacedock-state/spacetop-dev");
  assert.equal(stateBranch({
    eventName: "push",
    ref: "refs/heads/spacedock-state/spacetop-dev",
  }), "spacedock-state/spacetop-dev");
  const parsed = parseSimpleYaml(`
id: spacetop-dev
github-project:
  owner: InfuseAI
  number: 12
archived: true
score: 0.84
`);
  assert.deepEqual(parsed["github-project"], { owner: "InfuseAI", number: 12 });
  assert.equal(parsed.archived, true);
  assert.equal(parsed.score, 0.84);
  const values = entityProjectValues("spacetop-dev", "_archive/task.md", {
    frontmatter: { id: "069", title: "Detect sessions", status: "shape", kind: "feature" },
    body: "Body",
  }, true, { timestamp: "2026-06-24T10:20:30Z" });
  assert.equal(values.entityId, "spacetop-dev:069");
  assert.equal(values.status, "Done");
  assert.equal(values.archived, true);
  assert.equal(values.updatedAt, "2026-06-24T10:20:30Z");
}

if (require.main === module) {
  if (process.argv.includes("--self-test")) {
    selfTest();
    console.log("sync-spacedock-project self-test passed");
  }
}
