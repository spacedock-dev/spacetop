use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct App {
    workflow_dir: PathBuf,
}

impl App {
    pub fn new(workflow_dir: impl Into<PathBuf>) -> Self {
        Self {
            workflow_dir: workflow_dir.into(),
        }
    }

    pub fn workflow_dir(&self) -> &Path {
        &self.workflow_dir
    }
}

#[cfg(test)]
mod tests {
    use super::App;
    use std::path::Path;

    #[test]
    fn stores_workflow_directory() {
        let app = App::new("docs/spacetop-dev");

        assert_eq!(app.workflow_dir(), Path::new("docs/spacetop-dev"));
    }
}
