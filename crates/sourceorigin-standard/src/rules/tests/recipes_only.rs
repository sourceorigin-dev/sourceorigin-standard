use super::{Fixture, INHERITS};
use crate::rules::Rule;

#[test]
fn t3_and_ci_run_recipes() {
    let fixture = Fixture::new();
    fixture
        .add_crate("core", INHERITS)
        .write(
            "t3.json",
            "{\n  \"scripts\": [\n    { \"name\": \"Lint\", \"command\": \"just lint\" },\n    \
             { \"name\": \"Check\", \"command\": \"cargo check --locked\" }\n  ]\n}\n",
        )
        .write(
            ".github/workflows/ci.yml",
            "on: push\njobs:\n  rust:\n    steps:\n      - run: just lint\n      - run: |\n          \
             cargo build --locked --package xtask\n          cargo +1.85 check --locked\n",
        )
        .write(
            ".github/actions/ui/action.yml",
            "runs:\n  using: composite\n  steps:\n    - run: cargo build --locked\n      shell: bash\n",
        )
        .write(
            "standard.toml",
            "cargo-in-ci = [\n  \"cargo +1.85 check --locked\",\n  \"cargo xtask dist\",\n]\n",
        );
    assert_eq!(
        fixture.violations(Rule::RecipesOnly),
        [
            ".github/actions/ui/action.yml:4: `cargo build --locked` runs Cargo rather than a \
             recipe: run a recipe of the justfile, or list the line in standard.toml's \
             `cargo-in-ci` if no recipe fits (recipes-only)",
            ".github/workflows/ci.yml:7: `cargo build --locked --package xtask` runs Cargo \
             rather than a recipe: run a recipe of the justfile, or list the line in \
             standard.toml's `cargo-in-ci` if no recipe fits (recipes-only)",
            "standard.toml:3: `cargo-in-ci` lists `cargo xtask dist`, which no workflow runs: \
             remove it (recipes-only)",
            "t3.json:4: `cargo check --locked` isn't a recipe: add it to the justfile, and run \
             it with `just` (recipes-only)",
        ]
    );
}
