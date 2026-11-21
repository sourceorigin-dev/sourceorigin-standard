# sourceorigin-standard

The engineering standard of piqueld, sourceorigin and DirkEngine, as code. Each of those repositories'
`tools/xtask` depends on this crate from crates.io and runs its checks, so a
rule fails a check instead of living in prose. A rule is enforced by the
first of these that can express it: types, rustc and Clippy settings, a check
in this crate, a CI check on the pull request, a cleanup-pass skill.

- `crates/sourceorigin-standard`: the published library. Its README is its
  documentation and shows how a repository's xtask uses it.

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
- **Shared machine:** build with `CARGO_BUILD_JOBS=3` and `nice -n 10`;
  other agents compile on the same cores.

## Conventions

This repository follows the standard it ships: the crate's README states
its rules, and the checks enforce them here as everywhere. Library code
returns `thiserror` errors that keep their sources; `anyhow` stays in
`tools/xtask`.

## Validation

`just validate` (or `just` alone) formats the code and runs the lints, tests,
docs, audits and the publish dry run. The workspace is small, so run it
before handing work back. While iterating, run only what you changed, such as
`just test -p sourceorigin-standard`. If a step can't run, report the exact
command and why.

A new worktree runs `just setup-worktree`, which builds the workspace at low
priority on half the cores.

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

The crate's README is its documentation (`#![doc = include_str!(...)]`).
Documentation changes in the same pull request as the code.

Read a dependency's real source, at the version `Cargo.lock` pins, in
`~/.cargo/registry/src`, rather than trusting what you remember of its API.

If a rule here fights the task, say so plainly, and get the owner's sign-off
before breaking it.
