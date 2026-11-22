use super::Fixture;
use crate::rules::Rule;

#[test]
fn dependencies_are_declared_once_for_the_workspace() {
    let fixture = Fixture::new();
    fixture
        .write(
            "Cargo.toml",
            "[workspace]\nmembers = [\"crates/*\"]\n\n[workspace.dependencies]\n\
             core = { path = \"crates/core\" }\nserde = \"1\"\n",
        )
        .add_crate("core", "")
        .add_crate(
            "app",
            "\n[dependencies]\ncore.workspace = true\nserde = { workspace = true, features \
             = [\"derive\"] }\n\n[dev-dependencies]\ntempfile = \"3\"\n\n\
             [target.'cfg(unix)'.dependencies]\nlibc = { version = \"0.2\" }\n",
        );
    assert_eq!(
        fixture.violations(Rule::WorkspaceDependencies),
        [
            "Cargo.toml:5: `core` has a `path` and no `version`: add the crate's version, so a \
             dependent can be published (workspace-dependencies)",
            "crates/app/Cargo.toml:10: `tempfile` is declared here: declare it once in the root's \
             `[workspace.dependencies]`, and inherit it with `tempfile.workspace = true` \
             (workspace-dependencies)",
            "crates/app/Cargo.toml:13: `libc` is declared here: declare it once in the root's \
             `[workspace.dependencies]`, and inherit it with `libc.workspace = true` \
             (workspace-dependencies)",
        ]
    );
}
