use super::{Fixture, INHERITS};
use crate::rules::Rule;

/// A justfile with locked, unlocked and other Cargo commands, and Cargo
/// commands that are text.
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
    echo \"cargo build failed\"
";

/// A workflow whose step and job names say `cargo test`.
const WORKFLOW: &str = "\
jobs:
  rust:
    name: cargo test
    steps:
      - name: cargo build
        run: |
          cargo test --frozen
          cargo run --package tool
          echo 'cargo build done'
";

/// A `t3.json` with one unlocked command.
const T3: &str = "{\n  \"scripts\": [\n    { \"name\": \"cargo check\", \"command\": \"cargo check \
                  --workspace\" }\n  ]\n}\n";

#[test]
fn cargo_commands_that_resolve_dependencies_pass_locked() {
    let fixture = Fixture::new();
    fixture
        .add_crate("core", INHERITS)
        .write("justfile", JUSTFILE)
        .write(".github/workflows/ci.yml", WORKFLOW)
        .write("t3.json", T3)
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
            unlocked(".github/workflows/ci.yml", 8, "run"),
            unlocked("justfile", 2, "build"),
            unlocked("justfile", 2, "test"),
            unlocked("justfile", 6, "doc"),
            unlocked("justfile", 10, "nextest"),
            unlocked("t3.json", 3, "check"),
        ]
    );
}

#[test]
fn fix_adds_locked_after_the_subcommand_and_nowhere_else() {
    let fixture = Fixture::new();
    fixture
        .add_crate("core", INHERITS)
        .write("justfile", JUSTFILE)
        .write(".github/workflows/ci.yml", WORKFLOW)
        .write("t3.json", T3)
        .fix_rule(Rule::CargoLocked);
    let read = |path: &str| {
        std::fs::read_to_string(fixture.0.root().join(path)).expect("the file is read")
    };
    assert_eq!(
        read("justfile").lines().collect::<Vec<_>>(),
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
            "    echo \"cargo build failed\"",
        ]
    );
    assert_eq!(
        read(".github/workflows/ci.yml"),
        WORKFLOW.replace("cargo run --package", "cargo run --locked --package")
    );
    assert_eq!(
        read("t3.json"),
        T3.replace("check --workspace", "check --locked --workspace")
    );
    assert_eq!(fixture.violations(Rule::CargoLocked), Vec::<String>::new());
}

/// A workflow whose Cargo commands sit behind prefixes, in a folded block,
/// in a heredoc, in a quoted scalar, or only as text.
const SHAPES: &str = "\
jobs:
  rust:
    steps:
      - run: echo building with cargo build
      - run: >
          cargo build
          --locked
      - run: |
          cat <<'EOF'
          cargo test
          EOF
          cargo doc
      - run: \"cargo clippy --workspace\"
      - run: FOO=1 nice -n 10 cargo check
";

#[test]
fn only_a_command_word_is_cargo_and_folded_blocks_are_one_command() {
    let fixture = Fixture::new();
    fixture
        .add_crate("core", INHERITS)
        .write(".github/workflows/ci.yml", SHAPES);
    let unlocked = |line: usize, subcommand: &str| {
        format!(
            ".github/workflows/ci.yml:{line}: `cargo {subcommand}` without `--locked`: add it, \
             so the command never updates Cargo.lock (cargo-locked)"
        )
    };
    assert_eq!(
        fixture.violations(Rule::CargoLocked),
        [
            unlocked(12, "doc"),
            unlocked(13, "clippy"),
            unlocked(14, "check")
        ]
    );
    fixture.fix_rule(Rule::CargoLocked);
    let fixed = std::fs::read_to_string(fixture.0.root().join(".github/workflows/ci.yml"))
        .expect("the workflow is read");
    assert_eq!(
        fixed,
        SHAPES
            .replace("cargo doc", "cargo doc --locked")
            .replace("clippy --workspace", "clippy --locked --workspace")
            .replace("cargo check", "cargo check --locked")
    );
}

/// The violation of `cargo {subcommand}` at `line` of the CI workflow.
fn unlocked_in_ci(line: usize, subcommand: &str) -> String {
    format!(
        ".github/workflows/ci.yml:{line}: `cargo {subcommand}` without `--locked`: add it, so the \
         command never updates Cargo.lock (cargo-locked)"
    )
}

#[test]
fn a_command_after_a_shell_keyword_is_one() {
    let fixture = Fixture::new();
    fixture.add_crate("core", INHERITS).write(
        ".github/workflows/ci.yml",
        "jobs:\n  rust:\n    steps:\n      - run: |\n          if cargo build; then\n            \
         echo built\n          fi\n          for x in 1; do cargo test; done\n          ! cargo check\n",
    );
    assert_eq!(
        fixture.violations(Rule::CargoLocked),
        [
            unlocked_in_ci(5, "build"),
            unlocked_in_ci(8, "test"),
            unlocked_in_ci(9, "check")
        ]
    );
}

#[test]
fn a_shift_opens_no_heredoc() {
    let fixture = Fixture::new();
    fixture.add_crate("core", INHERITS).write(
        ".github/workflows/ci.yml",
        "jobs:\n  rust:\n    steps:\n      - run: |\n          echo $((1 << 2))\n          cargo run\n          \
         x=$((1 << n))\n          cargo test\n",
    );
    assert_eq!(
        fixture.violations(Rule::CargoLocked),
        [unlocked_in_ci(6, "run"), unlocked_in_ci(8, "test")]
    );
}

#[test]
fn a_folded_block_ends_its_command_at_its_last_line() {
    let fixture = Fixture::new();
    fixture.add_crate("core", INHERITS).write(
        ".github/workflows/ci.yml",
        "jobs:\n  rust:\n    steps:\n      - run: >\n          cargo build\n          --locked\n      \
         - run: cargo check\n",
    );
    assert_eq!(
        fixture.violations(Rule::CargoLocked),
        [unlocked_in_ci(7, "check")]
    );
}

#[test]
fn a_blank_line_ends_a_folded_blocks_command() {
    let fixture = Fixture::new();
    fixture.add_crate("core", INHERITS).write(
        ".github/workflows/ci.yml",
        "jobs:\n  rust:\n    steps:\n      - run: >\n          cargo test\n\n          \
         cargo doc --locked\n",
    );
    assert_eq!(
        fixture.violations(Rule::CargoLocked),
        [unlocked_in_ci(5, "test")]
    );
}
