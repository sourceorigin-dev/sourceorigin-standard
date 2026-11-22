use super::{Fixture, INHERITS};
use crate::rules::Rule;

/// A justfile with locked, unlocked and other Cargo commands.
const JUSTFILE: &str = "\
build:
    @cargo build --workspace && cargo test
# cargo test in a comment
test *ARGS:
    cargo nextest run --locked \"$@\"
    cargo fmt --all && cargo +nightly doc --no-deps
lint:
    cargo clippy \\
        --locked -- -D warnings
    cargo xtask standard; cargo nextest run --workspace
";

#[test]
fn cargo_commands_that_resolve_dependencies_pass_locked() {
    let fixture = Fixture::new();
    fixture
        .add_crate("core", INHERITS)
        .write("justfile", JUSTFILE)
        .write(
            ".github/workflows/ci.yml",
            "jobs:\n  rust:\n    steps:\n      - run: |\n          cargo test --frozen\n          \
             cargo run --package tool\n",
        )
        .write("t3.json", "{ \"command\": \"cargo check --workspace\" }\n")
        .write("scripts/build.sh", "cargo build\n");
    let unlocked = |path: &str, line: usize, subcommand: &str| {
        format!(
            "{path}:{line}: `cargo {subcommand}` without `--locked`: add it, so the command \
             never updates Cargo.lock (cargo-locked)"
        )
    };
    assert_eq!(
        fixture.violations(Rule::CargoLocked),
        [
            unlocked(".github/workflows/ci.yml", 6, "run"),
            unlocked("justfile", 2, "build"),
            unlocked("justfile", 2, "test"),
            unlocked("justfile", 6, "doc"),
            unlocked("justfile", 10, "nextest"),
            unlocked("t3.json", 1, "check"),
        ]
    );
}

#[test]
fn fix_adds_locked_after_the_subcommand() {
    let fixture = Fixture::new();
    fixture
        .add_crate("core", INHERITS)
        .write("justfile", JUSTFILE)
        .fix_rule(Rule::CargoLocked);
    let justfile =
        std::fs::read_to_string(fixture.0.root().join("justfile")).expect("the justfile is read");
    assert_eq!(
        justfile.lines().collect::<Vec<_>>(),
        [
            "build:",
            "    @cargo build --locked --workspace && cargo test --locked",
            "# cargo test in a comment",
            "test *ARGS:",
            "    cargo nextest run --locked \"$@\"",
            "    cargo fmt --all && cargo +nightly doc --locked --no-deps",
            "lint:",
            "    cargo clippy \\",
            "        --locked -- -D warnings",
            "    cargo xtask standard; cargo nextest run --locked --workspace",
        ]
    );
}
