# sourceorigin-standard

The engineering standard of piqueld, sourceorigin and DirkEngine, as code.
Each repository's `cargo xtask` depends on this crate and runs its checks, so
a rule fails a check instead of living in prose that drifts.

## The rules

Each rule is a variant of [`Rule`], with a stable name and a doc saying what
it enforces. [`rules::check`] runs every rule over the repository and returns
its violations, each printed with its location, rule and fix, then how to
challenge the rule: a deviation in the repository's `standard.toml`, or a
change to the rule here, after asking the user. The
repository's `cargo xtask standard` prints them and fails; `just standard`
runs it, and so does a CI job on every change, since the rules read more
than the Rust sources. [`rules::fix`], behind `cargo xtask standard --fix`,
first makes the fixes that need no judgement and prints each: it adds
`[lints] workspace = true`, makes a workspace lint at `deny` or `forbid`
`warn` (never an `allow`), lowers a ceiling of `budget.toml` to what the
crate holds, and removes a stale deviation or entry of one. It edits each
TOML file as a document, so its layout and comments stay.
[`rules::describe`] lists the rules with their docs as Markdown, which
`cargo xtask standard rules` prints.

A repository applies the standard in `standard.toml` at its root. Every key
is optional, and a missing file means the defaults. An unknown key, or a
rule name that doesn't exist, fails with the parser's message naming it.

```toml
# Rules the repository departs from, each with its reason: everywhere, or
# only in the files `paths` matches and the crates `crates` names. A
# deviation, or an entry of its scope, that excuses no violation is one.
[deviations]
rule-name = "why the repository departs from it everywhere"
other-rule = { reason = "why", paths = ["crates/generated/**"], crates = ["vendored"] }

# The modules that may expect `unsafe_code`, on the smallest item inside
# them that needs it. A target's root module is its crate's name, such as
# `build_script_build` for a build script.
unsafe-modules = ["my_crate::ffi"]

[comments]
# The most lines of prose an item's or module's docs may span, code blocks
# and a crate root's `//!` docs aside.
doc-lines = 10
# The most lines a `#` comment in a manifest, justfile or `.github` YAML
# file may span, and the most comment lines such a file may hold.
block-lines = 6
file-lines = 40

[check-changed]
# Crates that build another without depending on it, such as a build
# script compiling a crate it embeds: a change to `built` reaches `builder`.
builds = [{ builder = "daemon", built = "dashboard" }]
```

`budget.toml`, beside it, holds each crate's ceiling of `#[expect]`
attributes and of `// INVARIANT:` comments, which state a fact the code
relies on that neither the types nor a test can hold. More than the ceiling
fails; fewer passes. Only raising a ceiling needs the owner's review,
through `CODEOWNERS`. A crate a table doesn't list may have none:

```toml
[expect]
piqueld = 3
piquelctl = 1

[invariant]
piqueld = 2
```

[`Rule`]: https://docs.rs/sourceorigin-standard/latest/sourceorigin_standard/rules/enum.Rule.html
[`rules::check`]: https://docs.rs/sourceorigin-standard/latest/sourceorigin_standard/rules/fn.check.html
[`rules::fix`]: https://docs.rs/sourceorigin-standard/latest/sourceorigin_standard/rules/fn.fix.html
[`rules::describe`]: https://docs.rs/sourceorigin-standard/latest/sourceorigin_standard/rules/fn.describe.html

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
  on them, from `cargo metadata`, plus the `builds` edges of
  `standard.toml`. A path the filters cover but no crate owns, such as `Cargo.lock`,
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

The repository's `tools/xtask` depends on the crate from crates.io, runs the
rules, and describes its CI for `check-changed`. Its jobs carry the keys `ci.yml` gives them, and each
filter its name in `.github/filters.yml`:

```rust,no_run
use std::path::Path;
use std::process::ExitCode;

use sourceorigin_standard::check_changed::{self, Ci, DEFAULT_BASE, Job, Recipe, Run, Trigger};
use sourceorigin_standard::rules;

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
};

fn root() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}

/// `cargo xtask standard`, `cargo xtask standard rules` and
/// `cargo xtask check-changed [base]`. A real xtask parses its tasks with
/// clap or by hand, and wraps errors with anyhow's context.
fn main() -> Result<ExitCode, Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["standard"] => {
            let violations = rules::check(root())?;
            for violation in &violations {
                eprintln!("{violation}");
            }
            Ok(if violations.is_empty() { ExitCode::SUCCESS } else { ExitCode::FAILURE })
        }
        ["standard", "rules"] => {
            print!("{}", rules::describe());
            Ok(ExitCode::SUCCESS)
        }
        ["check-changed", base @ ..] => {
            Ok(CI.run(root(), base.first().copied().unwrap_or(DEFAULT_BASE))?)
        }
        _ => Err("usage: cargo xtask (standard [rules] | check-changed [base])".into()),
    }
}

/// Fails until the table and ci.yml change together.
#[test]
fn ci_matches_the_workflow() {
    CI.check_workflow(root()).expect("the CI table matches ci.yml and filters.yml");
}
```

Its justfile runs the rules as `just standard`, which CI runs in a job of
its own on every change, and `check-changed` as `just check-changed`:

```just
# The standard's rules; `cargo xtask standard rules` lists them.
standard:
    @cargo xtask standard

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
