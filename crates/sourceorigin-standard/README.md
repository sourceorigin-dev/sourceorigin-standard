# sourceorigin-standard

The engineering standard of piqueld, sourceorigin and DirkEngine, as code.
Every rule below is a check that each repository runs with
`cargo xtask standard`, so the standard is what the checks enforce, not
prose that drifts beside the code.

Rules exist to help people and agents write better code, never to hold them
back. Every violation says how to fix it and how to challenge the rule: when
a rule gets in the way, stop and ask the user why, and with their approval
add a deviation, with its reason, to the repository's `standard.toml`, or
change the rule here, in the same pull request.

A repository adopts the standard in four steps:

1. Its `tools/xtask` depends on this crate from crates.io and runs
   [`rules::check`] as `cargo xtask standard`, as
   [below](#wiring-it-into-a-repositorys-xtask) shows.
2. `just standard` runs `cargo xtask standard`, and CI runs it in a job of
   its own on every change, since the rules read more than the Rust
   sources.
3. `[workspace.lints]` sets every lint the standard requires to `warn`, and
   `just lint` passes `-D warnings`; a lint outside that set may be `allow`
   there, in one reviewed place.
4. It commits `standard.toml` when it needs settings or deviations, and
   `budget.toml`, under `CODEOWNERS`, when it has `#[expect]` attributes or
   `// INVARIANT:` comments.

## The rules

<!-- rules:start: generated from the docs of `Rule` by `just readme` -->
- `workspace-lints`: Every workspace crate's manifest has `[lints] workspace = true`; `[workspace.lints]` sets each lint the standard requires (the `rust` and `clippy` lints of its lint block) to `warn`, and every other lint to `warn` or `allow`.
- `rust-syntax`: Every Rust file git tracks or would track parses as Rust.
- `config-syntax`: The crates' manifests, `budget.toml`, `deny.toml`, `rustfmt.toml`, `clippy.toml`, `rust-toolchain.toml`, `.config/nextest.toml`, `.cargo/config.toml`, every `.github` YAML file and `t3.json` parse, and `just --dump` reads the justfile. When `cargo metadata` can't read the workspace, the file it names is the only violation.
- `unsafe-modules`: `#[expect(unsafe_code, …)]` sits only inside the modules `standard.toml`'s `unsafe-modules` lists, such as `my_crate::ffi` or `build_script_build` (a target's root module is its crate's name), and every listed module holds one.
- `budget`: Each crate's `#[expect]` lints (each lint an `#[expect]` names, inner, outer, through `cfg_attr` or in a `macro_rules!` body) number exactly what `budget.toml`'s `[expect]` table gives it, and its lines starting `// INVARIANT:` exactly what `[invariant]` does, in the files its targets reach; each manifest's lints set to `allow` number what `[allow]` gives its path. Zero for one a table doesn't list. The crates of other workspaces in the repository count too, but not a Dylint library's UI cases, which are test fixtures.
- `generated-lints`: Rust files `.gitattributes` marks `linguist-generated` contain no lint attribute (`allow`, `expect`, `warn`, `deny` or `forbid`, inner or outer): their exceptions sit on their `mod` declaration.
- `rust-comments`: Rust files that aren't generated hold only doc comments (`///`, `//!`), `// SAFETY:` comments and `// INVARIANT:` comments; any other `//` or `/* */` comment fails.
- `doc-length`: The prose of each item's or module's docs, in a file that isn't generated, spans at most `standard.toml`'s `[comments] doc-lines` lines (10 by default). Code blocks and a crate root's `//!` docs don't count.
- `config-comments`: In every `Cargo.toml`, justfile (and `*.just`) and `.github` YAML file, each block of `#` comment lines spans at most `standard.toml`'s `[comments] block-lines` (6 by default), and the file holds at most `[comments] file-lines` comment lines (40 by default).
- `workspace-manifest`: The root manifest's `[workspace]` sets `resolver = "3"`, its `[workspace.package]` sets `edition = "2024"`, and every workspace crate inherits it with `edition.workspace = true`.
- `layout`: Every workspace crate lives under `crates/`, `apps/` or `tools/`, the `xtask` crate in `tools/xtask`, and `.cargo/config.toml` aliases `cargo xtask` to `run --locked --package xtask --`.
- `cargo-locked`: Every `cargo build`, `check`, `clippy`, `doc`, `llvm-cov`, `nextest`, `publish`, `run` and `test` run by a justfile's recipe (and a `*.just` file's), a workflow's or composite action's `run:`, or a `t3.json` command passes `--locked` (or `--frozen`). Cargo runs only as a command word, after any `NAME=value`, `env`, `exec`, `time` or `nice -n N`; quoted text and a heredoc's body are no command, and a folded `run: >` block is one.
- `agent-files`: `AGENTS.md` sits at the root, and no `CLAUDE.md` exists. Each skill is `.agents/skills/<name>/SKILL.md`, its frontmatter's `name` is `<name>`, and `.claude/skills` is a symlink to `../.agents/skills`.
- `agents-md`: AGENTS.md names every workspace crate, by its name in backticks or its directory, and holds at most 150 lines.
- `toolchain`: `rust-toolchain.toml` pins `channel = "stable"` and installs at least the `rustfmt` and `clippy` components.
- `rustfmt-config`: `rustfmt.toml` sets `edition = "2024"`, `newline_style = "Unix"` and `use_field_init_shorthand = true`.
- `clippy-config`: `clippy.toml` sets `allow-expect-in-tests`, `allow-panic-in-tests` and `allow-dbg-in-tests` to `true`, and never `allow-unwrap-in-tests`.
- `nextest-config`: `.config/nextest.toml`'s default profile sets a `slow-timeout`, every `slow-timeout` in it has a `terminate-after`, and no profile or override sets `retries`.
- `workspace-dependencies`: Every dependency of a workspace crate, of every kind and target, is inherited with `workspace = true` from the root's `[workspace.dependencies]`, where each `path` dependency also has a `version`.
- `deny-config`: `deny.toml` sets `unknown-registry` and `unknown-git` under `[sources]`, and `wildcards` under `[bans]`, to `"deny"`, allows at least one license, and gives every ignored advisory a `reason`: `{ id = "…", reason = "…" }`.
- `anyhow-edges`: No library depends on `anyhow` (dev-dependencies aside), unless `standard.toml`'s `anyhow-crates` lists it as a binary's edge, such as a crate holding a CLI's commands. Each listed name is a workspace crate.
- `recipes`: The justfile has the recipes `validate`, `fmt`, `fmt-check`, `lint`, `standard`, `test`, `doc`, `doc-test`, `audit`, `unused` and `check-changed`. `just` alone runs `validate` or `check-changed`; `validate` runs `fmt` first, then at least `lint`, `standard`, `test`, `doc`, `doc-test`, `audit` and `unused`, and never `fmt-check`; `test` takes `*ARGS`, defaulting to `--workspace`.
- `recipes-only`: Every command of `t3.json` runs `just`, and no line of a workflow's or composite action's `run:` starts with `cargo` unless `standard.toml`'s `cargo-in-ci` lists it; each listed line is one a `run:` holds.
- `t3-setup`: A recipe `t3.json` runs on worktree creation that runs Cargo, itself or through the recipes it runs, sets `CARGO_BUILD_JOBS` to a default, half the cores, that a set one overrides, and runs the build with `nice -n 10`, both in its own body.
<!-- rules:end -->

