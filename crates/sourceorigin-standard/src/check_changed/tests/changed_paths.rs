use super::paths;
use crate::testing::Repository;
use crate::workspace::Workspace;

/// Committed, renamed, staged, unstaged and untracked files count; ignored
/// files and a later commit on main don't.
#[test]
fn changed_paths_count_every_change_since_the_merge_base() {
    let repository = Repository::new();
    for path in ["committed", "renamed", "unstaged"] {
        repository.write(path, path);
    }
    repository.write(".gitignore", "ignored\n");
    repository.git(&["add", "."]);
    repository.git(&["commit", "--quiet", "--message=base"]);
    repository.write("on-main", "on-main");
    repository.git(&["add", "on-main"]);
    repository.git(&["commit", "--quiet", "--message=main"]);
    repository.git(&["checkout", "--quiet", "-b", "change", "HEAD~"]);
    repository.write("committed", "changed");
    repository.git(&["mv", "renamed", "moved"]);
    repository.git(&["commit", "--quiet", "--all", "--message=change"]);
    repository.write("staged", "staged");
    repository.git(&["add", "staged"]);
    repository.write("unstaged", "changed");
    repository.write("untracked", "untracked");
    repository.write("ignored", "ignored");

    let (_, changed) = Workspace::new(repository.root())
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
