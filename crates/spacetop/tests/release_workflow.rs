use serde_yaml::Value;
use std::{fs, path::PathBuf};

const RELEASE_POLICY: &str = "docs/release-policy.md";
const RELEASE_WORKFLOW: &str = ".github/workflows/release.yml";

#[test]
fn release_workflow_is_driven_by_published_github_release() {
    let workflow = read_yaml(RELEASE_WORKFLOW);

    let release_trigger = field(field(&workflow, "on"), "release");
    assert_eq!(
        string_sequence(field(release_trigger, "types")),
        vec!["published"],
        "release workflow must run from a published GitHub Release event"
    );
    assert!(
        mapping_field(field(&workflow, "on"), "push").is_none(),
        "release workflow must not be tag-push driven"
    );

    let validate_job = field(field(&workflow, "jobs"), "validate");
    let validate_checkout = step_named(validate_job, "Checkout");
    assert_eq!(
        string_field(field(validate_checkout, "with"), "ref"),
        "${{ github.event.release.tag_name }}",
        "validation must checkout the released tag"
    );

    let validate_step = step_named(validate_job, "Validate tag and Cargo versions");
    assert_eq!(
        string_field(field(validate_step, "env"), "RELEASE_TAG"),
        "${{ github.event.release.tag_name }}",
        "release version must be loaded from the GitHub Release tag"
    );
    assert_eq!(
        string_field(field(validate_step, "env"), "RELEASE_NAME"),
        "${{ github.event.release.name }}",
        "release title validation must read the GitHub Release name"
    );

    let validation_script = string_field(validate_step, "run");
    assert!(
        validation_script.contains("tag=\"${RELEASE_TAG}\""),
        "release shell scripts must consume the GitHub Release tag through an environment variable"
    );
    assert!(
        validation_script.contains("if [[ \"${RELEASE_NAME}\" != \"${tag}\" ]]; then"),
        "release title validation must require an exact title/tag match"
    );
    assert!(
        !validation_script.contains("-n \"${RELEASE_NAME}\""),
        "release title validation must not allow an empty title"
    );

    let build_job = field(field(&workflow, "jobs"), "build");
    let build_checkout = step_named(build_job, "Checkout");
    assert_eq!(
        string_field(field(build_checkout, "with"), "ref"),
        "${{ needs.validate.outputs.tag }}",
        "release builds must checkout the validated release tag"
    );

    let upload_job = field(field(&workflow, "jobs"), "upload");
    let upload_step = step_named(upload_job, "Upload assets to existing release");
    let upload_script = string_field(upload_step, "run");
    assert!(
        upload_script.contains("gh release upload \"${tag}\""),
        "release workflow must upload assets to the existing GitHub Release"
    );
    assert!(
        !upload_script.contains("gh release create"),
        "release workflow must not create the GitHub Release"
    );
}

#[test]
fn release_policy_targets_an_exact_release_commit() {
    let policy = read_text(RELEASE_POLICY);

    assert!(
        !policy.contains("--target main"),
        "release policy must not recommend targeting a moving main branch"
    );
    assert!(
        policy.contains("release_commit=\"$(git rev-parse HEAD)\""),
        "release policy must capture the exact release commit SHA"
    );
    assert!(
        policy.contains("--target \"${release_commit}\""),
        "release policy CLI example must target the exact release commit"
    );
}

fn read_text(path: &str) -> String {
    fs::read_to_string(workspace_root().join(path)).unwrap_or_else(|error| {
        panic!("failed to read {path}: {error}");
    })
}

fn read_yaml(path: &str) -> Value {
    let text = read_text(path);
    serde_yaml::from_str(&text).unwrap_or_else(|error| {
        panic!("failed to parse {path} as YAML: {error}");
    })
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("crate should live two levels below workspace root")
        .to_path_buf()
}

fn field<'a>(value: &'a Value, key: &str) -> &'a Value {
    mapping_field(value, key).unwrap_or_else(|| {
        panic!("missing YAML field {key:?} in {value:?}");
    })
}

fn mapping_field<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    let Value::Mapping(mapping) = value else {
        panic!("expected YAML mapping while reading {key:?}, got {value:?}");
    };
    mapping.get(Value::String(key.to_owned()))
}

fn step_named<'a>(job: &'a Value, name: &str) -> &'a Value {
    let steps = field(job, "steps");
    let Value::Sequence(steps) = steps else {
        panic!("expected job steps sequence, got {steps:?}");
    };
    steps
        .iter()
        .find(|step| string_field(step, "name") == name)
        .unwrap_or_else(|| panic!("missing workflow step {name:?}"))
}

fn string_field<'a>(value: &'a Value, key: &str) -> &'a str {
    let field = field(value, key);
    let Value::String(string) = field else {
        panic!("expected YAML string field {key:?}, got {field:?}");
    };
    string
}

fn string_sequence(value: &Value) -> Vec<&str> {
    let Value::Sequence(values) = value else {
        panic!("expected YAML string sequence, got {value:?}");
    };
    values
        .iter()
        .map(|value| {
            let Value::String(string) = value else {
                panic!("expected YAML string sequence item, got {value:?}");
            };
            string.as_str()
        })
        .collect()
}