Each rule is a variant of [`Rule`]: its name is the one `standard.toml` and
violations use, and its doc is the text above.

## Configuration

A repository applies the standard in `standard.toml` at its root. Every key
is optional, and a missing file means the defaults. An unknown key, or a
rule name that doesn't exist, fails with the parser's message naming it.

```toml
# The modules that may expect `unsafe_code`, on the smallest item inside
# them that needs it. A target's root module is its crate's name, such as
# `build_script_build` for a build script.
unsafe-modules = ["my_crate::ffi"]

# The libraries that may depend on `anyhow`, each a binary's edge, such as
# a crate holding a CLI's commands. A crate with no library may anyway.
anyhow-crates = ["my-cli"]

# The lines of workflows' `run:` that run Cargo rather than a recipe, each
# whole, such as a check on the minimum supported Rust version.
cargo-in-ci = ["cargo +1.85 check --locked --workspace"]

# Rules the repository departs from, each with its reason: everywhere, or
# only in the files `paths` matches and the crates `crates` names. A
# deviation, or an entry of its scope, that excuses no violation is one.
[deviations]
rule-name = "why the repository departs from it everywhere"
other-rule = { reason = "why", paths = ["crates/generated/**"], crates = ["vendored"] }

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

`budget.toml`, beside it, holds each crate's number of `#[expect]` lints
(an `#[expect(a, b)]` counts two) and of `// INVARIANT:` lines, which state
a fact the code relies on that neither the types nor a test can hold, in
the files its targets reach, a crate of another workspace in the repository
(such as one on its own toolchain) included, but not a Dylint library's UI
cases, whose `#[expect]`s are what they test. It also holds each manifest's
number of lints set to `allow`, by its path. Each count must equal its
budget: more fails until the number is raised, which the owner reviews
through `CODEOWNERS`, and fewer fails until it is lowered, which `--fix`
does with no approval needed. So every new exception shows as a raised
number. A crate or manifest a table doesn't list has none:

