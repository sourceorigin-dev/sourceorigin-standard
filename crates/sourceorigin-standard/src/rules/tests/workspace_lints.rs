use super::{Fixture, INHERITS};
use crate::rules::Rule;

#[test]
fn crates_inherit_the_workspace_lints() {
    let fixture = Fixture::new();
    fixture
        .add_crate("inherits", INHERITS)
        .add_crate("none", "")
        .add_crate("own", "\n[lints.rust]\nunsafe_code = \"allow\"\n");
    assert_eq!(
        fixture.violations(Rule::WorkspaceLints),
        [
            "crates/none/Cargo.toml:1: no `[lints]`: add `[lints] workspace = true` \
             (workspace-lints)",
            "crates/own/Cargo.toml:5: `[lints]` declares the crate's own lints: move them to \
             `[workspace.lints]`, and inherit those with `workspace = true` (workspace-lints)",
        ]
    );
}

#[test]
fn every_required_lint_warns_and_others_warn_or_allow() {
    let fixture = Fixture::new();
    let lints = super::LINTS
        .replace("unsafe_code = \"warn\"", "unsafe_code = \"forbid\"")
        .replace("missing_docs = \"warn\"", "missing_docs = \"allow\"")
        .replace(
            "pedantic = { level = \"warn\", priority = -1 }",
            "pedantic = { level = \"deny\", priority = -1 }",
        )
        .replace(
            "panic = \"warn\"\n",
            "module_name_repetitions = \"allow\"\n",
        );
    fixture.add_crate("core", INHERITS).write(
        "Cargo.toml",
        &format!("[workspace]\nmembers = [\"crates/*\"]\n\n{lints}"),
    );
    assert_eq!(
        fixture.violations(Rule::WorkspaceLints),
        [
            "Cargo.toml:1: the standard requires `panic = \"warn\"` in \
             `[workspace.lints.clippy]` (workspace-lints)",
            "Cargo.toml:5: `unsafe_code` is `forbid`: make it `warn`, which `-D warnings` makes \
             an error (workspace-lints)",
            "Cargo.toml:6: `missing_docs` is `allow`, but the standard requires it: make it \
             `warn` (workspace-lints)",
            "Cargo.toml:10: `clippy::pedantic` is `deny`: make it `warn`, which `-D warnings` \
             makes an error (workspace-lints)",
        ]
    );
}

#[test]
fn fix_adds_each_required_lint_the_workspace_lacks() {
    let fixture = Fixture::new();
    fixture.add_crate("core", INHERITS).write(
        "Cargo.toml",
        "[workspace]\nmembers = [\"crates/*\"]\n\n[workspace.lints.rust]\nunsafe_code = \"warn\"\n",
    );
    fixture.fix_rule(Rule::WorkspaceLints);
    assert_eq!(
        fixture.violations(Rule::WorkspaceLints),
        Vec::<String>::new()
    );
    let manifest =
        std::fs::read_to_string(fixture.0.root().join("Cargo.toml")).expect("the manifest is read");
    assert!(
        manifest
            .contains("\n[workspace.lints.clippy]\nall = { level = \"warn\", priority = -1 }\n"),
        "{manifest}"
    );
}
