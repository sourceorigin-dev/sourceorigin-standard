# sourceorigin-standard

The engineering standard of piqueld, sourceorigin and DirkEngine, as code.
Each repository's `cargo xtask` depends on this crate and runs its checks, so
a rule fails a check instead of living in prose that drifts.

## `check-changed`

Runs CI's quick checks for only what changed since the merge base with
`origin/main`, counting uncommitted and untracked files, so a developer or
agent can hand work back without running the whole suite or guessing.

- It reads `.github/filters.yml`, the path filters CI's `changes` job passes
  to dorny/paths-filter, and matches the changed paths as paths-filter does:
  picomatch globs with `dot: true`, YAML anchors resolved, `!` negating a
  pattern, and the `predicate-quantifier` (`some`, `every` or
  `some-with-excludes`) of the workflow's paths-filter step.
- It runs the recipes of each job the matched filters start. Recipes that
  take crates run on the crates the change touches and every crate depending
  on them, from `cargo metadata`, plus any build edges the repository
  declares. A path the filters cover but no crate owns, such as `Cargo.lock`,
  widens them to the whole workspace.
- It leaves slow jobs to CI and prints their commands, prints a summary of
  every check (passed, FAILED, skipped or left to CI, with the reason), and
  fails if a check failed.

The repository describes its CI as a typed table: a [`Filter`] enum naming
the filters, and a [`Ci`] of [`Job`]s, each with the filters that start it
and the [`Recipe`]s it runs. Loading `.github/filters.yml` fails when it names
a filter the enum doesn't, or misses one it does. [`Ci::check_workflow`],
called from a unit test, fails when the table and
`.github/workflows/ci.yml` disagree: a job in neither the table nor its
`ignored` list, a job whose `if:` reads the `changes` outputs of other
filters than its trigger names, or one whose `run:` scripts, matrix entries
included, call other `just` recipes than the table's. [`Ci::run`] checks the same before running anything.

[`Filter`]: https://docs.rs/sourceorigin-standard/latest/sourceorigin_standard/check_changed/trait.Filter.html
[`Ci`]: https://docs.rs/sourceorigin-standard/latest/sourceorigin_standard/check_changed/struct.Ci.html
[`Job`]: https://docs.rs/sourceorigin-standard/latest/sourceorigin_standard/check_changed/struct.Job.html
[`Recipe`]: https://docs.rs/sourceorigin-standard/latest/sourceorigin_standard/check_changed/struct.Recipe.html
[`Ci::check_workflow`]: https://docs.rs/sourceorigin-standard/latest/sourceorigin_standard/check_changed/struct.Ci.html#method.check_workflow
[`Ci::run`]: https://docs.rs/sourceorigin-standard/latest/sourceorigin_standard/check_changed/struct.Ci.html#method.run

## Wiring it into a repository's xtask

The repository's `tools/xtask` depends on the crate from crates.io and
describes its CI. Its jobs carry the keys `ci.yml` gives them, and each
filter its name in `.github/filters.yml`:

```rust,no_run
use std::path::Path;
use std::process::ExitCode;

use sourceorigin_standard::check_changed::{self, Builds, Ci, DEFAULT_BASE, Job, Recipe, Run, Trigger};

/// A filter of .github/filters.yml.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Filter {
    Rust,
    Deny,
    Ui,
}

impl check_changed::Filter for Filter {
    const ALL: &'static [Self] = &[Self::Rust, Self::Deny, Self::Ui];

    fn name(self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::Deny => "deny",
            Self::Ui => "ui",
        }
    }
}

const fn recipe(command: &'static [&'static str], run: Run) -> Recipe {
    Recipe { command, run }
}

/// The jobs of .github/workflows/ci.yml.
const CI: Ci<Filter> = Ci {
    jobs: &[
        Job {
            id: "checks",
            trigger: Trigger::AnyOf(&[Filter::Rust, Filter::Deny]),
            recipes: &[
                recipe(&["just", "fmt-check"], Run::Workspace),
                recipe(&["just", "audit"], Run::Workspace),
            ],
        },
        Job {
            id: "rust",
            trigger: Trigger::AnyOf(&[Filter::Rust]),
            recipes: &[
                // Each takes `--package=<crate>` arguments.
                recipe(&["just", "lint"], Run::Crates { libraries_only: false }),
                recipe(&["just", "test"], Run::Crates { libraries_only: false }),
                recipe(&["just", "doc-test"], Run::Crates { libraries_only: true }),
                // It checks files generated from the daemon.
                recipe(&["just", "openapi-check"], Run::Only(&["daemon"])),
            ],
        },
        Job {
            id: "browser",
            trigger: Trigger::AnyOf(&[Filter::Ui]),
            recipes: &[recipe(&["just", "test-browser"], Run::InCi("slow, so drafts skip it too"))],
        },
        Job {
            id: "docs",
            trigger: Trigger::EveryChange,
            recipes: &[recipe(&["just", "docs"], Run::Workspace)],
        },
    ],
    // The summary job, which runs no check.
    ignored: &["ci"],
    // The daemon's build script compiles the dashboard it embeds.
    builds: &[Builds { builder: "daemon", built: "dashboard" }],
};

fn root() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}

/// `cargo xtask check-changed [base]`. A real xtask parses its tasks with
/// clap or by hand, and wraps the error with anyhow's context.
fn main() -> Result<ExitCode, check_changed::Error> {
    let base = std::env::args().nth(2);
    CI.run(root(), base.as_deref().unwrap_or(DEFAULT_BASE))
}

/// Fails until the table and ci.yml change together.
#[test]
fn ci_matches_the_workflow() {
    CI.check_workflow(root()).expect("the CI table matches ci.yml and filters.yml");
}
```

Its justfile runs it as `just check-changed`:

```just
# CI's quick checks, for only what changed since the merge base with a branch.
[positional-arguments]
check-changed *ARGS:
    @cargo xtask check-changed "$@"
```

## Requirements

- `.github/workflows/ci.yml` has a `changes` job whose dorny/paths-filter
  step has an `id` and reads `filters: .github/filters.yml`. The other jobs
  read its outputs as `needs.changes.outputs.<name>`, and each output reads
  the filters it is built from as `steps.<id>.outputs.<filter>`.
- The filters are globs or lists of them. paths-filter's maps from change
  types to globs (`added: '**'`) and picomatch's extglobs (`!(docs)/**`) are
  rejected rather than matched differently.
- The repository's root is its Cargo workspace's root, and `just` and `git`
  are installed.
