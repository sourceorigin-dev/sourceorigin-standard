use super::{Fixture, INHERITS};
use crate::rules::Rule;

/// A manifest tail depending on `anyhow`, and on `thiserror` for tests.
const ANYHOW: &str = "\n[dependencies]\nanyhow = \"1\"\n\n[dev-dependencies]\nthiserror = \"2\"\n";

#[test]
fn only_listed_edges_and_binaries_depend_on_anyhow() {
    let fixture = Fixture::new();
    fixture
        .add_crate("core", &format!("{ANYHOW}{INHERITS}"))
        .add_crate("cli", &format!("{ANYHOW}{INHERITS}"))
        .add_crate(
            "tests",
            &format!("\n[dev-dependencies]\nanyhow = \"1\"\n{INHERITS}"),
        )
        .write(
            "crates/tool/Cargo.toml",
            &format!("[package]\nname = \"tool\"\nedition = \"2024\"\n{ANYHOW}{INHERITS}"),
        )
        .write("crates/tool/src/main.rs", "fn main() {}\n")
        .write(
            "standard.toml",
            "anyhow-crates = [\n  \"cli\",\n  \"gone\",\n]\n",
        );
    assert_eq!(
        fixture.violations(Rule::AnyhowEdges),
        [
            "crates/core/Cargo.toml:1: the library `core` depends on `anyhow`: return a \
             `thiserror` enum instead, or list the crate in standard.toml's `anyhow-crates` if \
             it is a binary's edge (anyhow-edges)",
            "standard.toml:3: `anyhow-crates` lists `gone`, no workspace crate: remove it \
             (anyhow-edges)",
        ]
    );
}
