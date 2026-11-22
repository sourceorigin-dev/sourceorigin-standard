# sourceorigin-standard's development tasks. `just` alone runs `validate`, the
# checks every change passes before review. CI runs the same recipes
# (.github/workflows/ci.yml).

default: validate

# Formats the sources, then runs every check CI runs.
validate: fmt lint standard test test-pr-checks doc doc-test audit unused package

build:
    @cargo build --locked --workspace --all-targets

fmt:
    @cargo fmt --all

# CI checks formatting without modifying sources; `validate` uses `fmt`.
fmt-check:
    @cargo fmt --all -- --check

# The workspace pass unifies features across crates, so a crate can compile
# there only because another crate enables a feature it needs: linting each
# crate alone catches that.
# Clippy with warnings denied: the workspace, then each crate alone.
lint:
    #!/usr/bin/env bash
    set -euo pipefail
    shopt -s nullglob
    cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
    for manifest in crates/*/Cargo.toml tools/*/Cargo.toml; do
        cargo clippy --locked --manifest-path "$manifest" --all-targets -- -D warnings
    done

# The standard's rules; `cargo xtask standard rules` lists them.
standard:
    @cargo xtask standard

# Arguments replace `--workspace`, such as `just test -p sourceorigin-standard`.
# The tests; arguments go on to nextest.
[positional-arguments]
test *ARGS='--workspace':
    @cargo nextest run --locked "$@"

# The scripts of .github/workflows/pr-checks.yml, under Node's test runner.
test-pr-checks:
    @node --test .github/pr-checks/*.test.js

# Rustdoc warnings, such as broken intra-doc links, fail the build.
doc:
    @RUSTDOCFLAGS='-D warnings' cargo doc --locked --workspace --no-deps

doc-test:
    @cargo test --locked --workspace --doc

# Advisories, licenses, bans and sources, as deny.toml configures them.
audit:
    @cargo deny --locked check

# Dependencies a crate declares but never names.
unused:
    @cargo machete

# CI only: needs lints/rust-toolchain.toml's nightly, cargo-dylint and
# dylint-link. Run from `cargo xtask`, RUSTUP_TOOLCHAIN would override the
# nightly; dylint-link names the library for its toolchain. The UI tests run
# one at a time: on a cold cache, each would build the Dylint driver at once.
# The Dylint lints: formatting, Clippy with warnings denied, the UI tests and
# the audit of lints/, then the lints on this repository, warnings denied.
lints:
    #!/usr/bin/env bash
    set -euo pipefail
    unset RUSTUP_TOOLCHAIN
    (
        cd lints
        export RUSTFLAGS='-C linker=dylint-link'
        cargo fmt -- --check
        cargo clippy --locked --lib --tests -- -D warnings
        cargo nextest run --locked --config-file ../.config/nextest.toml --test-threads 1
        cargo deny --locked check
    )
    DYLINT_RUSTFLAGS='-D warnings' cargo dylint --all -- --locked --all-targets

# tools/xtask/src/ci.rs maps each CI job to its recipes; only `lints` is left
# to CI, as it needs its own toolchain.
# CI's checks, for only what changed since the merge base with a branch
# (origin/main unless named), counting uncommitted and untracked files.
[positional-arguments]
check-changed *ARGS:
    @cargo xtask check-changed "$@"

# The crate's README is the written standard; a test fails while this is stale.
# Writes the rules' docs into the crate's README.
readme:
    @cargo xtask readme

# Packages the crate and builds it from its package, as crates.io would, without
# uploading anything. The owner publishes.
package:
    @cargo publish --locked --dry-run --allow-dirty --package sourceorigin-standard

# T3 Code often creates several worktrees at once, and a full-speed build in
# each saturates the machine.
# Builds the workspace and its tests at low priority on half the cores, unless
# CARGO_BUILD_JOBS is set.
setup-worktree:
    #!/usr/bin/env bash
    set -euo pipefail
    export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-$(( ($(nproc) + 1) / 2 ))}"
    exec nice -n 10 just build
