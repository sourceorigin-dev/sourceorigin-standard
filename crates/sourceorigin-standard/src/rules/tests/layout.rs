use super::Fixture;
use crate::rules::Rule;

/// A root manifest whose members are `members`.
fn root(members: &str) -> String {
    format!("[workspace]\nmembers = [{members}]\n")
}

impl Fixture {
    /// Adds the crate at `dir`.
    fn add_crate_at(&self, dir: &str) -> &Self {
        let name = dir.rsplit('/').next().unwrap_or(dir);
        self.write(
            &format!("{dir}/Cargo.toml"),
            &format!("[package]\nname = \"{name}\"\nedition = \"2024\"\n"),
        )
        .write(&format!("{dir}/src/lib.rs"), "")
    }
}

#[test]
fn crates_live_in_their_directories_and_cargo_xtask_runs_locked() {
    let fixture = Fixture::new();
    fixture
        .write(
            "Cargo.toml",
            &root("\"crates/core\", \"apps/daemon\", \"tools/xtask\", \"lib\""),
        )
        .add_crate_at("crates/core")
        .add_crate_at("apps/daemon")
        .add_crate_at("tools/xtask")
        .add_crate_at("lib")
        .write(
            ".cargo/config.toml",
            "[alias]\nxtask = \"run --package xtask --\"\n",
        );
    assert_eq!(
        fixture.violations(Rule::Layout),
        [
            ".cargo/config.toml:2: the `xtask` alias is `run --package xtask --`: make it \
             `run --locked --package xtask --` (layout)",
            "lib/Cargo.toml:1: the crate is outside `crates/`, `apps/` and `tools/`: move it to \
             `crates/` for a library, `apps/` for a binary, or `tools/` for a development task \
             (layout)",
        ]
    );
}

#[test]
fn the_xtask_crate_lives_in_tools() {
    let fixture = Fixture::new();
    fixture
        .write("Cargo.toml", &root("\"crates/xtask\""))
        .add_crate_at("crates/xtask")
        .write(
            ".cargo/config.toml",
            "[alias]\nxtask = [\"run\", \"--locked\", \"--package\", \"xtask\", \"--\"]\n",
        );
    assert_eq!(
        fixture.violations(Rule::Layout),
        [
            "tools/xtask:1: no `xtask` crate in `tools/xtask`: put the repository's development \
             tasks there, run as `cargo xtask` (layout)"
        ]
    );
}

#[test]
fn fix_aliases_cargo_xtask() {
    for config in [
        None,
        Some("[env]\nA = \"1\"\n"),
        Some("[alias]\nxtask = \"run\"\n"),
    ] {
        let fixture = Fixture::new();
        fixture
            .write("Cargo.toml", &root("\"tools/xtask\""))
            .add_crate_at("tools/xtask");
        match config {
            Some(config) => fixture.write(".cargo/config.toml", config),
            None => fixture.remove(".cargo/config.toml"),
        };
        fixture.fix_rule(Rule::Layout);
        assert_eq!(
            fixture.violations(Rule::Layout),
            Vec::<String>::new(),
            "{config:?}"
        );
    }
}
