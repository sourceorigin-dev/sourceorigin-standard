# sourceorigin-xtask's development tasks. `just` alone runs `validate`, the
# checks every change passes before review. CI runs the same recipes
# (.github/workflows/ci.yml).

default: validate

# Formats the sources, then runs every check CI runs.
validate: fmt lint test doc doc-test audit unused package

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

# Arguments replace `--workspace`, such as `just test -p sourceorigin-xtask`.
# The tests; arguments go on to nextest.
[positional-arguments]
test *ARGS='--workspace':
    @cargo nextest run --locked "$@"

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

# tools/xtask/src/ci.rs maps each CI job to its recipes; none is left to CI,
# as every one is quick.
# CI's checks, for only what changed since the merge base with a branch
# (origin/main unless named), counting uncommitted and untracked files.
[positional-arguments]
check-changed *ARGS:
    @cargo xtask check-changed "$@"

# Packages the crate and builds it from its package, as crates.io would, without
# uploading anything. The owner publishes; see docs/dev/rust-conventions.md.
package:
    @cargo publish --locked --dry-run --allow-dirty --package sourceorigin-xtask

# Writes target/worktree-setup/running (holding the PID) while it builds, then
# replaces it with `done`, or with `failed` holding the tail of the output.
# Prepares a new worktree: builds the workspace and its tests.
setup-worktree:
    #!/usr/bin/env bash
    set -euo pipefail
    markers=target/worktree-setup
    mkdir -p "$markers"
    exec 9> "$markers/lock"
    if ! flock -n 9; then
        echo "worktree setup is already running: wait with \`flock $markers/lock true\`" >&2
        exit 1
    fi
    echo $$ > "$markers/running"
    rm -f "$markers/done" "$markers/failed" "$markers/output.log"
    status=0
    # Nothing else inherits the lock, so it ends with this script.
    just build 9>&- 2>&1 | tee "$markers/output.log" 9>&- || status=$?
    if [ "$status" -eq 0 ]; then
        touch "$markers/done"
    else
        tail -n 40 "$markers/output.log" > "$markers/failed.tmp"
        mv "$markers/failed.tmp" "$markers/failed"
    fi
    rm -f "$markers/running"
    exit "$status"
