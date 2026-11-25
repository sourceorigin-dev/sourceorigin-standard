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
        .add_crate("core", "version = \"0.1.0\"\n")
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
             dependent can be published, or set `publish = false` on a crate that isn't \
             (workspace-dependencies)",
            "crates/app/Cargo.toml:10: `tempfile` is declared here: declare it once in the root's \
             `[workspace.dependencies]`, and inherit it with `tempfile.workspace = true` \
             (workspace-dependencies)",
            "crates/app/Cargo.toml:13: `libc` is declared here: declare it once in the root's \
             `[workspace.dependencies]`, and inherit it with `libc.workspace = true` \
             (workspace-dependencies)",
        ]
    );
}

/// A crate that isn't published needs no version, as its dependents
/// publish without it, and a crate's tests may depend on the crate itself
/// to turn a feature on. A crate without a `version` isn't published
/// either, so the published ones here have one.
#[test]
fn an_unpublished_crate_and_a_crate_itself_need_no_version() {
    let fixture = Fixture::new();
    fixture
        .write(
            "Cargo.toml",
            "[workspace]\nmembers = [\"crates/*\"]\n\n[workspace.dependencies]\n\
             generated = { path = \"crates/generated\" }\n",
        )
        .add_crate("generated", "version = \"0.1.0\"\npublish = false\n")
        .add_crate(
            "app",
            "version = \"0.1.0\"\n\n[features]\nfake = []\n\n[dev-dependencies]\n\
             generated.workspace = true\napp = { path = \".\", features = [\"fake\"] }\n",
        );
    assert_eq!(
        fixture.violations(Rule::WorkspaceDependencies),
        Vec::<String>::new()
    );
}
