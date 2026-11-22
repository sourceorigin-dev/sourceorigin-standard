use super::Fixture;
use crate::rules::Rule;

#[test]
fn the_root_sets_the_resolver_and_edition_every_crate_inherits() {
    let fixture = Fixture::new();
    fixture
        .write(
            "Cargo.toml",
            "[workspace]\nmembers = [\"crates/*\"]\n\n[workspace.package]\nedition = \"2021\"\n",
        )
        .add_crate("own", "")
        .write(
            "crates/inherits/Cargo.toml",
            "[package]\nname = \"inherits\"\nedition.workspace = true\n",
        )
        .write("crates/inherits/src/lib.rs", "")
        .write(
            "crates/missing/Cargo.toml",
            "[package]\nname = \"missing\"\n",
        )
        .write("crates/missing/src/lib.rs", "");
    assert_eq!(
        fixture.violations(Rule::WorkspaceManifest),
        [
            "Cargo.toml:1: `[workspace]` sets no `resolver`: add `resolver = \"3\"` \
             (workspace-manifest)",
            "Cargo.toml:5: `[workspace.package]` sets `edition = \"2021\"`: make it \"2024\", \
             migrating with `cargo fix --edition` (workspace-manifest)",
            "crates/missing/Cargo.toml:1: the crate sets its own edition: inherit the \
             workspace's with `edition.workspace = true` (workspace-manifest)",
            "crates/own/Cargo.toml:3: the crate sets its own edition: inherit the workspace's \
             with `edition.workspace = true` (workspace-manifest)",
        ]
    );
}

#[test]
fn fix_makes_a_crate_inherit_the_edition_it_repeats() {
    let fixture = Fixture::new();
    fixture
        .write(
            "Cargo.toml",
            "[workspace]\nresolver = \"3\"\nmembers = [\"crates/*\"]\n\n\
             [workspace.package]\nedition = \"2024\"\n",
        )
        .add_crate("core", "");
    fixture.fix_rule(Rule::WorkspaceManifest);
    assert_eq!(
        fixture.violations(Rule::WorkspaceManifest),
        Vec::<String>::new()
    );
}