```toml
[expect]
piqueld = 3
piquelctl = 1

[invariant]
piqueld = 2

[allow]
"Cargo.toml" = 1
```

## Commands

- `cargo xtask standard`, which `just standard` and CI run, runs
  [`rules::check`] and prints each violation with its location, rule and
  fix, then how to challenge the rule, suggesting a deviation scoped to its
  file.
- `cargo xtask standard --fix` first runs [`rules::fix`], which makes the
  fixes that need no judgement and prints each, such as adding
  `[lints] workspace = true`, making a workspace lint at `deny` or `forbid`
  `warn` (never an `allow`), adding a required lint the workspace lacks,
  making a crate inherit the workspace's edition,
  setting a budget of `budget.toml` to what the crate holds, or removing a
  stale deviation or entry of one. It edits each
  TOML file as a document, so its layout and comments stay. A deviation
  keeps it from fixing what its scope excuses, and two fixes that conflict
  on one file stop it before it changes anything.
- `cargo xtask standard report` prints [`rules::report`] as JSON: each
  rule's violations and deviation, deviated or not, with the repository's
  `#[expect]` attributes by lint, its `// INVARIANT:` lines and the lints
  `[workspace.lints]` allows. The
  monthly cleanup pass aggregates them across repositories to find the rules
  causing the most friction.
- `cargo xtask standard rules` prints [`rules::describe`], the list above.

[`Rule`]: https://docs.rs/sourceorigin-standard/latest/sourceorigin_standard/rules/enum.Rule.html
[`rules::check`]: https://docs.rs/sourceorigin-standard/latest/sourceorigin_standard/rules/fn.check.html
[`rules::fix`]: https://docs.rs/sourceorigin-standard/latest/sourceorigin_standard/rules/fn.fix.html
[`rules::report`]: https://docs.rs/sourceorigin-standard/latest/sourceorigin_standard/rules/fn.report.html
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
rules, and describes its CI for `check-changed`. Its jobs carry the keys
`ci.yml` gives them, each filter its name in `.github/filters.yml`, and the
`lint`, `test` and `doc-test` recipes take `--package=<crate>` arguments:

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
                recipe(&["just", "lint"], Run::Crates { libraries_only: false }),
                recipe(&["just", "test"], Run::Crates { libraries_only: false }),
                recipe(&["just", "doc-test"], Run::Crates { libraries_only: true }),
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

/// `cargo xtask standard [--fix | report | rules]` and
/// `cargo xtask check-changed [base]`. A real xtask parses its tasks with
/// clap or by hand, and wraps errors with anyhow's context.
fn main() -> Result<ExitCode, Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["standard", rest @ ..] if matches!(rest, [] | ["--fix"]) => {
            if rest == ["--fix"] {
                for fix in rules::fix(root())? {
                    println!("fixed {fix}");
                }
            }
            let violations = rules::check(root())?;
            for violation in &violations {
                eprintln!("{violation}");
            }
            Ok(if violations.is_empty() { ExitCode::SUCCESS } else { ExitCode::FAILURE })
        }
        ["standard", "report"] => {
            println!("{}", rules::report(root())?.to_json()?);
            Ok(ExitCode::SUCCESS)
        }
        ["standard", "rules"] => {
            print!("{}", rules::describe());
            Ok(ExitCode::SUCCESS)
        }
        ["check-changed", base @ ..] => {
            Ok(CI.run(root(), base.first().copied().unwrap_or(DEFAULT_BASE))?)
        }
        _ => Err("usage: cargo xtask (standard [--fix | report | rules] | check-changed [base])".into()),
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
