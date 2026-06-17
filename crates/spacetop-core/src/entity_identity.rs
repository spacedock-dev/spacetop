use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

/// Derive the workflow entity slug from a flat `{slug}.md` path or a
/// folder-form `{slug}/index.md` path.
pub fn entity_slug(path: &Path) -> Option<String> {
    entity_slug_os(path).map(|slug| slug.to_string_lossy().into_owned())
}

pub(crate) fn entity_slug_os(path: &Path) -> Option<OsString> {
    let stem = path.file_stem()?;
    if stem == "index" {
        path.parent().and_then(Path::file_name).map(OsStr::to_owned)
    } else {
        Some(stem.to_owned())
    }
}

pub(crate) fn archived_entity_paths(archive_root: &Path, slug: &OsStr) -> (PathBuf, PathBuf) {
    let mut flat_name = OsString::from(slug);
    flat_name.push(".md");
    (
        archive_root.join(Path::new(&flat_name)),
        archive_root.join(Path::new(slug)).join("index.md"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entity_slug_uses_flat_file_stem() {
        assert_eq!(
            entity_slug(Path::new("workflow/task-064.md")),
            Some("task-064".to_string())
        );
    }

    #[test]
    fn entity_slug_uses_parent_for_folder_form_index() {
        assert_eq!(
            entity_slug(Path::new("workflow/task-064/index.md")),
            Some("task-064".to_string())
        );
    }

    #[test]
    fn entity_slug_returns_none_when_path_has_no_filename() {
        assert_eq!(entity_slug(Path::new("/")), None);
    }
}
