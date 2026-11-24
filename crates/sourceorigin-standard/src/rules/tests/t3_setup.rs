use super::{Fixture, INHERITS};
use crate::rules::Rule;

/// A `t3.json` running `just <recipe>` when a worktree is created.
fn t3(recipe: &str) -> String {
    format!(
        "{{\n  \"scripts\": [\n    {{\n      \"name\": \"Setup\",\n      \"command\": \"just \
         {recipe}\",\n      \"runOnWorktreeCreate\": true\n    }}\n  ]\n}}\n"
    )
}

/// A justfile whose `setup` builds through `just build`, `limited` at low
/// priority on half the cores, and `install` doesn't build.
const JUSTFILE: &str = "\
setup:
    just build

limited:
    #!/usr/bin/env bash
    export CARGO_BUILD_JOBS=\"${CARGO_BUILD_JOBS:-$(( ($(nproc) + 1) / 2 ))}\"
    exec nice -n 10 just build

install:
    pnpm install

build: generate
    cargo build --locked

generate:
";

#[test]
fn a_worktree_build_runs_at_low_priority_on_half_the_cores() {
    let fixture = Fixture::new();
    fixture
        .add_crate("core", INHERITS)
        .write("justfile", JUSTFILE);
    for (recipe, violations) in [
        (
            "setup",
            vec![
                "t3.json:5: `just setup` builds on every core at full priority when a worktree \
                 is created: in its body, `export CARGO_BUILD_JOBS=\"${CARGO_BUILD_JOBS:-$(( \
                 ($(nproc) + 1) / 2 ))}\"` and run the build with `nice -n 10` (t3-setup)",
            ],
        ),
        ("limited", vec![]),
        ("install", vec![]),
    ] {
        fixture.write("t3.json", &t3(recipe));
        assert_eq!(fixture.violations(Rule::T3Setup), violations, "{recipe}");
    }
}
