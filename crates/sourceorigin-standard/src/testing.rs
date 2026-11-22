//! Fixtures the tests of every check share.

use std::path::Path;
use std::process::Command;

use tempfile::TempDir;

/// A scratch git repository, deleted when dropped.
pub(crate) struct Repository(TempDir);

impl Repository {
    /// An empty repository whose branch is `main`.
    pub(crate) fn new() -> Self {
        let repository = Self(tempfile::tempdir().expect("a temporary directory"));
        repository.git(&["init", "--quiet", "--initial-branch=main"]);
        repository
    }

    pub(crate) fn root(&self) -> &Path {
        self.0.path()
    }

    /// Writes `text` to `path`, from the root, creating its directories.
    pub(crate) fn write(&self, path: &str, text: &str) {
        let path = self.root().join(path);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).expect("the directory is created");
        }
        std::fs::write(path, text).expect("the file is written");
    }

    /// Runs git in the repository as a test author.
    pub(crate) fn git(&self, args: &[&str]) {
        let status = Command::new("git")
            .args(["-c", "user.name=test", "-c", "user.email=test@example.com"])
            // Neither sign nor run hooks, whatever the developer's configuration says.
            .args([
                "-c",
                "commit.gpgsign=false",
                "-c",
                "core.hooksPath=/dev/null",
            ])
            .args(args)
            .current_dir(self.root())
            .status()
            .expect("git runs");
        assert!(status.success(), "git {}", args.join(" "));
    }
}
