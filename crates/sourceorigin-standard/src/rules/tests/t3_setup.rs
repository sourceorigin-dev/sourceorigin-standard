use super::{Fixture, INHERITS};
use crate::rules::Rule;

/// A `t3.json` running `just <recipe>` when a worktree is created.
fn t3(recipe: &str) -> String {
    format!(
        "{{\n  \"scripts\": [\n    {{\n      \"name\": \"Setup\",\n      \"command\": \"just \
         {recipe}\",\n      \"runOnWorktreeCreate\": true\n    }}\n  ]\n}}\n"
    )
}

/// A justfile whose `setup` builds through `just build`, `limited` and
/// `configured` at low priority on half the cores, the latter unless Cargo's
/// `build.jobs` is set, `unhalved` on every core unless it is set, and
/// `install` doesn't build.
const JUSTFILE: &str = "\
setup:
    just build

limited:
    #!/usr/bin/env bash
    export CARGO_BUILD_JOBS=\"${CARGO_BUILD_JOBS:-$(( ($(nproc) + 1) / 2 ))}\"
    exec nice -n 10 just build

configured:
    #!/usr/bin/env bash
    if ! RUSTC_BOOTSTRAP=1 cargo -Zunstable-options config get build.jobs >/dev/null 2>&1; then
        export CARGO_BUILD_JOBS=\"$(( ($(nproc 2>/dev/null || echo 1) + 1) / 2 ))\"
    fi
    nice -n 10 just build

unhalved:
    #!/usr/bin/env bash
    if ! RUSTC_BOOTSTRAP=1 cargo -Zunstable-options config get build.jobs >/dev/null 2>&1; then
        export CARGO_BUILD_JOBS=\"$(nproc)\"
    fi
    nice -n 10 just build

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
    let unlimited = |recipe: &str| {
        vec![format!(
            "t3.json:5: `just {recipe}` builds on every core at full priority when a worktree is \
             created: in its body, `export CARGO_BUILD_JOBS=\"${{CARGO_BUILD_JOBS:-$(( ($(nproc) \
             + 1) / 2 ))}}\"`, or export it only when `cargo config get build.jobs` finds \
             nothing, and run the build with `nice -n 10` (t3-setup)"
        )]
    };
    for (recipe, violations) in [
        ("setup", unlimited("setup")),
        ("unhalved", unlimited("unhalved")),
        ("limited", vec![]),
        ("configured", vec![]),
        ("install", vec![]),
    ] {
        fixture.write("t3.json", &t3(recipe));
        assert_eq!(fixture.violations(Rule::T3Setup), violations, "{recipe}");
    }
}
