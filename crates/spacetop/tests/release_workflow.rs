use std::{fs, path::PathBuf};

const RELEASE_WORKFLOW: &str = ".github/workflows/release.yml";

#[test]
fn release_workflow_is_driven_by_published_github_release() {
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("crate should live two levels below workspace root")
        .to_path_buf();
    let workflow = fs::read_to_string(workspace_root.join(RELEASE_WORKFLOW))
        .expect("release workflow should be readable");

    assert!(
        workflow.contains("release:\n    types: [published]"),
        "release workflow must run from a published GitHub Release event"
    );
    assert!(
        !workflow.contains("push:\n    tags:"),
        "release workflow must not be tag-push driven"
    );
    assert!(
        workflow.contains("RELEASE_TAG: ${{ github.event.release.tag_name }}"),
        "release version must be loaded from the GitHub Release tag"
    );
    assert!(
        workflow.contains("tag=\"${RELEASE_TAG}\""),
        "release shell scripts must consume the GitHub Release tag through an environment variable"
    );
    assert!(
        workflow.contains("ref: ${{ github.event.release.tag_name }}"),
        "release builds must checkout the released tag"
    );
    assert!(
        workflow.contains("gh release upload \"${tag}\""),
        "release workflow must upload assets to the existing GitHub Release"
    );
    assert!(
        !workflow.contains("gh release create"),
        "release workflow must not create the GitHub Release"
    );
}
