use std::path::Path;
use std::process::Command;

use super::paths;
use crate::workspace::Workspace;

/// Runs git in `dir` as a test author.
fn git(dir: &Path, args: &[&str]) {
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
        .current_dir(dir)
        .status()
        .expect("git runs");
    assert!(status.success(), "git {}", args.join(" "));
}

#[test]
fn changed_paths_count_every_change_since_the_merge_base() {
    let repository = tempfile::tempdir().expect("a temporary directory");
    let root = repository.path();
    let write = |path: &str, text: &str| {
        std::fs::write(root.join(path), text).expect("the file is written");
    };
    git(root, &["init", "--quiet", "--initial-branch=main"]);
    for path in ["committed", "renamed", "unstaged"] {
        write(path, path);
    }
    write(".gitignore", "ignored\n");
    git(root, &["add", "."]);
    git(root, &["commit", "--quiet", "--message=base"]);
    // A later commit on main isn't part of the change.
    write("on-main", "on-main");
    git(root, &["add", "on-main"]);
    git(root, &["commit", "--quiet", "--message=main"]);
    git(root, &["checkout", "--quiet", "-b", "change", "HEAD~"]);
    write("committed", "changed");
    git(root, &["mv", "renamed", "moved"]);
    git(root, &["commit", "--quiet", "--all", "--message=change"]);
    write("staged", "staged");
    git(root, &["add", "staged"]);
    write("unstaged", "changed");
    write("untracked", "untracked");
    write("ignored", "ignored");

    let (_, changed) = Workspace::new(root)
        .changed_paths("main")
        .expect("the paths are listed");
    assert_eq!(
        changed,
        paths(&[
            "committed",
            "moved",
            "renamed",
            "staged",
            "unstaged",
            "untracked",
        ])
    );
}
