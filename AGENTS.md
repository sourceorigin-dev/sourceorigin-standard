# sourceorigin-xtask

A library of the `cargo xtask` checks the
[engineering standard](https://gist.github.com/PiquelChips/04330e52c03efbe9ce4c889c88673a27)
asks of piqueld, sourceorigin and DirkEngine. Each of those repositories'
`tools/xtask` depends on it from crates.io and keeps only what is specific to
it. The crate holds everything generic, so the three never drift apart.

- `crates/sourceorigin-xtask`: the published library. Its README is its
  documentation and shows how a repository's xtask uses it.
- `tools/xtask`: this repository's own `cargo xtask`, which uses the crate
  as the other repositories do.

Terms that are easy to mix up:

- **Filter:** a named list of path globs in a repository's
  `.github/filters.yml`, which CI's `changes` job matches with
  dorny/paths-filter. **Output:** a `changes` job output, built from one or
  more filters, that a job's `if:` reads.
- **Job and recipe:** a job is a job of the repository's `ci.yml`; a recipe
  is one of the `just` commands it runs.

## Hazards

Other agents work in parallel worktrees on this machine, and the developer
uses it too. Before anything destructive, ask what else it touches.

- **Never publish to crates.io** (`cargo publish` without `--dry-run`). A
  version can't be deleted, only yanked, and three repositories depend on
  it: the owner publishes. `just package` is the dry run.
- **Never change piqueld, sourceorigin or DirkEngine from here**, even to try
  the crate out: they move to a new version in their own pull requests.
- **Processes:** stop only a process you started, by the PID you captured.
  Never kill by name or pattern (`pkill -f`, `killall`): your own agent
  process has the worktree's path in its arguments, and other worktrees run
  the same binaries.
- **Shared machine:** set `CARGO_BUILD_JOBS=4` before building; other
  agents share the cores.

## Conventions

Follow `docs/dev/rust-conventions.md`, which applies the engineering
standard to this repository.

## Validation

`just check-changed [branch]` runs CI's checks for only what changed since
the merge base with `origin/main` (or the branch named), counting uncommitted
and untracked files; it is this crate's own feature, through `tools/xtask`.
Run it before handing work back. `just validate` (or `just` alone) formats
the code and runs every check: run it when a change is workspace-wide or
when asked. While iterating, run only what you changed, such as
`just test -p sourceorigin-xtask`. If a step can't run, report the exact
command and why.

`just lint` also runs `cargo xtask test-waits`, which fails on a sleep in a
test that neither runs on paused tokio time nor says why on the line above
(`// wall-clock: <why>`); docs/dev/rust-conventions.md has the rule.

`tools/xtask/src/ci.rs` describes `.github/workflows/ci.yml` for
`check-changed`: change the two together, as its `ci_matches_the_workflow`
test fails until they agree.

A new worktree runs `just setup-worktree`, which builds the workspace. While
`target/worktree-setup/running` names a live process, wait for it. If
`target/worktree-setup/failed` exists, read it before anything else. With no
marker, or a `running` whose process is gone, run `just setup-worktree`
yourself.

## Surfaces

A change reaches every place it applies to. Before handing one back, walk
this list and say which entries applied:

- the public API, with a doc comment on every public item;
- the crate's README, whose example is how a repository's xtask wires it in;
- the pull request's note of what each repository's xtask must change when
  the API breaks;
- this repository's own `tools/xtask`, which uses the crate as the others do.

## Issues, commits & pull requests

Issues live in GitHub Issues. Titles of issues and pull requests follow
Conventional Commits and say what changes for whoever uses the crate; pull
requests are squash-merged, so the title becomes the commit on `main`. No
`Co-authored-by` trailers.

Keep pull requests small and focused: one change each, which a reviewer can
read in one sitting. Split refactors from behaviour changes, and stack
dependent pull requests, each on the last. Open them as drafts while
iterating.

## Documentation

The crate's README is its documentation (`#![doc = include_str!(...)]`), and
`docs/dev/` holds the contributor guide. Documentation changes in the same
pull request as the code.

Read a dependency's real source, at the version `Cargo.lock` pins, in
`~/.cargo/registry/src`, rather than trusting what you remember of its API.

If a rule here fights the task, say so plainly, and get the owner's sign-off
before breaking it.
