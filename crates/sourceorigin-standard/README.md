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

A repository adopts the standard in six steps:

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
   `budget.toml`, under `CODEOWNERS`, when it has `#[expect]` attributes,
   `// INVARIANT:`, `// wall-clock:` or `// EXTERNAL:` comments.
5. A workflow of its own calls this repository's pull request checks, as
   [below](#pull-request-checks) shows, on `main` (where GitHub reads it
   from), and its ruleset requires two
   checks: `ci`, for the code, and `standard / pr-checks`, for the pull
   request.
6. It runs `cargo xtask standard skills` and commits the
   [skills](#skills) it writes into `.agents/skills`.

## The rules

<!-- rules:start: generated from the docs of `Rule` by `just readme` -->
- `workspace-lints`: Every workspace crate's manifest has `[lints] workspace = true`; `[workspace.lints]` sets each lint the standard requires (the `rust` and `clippy` lints of its lint block) to `warn`, and every other lint to `warn` or `allow`.
- `rust-syntax`: Every Rust file git tracks or would track is UTF-8 text and parses as Rust.
- `config-syntax`: The crates' manifests, `budget.toml`, `deny.toml`, `rustfmt.toml`, `clippy.toml`, `rust-toolchain.toml`, `.config/nextest.toml`, `.cargo/config.toml`, every `.github` YAML file and `t3.json` parse, and `just --dump` reads the justfile. Each file a rule reads is UTF-8 text; one that isn't is one violation, and the rules go on. When `cargo metadata` can't read the workspace, the file cargo names, or else the root manifest, is the only violation, with cargo's whole message.
- `unsafe-modules`: `#[expect(unsafe_code, …)]` sits only inside the modules `standard.toml`'s `unsafe-modules` lists, such as `my_crate::ffi` or `build_script_build` (a target's root module is its crate's name), and every listed module holds one.
- `budget`: Each crate's `#[expect]` lints (each lint an `#[expect]` names, inner, outer, through `cfg_attr` or in a `macro_rules!` body) number exactly what `budget.toml`'s `[expect]` table gives it, and its lines starting `// INVARIANT:` exactly what `[invariant]` does, those starting `// wall-clock:` what `[wall-clock]` does and those starting `// EXTERNAL:` what `[external]` does, in the files its targets reach; each manifest's lints set to `allow` number what `[allow]` gives its path. Zero for one a table doesn't list. The crates of other workspaces in the repository count too, but not a Dylint library's UI cases, which are test fixtures.
- `generated-lints`: Rust files `.gitattributes` marks `linguist-generated` contain no lint attribute (`allow`, `expect`, `warn`, `deny` or `forbid`, inner or outer): their exceptions sit on their `mod` declaration.
- `rust-comments`: Rust files that aren't generated hold only doc comments (`///`, `//!`), `// SAFETY:` comments, `// INVARIANT:` comments, `// wall-clock: <why>` comments, which say why the test below has to poll or sleep, and `// EXTERNAL:` comments, which state a fact forced on the code by a dependency, platform, vendor or protocol it can't change, with a link to the issue, RFC or vendor document that shows it; any other `//` or `/* */` comment, or an `// EXTERNAL:` line whose part of the comment holds no link (`://`), fails.
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
- `workspace-dependencies`: Every dependency of a workspace crate, of every kind and target, is inherited with `workspace = true` from the root's `[workspace.dependencies]`, where each `path` dependency also has a `version` unless its crate sets `publish = false`. A crate's dependency on itself, as its tests take to turn on a feature, stays in its own manifest.
- `deny-config`: `deny.toml` sets `unknown-registry` and `unknown-git` under `[sources]`, and `wildcards` under `[bans]`, to `"deny"`, allows at least one license, and gives every ignored advisory a `reason`: `{ id = "…", reason = "…" }`.
- `anyhow-edges`: No library depends on `anyhow` (dev-dependencies aside), unless `standard.toml`'s `anyhow-crates` lists it as a binary's edge, such as a crate holding a CLI's commands. Each listed name is a workspace crate.
- `recipes`: The justfile has the recipes `validate`, `fmt`, `fmt-check`, `lint`, `standard`, `test`, `doc`, `doc-test`, `audit`, `unused` and `check-changed`. `just` alone runs `validate` or `check-changed`; `validate` runs `fmt` first (or `fmt-check`, so that it changes no files, but never both), then at least `lint`, `standard`, `test`, `doc`, `doc-test`, `audit` and `unused`; `test` takes `*ARGS`, defaulting to `--workspace`.
- `recipes-only`: Every command of `t3.json` runs `just`, and no line of a workflow's or composite action's `run:` starts with `cargo` unless `standard.toml`'s `cargo-in-ci` lists it; each listed line is one a `run:` holds.
- `t3-setup`: A recipe `t3.json` runs on worktree creation that runs Cargo, itself or through the recipes it runs, sets `CARGO_BUILD_JOBS` to a default, half the cores, that a set one overrides (or exports it only when `cargo config get build.jobs` finds no setting), and runs the build with `nice -n 10`, both in its own body.
- `ci-shape`: `.github/workflows/ci.yml` runs on `merge_group`, and has a `ci` job that runs `if: always()` and needs every other job but those that need it, directly or through other jobs, and so run after it, and those `standard.toml`'s `ci-watchers` lists, which watch it from outside, such as by polling the run; each listed name is a job: with `standard / pr-checks`, it is one of the repository's two required checks.
- `actions-pinned`: Every `uses:` of a workflow or composite action naming another repository's action or workflow pins it to a 40-character commit SHA, followed by a `# v…` comment giving its version.
- `workflow-permissions`: Every workflow's top-level `permissions` is exactly `contents: read`, or `{}`; a job that needs more sets its own.
- `job-timeouts`: Every job of a workflow sets `timeout-minutes`, except one calling a reusable workflow.
- `cache-saves`: Every `Swatinem/rust-cache` step's `save-if` is `false` or `${{ github.ref == 'refs/heads/main' }}`, and no composite action, nor workflow run by `pull_request`, `pull_request_target` or `merge_group`, uses `actions/cache`, which saves on a miss: they use `actions/cache/restore`, saving with `actions/cache/save` from `main`.
- `scheduled-audit`: A workflow runs `just audit` (or `cargo deny`) on a `schedule` at least weekly: a cron whose day of the month and month are `*`.
- `pr-template`: `.github/pull_request_template.md` has the headings `## What and why` and `## Testing`.
- `docs-summary`: With a `book.toml`, every Markdown page under its `src` is linked from its `SUMMARY.md`, or included in a page with `{{#include …}}`.
- `docs-images`: Every image in a Markdown file, outside code and the skills synced from this crate, has alt text: `![what it shows](…)`, or an `<img>` with a non-empty `alt`.
- `wording`: No Markdown file but a synced skill's holds, as a word of its prose (not code, a link's destination, an HTML tag or a URL, nor part of a name such as `my_lib`, `lib.rs` or `lib@example.com`), a spelling `standard.toml`'s `[wording]` rejects, such as `"Source Origin" = "SourceOrigin"`.
- `childless-mod-rs`: A module is `foo/mod.rs` only when its directory holds another Rust file, parsed or not; otherwise it is `foo.rs`. A `mod.rs` beside an integration test's, benchmark's or example's root, such as `tests/common/mod.rs`, passes, and so does one under `src/bin`, where binaries share modules.
- `inline-tests`: An inline `#[cfg(test)] mod … { }` (or one whose `cfg` requires a feature named for tests) at the top level of a file a target reaches outside test code spans fewer than 50 lines or at most 30% of the file's; a larger one is a file of its own, `mod tests;`.
- `inline-limits`: `Duration::from_*` with a number, or arithmetic on numbers, sits only in a `const` or `static` item, a `const fn` or test code: a limit, timeout or size is a named constant. Test code is what a test attribute such as `#[tokio::test]`, or a `cfg` requiring `test` or a feature named for tests (`test-util`), marks, the files such a module declares, and integration tests and benchmarks.
- `no-json-macro`: Code outside tests, in the crates `standard.toml`'s `no-json-macro` lists, invokes no `json!`: it serializes structs. Each name there is a workspace crate.
- `route-literals`: When `standard.toml`'s `route-modules` lists modules, such as `my_app::routes`, code outside them, tests and examples passes no string literal starting with `/` to `Router::route`, `route_service`, `nest` or `nest_service`, or to a `Redirect` constructor: it uses the paths those modules name. Each listed module is a file of a library or binary.
- `duplicate-items`: No function body outside test code with at least 20 tokens repeats another, token for token once its local bindings (parameters and the names its patterns bind) are renamed in the order they appear; paths, fields, methods and types stay as written.
- `tracing-fields`: Outside tests, no `tracing` event or span macro (`info!`, `info_span!`, `event!`, …) records a field whose name, or a run of its dotted parts, `standard.toml`'s `denied-fields` lists: `query` denies `db.query.text`, `url.query` only `url.query` and `http.url.query`. Nor does an `#[instrument]` function, in its `fields(…)` or as an argument it doesn't `skip`.
- `skills-synced`: Each skill in `.agents/skills` that comes from sourceorigin-standard (`standard`, `cleanup` and `docs`, and any other whose `SKILL.md` says it is synced) matches the crate's copy, file for file. `cargo xtask standard skills` writes them.
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

# The jobs of `.github/workflows/ci.yml` that watch the `ci` job from
# outside, such as one polling the run through the API to cancel it on a
# failure, which `ci` can't need without waiting on them forever.
ci-watchers = ["fail-fast"]

# The crates whose code outside tests serializes structs instead of invoking
# `json!`.
no-json-macro = ["my_protocol"]

# The modules that name the HTTP paths routers and redirects use; code
# elsewhere passes them no `/…` literal. Unset, paths aren't checked.
route-modules = ["my_app::routes"]

# The field names `tracing` events, spans and `#[instrument]` functions
# outside tests never record, as a name or a run of its dotted parts. Unset,
# fields aren't checked.
denied-fields = ["token", "secret", "cookie", "user_agent", "ip"]

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

# Spellings the docs reject, each with the one to write instead.
[wording]
"Source Origin" = "SourceOrigin"

[check-changed]
# Crates that build another without depending on it, such as a build
# script compiling a crate it embeds: a change to `built` reaches `builder`.
builds = [{ builder = "daemon", built = "dashboard" }]

[pr-checks]
# The pull request checks' settings, which they read and validate
# themselves (see below).
scopes = ["daemon", "cli"]
```

`budget.toml`, beside it, holds each crate's number of `#[expect]` lints
(an `#[expect(a, b)]` counts two), of `// INVARIANT:` lines, which state a
fact the code relies on that neither the types nor a test can hold, of
`// wall-clock:` lines, which say why the test below has to poll or sleep,
and of `// EXTERNAL:` lines, which state a fact forced on the code by a
dependency, platform, vendor or protocol it can't change, with a link that
shows it, in the files its targets reach, a crate of another workspace in the
repository (such as one on its own toolchain) included, but not a Dylint library's UI
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

[wall-clock]
piqueld = 4

[external]
piqueld = 1

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
  setting a budget of `budget.toml` to what the crate holds, moving a
  childless `foo/mod.rs` to `foo.rs` when that can't change what it reads,
  moving inline tests over their share of a file to a file of their own
  (`cargo fmt` then reflows the lines a move made shorter), removing a
  stale deviation or entry of one, or syncing the skills. It edits each
  TOML file as a document, so its layout and comments stay. A deviation
  keeps it from fixing what its scope excuses, and two fixes that conflict
  on one file stop it before it changes anything.
- `cargo xtask standard report` prints [`rules::report`] as JSON: each
  rule's violations and deviation, deviated or not, with the repository's
  `#[expect]` attributes by lint, its `// INVARIANT:` and `// EXTERNAL:`
  lines and the lints
  `[workspace.lints]` allows. The
  monthly cleanup pass aggregates them across repositories to find the rules
  causing the most friction.
- `cargo xtask standard rules` prints [`rules::describe`], the list above.
- `cargo xtask standard skills` runs [`rules::sync_skills`], which writes
  the crate's [skills](#skills) into `.agents/skills` and prints each file
  it changed.

[`Rule`]: https://docs.rs/sourceorigin-standard/latest/sourceorigin_standard/rules/enum.Rule.html
[`rules::check`]: https://docs.rs/sourceorigin-standard/latest/sourceorigin_standard/rules/fn.check.html
[`rules::fix`]: https://docs.rs/sourceorigin-standard/latest/sourceorigin_standard/rules/fn.fix.html
[`rules::report`]: https://docs.rs/sourceorigin-standard/latest/sourceorigin_standard/rules/fn.report.html
[`rules::describe`]: https://docs.rs/sourceorigin-standard/latest/sourceorigin_standard/rules/fn.describe.html
[`rules::sync_skills`]: https://docs.rs/sourceorigin-standard/latest/sourceorigin_standard/rules/fn.sync_skills.html

## Skills

The crate ships the agent skills every repository shares, so they are
written once and can't drift apart:

- `standard`: how an agent fixes a violation, uses `--fix`, and asks the
  user instead of working around a rule; and how to add a rule here.
- `cleanup`: the monthly cleanup pass. It measures the repository, ranks the
  rules by friction from `cargo xtask standard report`, finds the missing
  ones from the mistakes agents repeat, answers the
  judgement checks no rule can enforce, and runs one agent per area, with
  reviews, a boundary lane and a fix PR per bug.
- `docs`: how to write the user guide, the developer guide's recipes and
  doc comments, and the brief of the doc comment review a fresh subagent
  runs before a pull request opens.

`cargo xtask standard skills` writes each into `.agents/skills/<name>/`,
its `SKILL.md` saying it is synced and where to change it. The
`skills-synced` rule fails while a copy is missing or differs from the
crate's, or the crate no longer ships it, and `--fix` syncs them. What differs per
repository comes from the repository's own files: its issue tracker,
surfaces and hazards from `AGENTS.md`, its documentation page, and its areas
from the workspace's layout. Skills of the repository's own, without the
marker, are left alone.

## Pull request checks

Some rules are about a pull request itself rather than the code, so
`.github/workflows/pr-checks.yml` in this repository checks them, reading
the pull request through the GitHub API, without a checkout or a toolchain.
Each check is a job, and a finding says what to fix and how to challenge the
check: a deviation in `standard.toml`, or a change to the check here.

- `pr-title`: the title is a Conventional Commit, `type(scope)!: subject`,
  of type `feat`, `fix`, `refactor`, `docs`, `test`, `chore`, `ci`, `perf`,
  `build` or `revert`, and of a scope the repository allows, if any.
- `pr-body`: the description has every heading of the repository's
  `.github/pull_request_template.md`, each with more than the template's
  comments under it. An issue link is optional; one that closes an issue
  reads `Fixes #123`, or `Fixes PQD-123` where issues live in Linear.
- `no-co-authors`: neither the description nor a commit message has a
  `Co-authored-by` trailer.
- `fix-has-test`: a pull request whose title has the type `fix` changes a
  test (a `tests.rs`, a `tests/` directory, an added `#[test]`, or another
  language's test file), or carries the `visual` label and images in its
  description.
- `new-dependency`: a pull request that adds an entry to
  `[workspace.dependencies]`, or an exception to `deny.toml` (an ignored
  advisory, an allowed license, a skipped duplicate, an allowed source),
  has a `## Dependencies` section in its description saying why.
- `lint-exceptions`: a pull request that loosens a rule needs approval:
  any change to `standard.toml`, `clippy.toml`, `deny.toml`, `dylint.toml`
  or `.cargo/config.toml`, under its name or the one it is renamed from, and
  a budget of `budget.toml` that rises or appears. A change to
  `standard.toml` that only tightens it needs none: one that removes
  deviations, removes entries from a deviation's `paths` or `crates`, or
  rewords a reason, and changes nothing else. As each budget equals
  its count, every new `#[expect]`, `// INVARIANT:`, `// wall-clock:`,
  `// EXTERNAL:` or lint a manifest sets to `allow` shows as one. Approval is the
  `approved-exception` label, added by one of the `approvers` after the
  latest push: the newest entry of the head repository's activity log for
  the pull request's branch, which must have moved it to its head commit. Any later push, a force-push
  or a fast-forward back to an earlier commit included, needs it added
  again. With no such entry, as for a fork the job's token can't read, there
  is no approval. The job's
  summary lists every exception, approved or not.
- `frozen-files`: a file matching a glob of `frozen-files` that exists on
  the base branch, such as an applied migration, is not modified, renamed
  or deleted, and a new one starts with a comment saying what it changes. A
  deliberate change to an existing one needs the same approval as a lint
  exception. Only a pull request can check this: on `main` there is
  nothing to compare with.

A repository calls it from `.github/workflows/pr.yml`, pinned to a commit of
this repository's `main`, with the crate's version in the comment. It runs
on `pull_request_target`, so GitHub takes the workflow and its scripts from
the default branch, the checks read their settings at the pull request's
base, and a pull request can't loosen the checks it is checked by. That is
safe: no step checks out or runs the pull request's code, and the token
only reads. From December 2, 2026, a public repository runs it only once
an Actions event policy allows `pull_request_target` for `pr.yml`. Labels
re-run it, as some checks accept one:

```yaml
name: PR

on:
  pull_request_target:
    types: [opened, edited, synchronize, reopened, labeled, unlabeled]
  merge_group:

concurrency:
  group: ${{ github.workflow }}-${{ github.event.pull_request.number || github.ref }}
  cancel-in-progress: true

permissions:
  contents: read

jobs:
  standard:
    uses: sourceorigin-dev/standard/.github/workflows/pr-checks.yml@<sha> # v0.1.3
    permissions:
      contents: read
      pull-requests: read
      issues: read
```

Its settings are the `[pr-checks]` table of `standard.toml`, read at the
pull request's base; an unknown key or a value of the wrong kind fails
every check, naming its line:

```toml
[pr-checks]
# The scopes a title may take. Without them, a title takes none.
scopes = ["daemon", "cli", "web"]
# What precedes an issue's number where the repository's issues live: `#`
# by default, or a Linear team's key and a dash.
issue-prefix = "PQD-"
# Who may approve an exception with the `approved-exception` label.
approvers = ["PiquelChips"]
# `false` only reports exceptions, as a repository adopting the check
# starts; `true` by default.
enforce-lint-exceptions = true
# The files that never change once on the base branch.
frozen-files = ["crates/*/migrations/*.sql"]

# The checks the repository departs from, each with its reason: their
# findings pass as notices.
[pr-checks.deviations]
pr-title = "why the repository departs from it"
```

The repository's ruleset requires two checks: `ci`, the summary of
`ci.yml`, for the code, and `standard / pr-checks`, the summary of these
checks, for the pull request. They can't share one: editing a description
or labelling re-runs these checks, and `ci.yml` re-running on each would
either rebuild everything or replace a failed `ci` with one that skipped
the code. A merge group passes them, its pull requests having passed.
`CODEOWNERS` gives the owner the files that loosen a rule:

```text
/standard.toml @PiquelChips
/budget.toml   @PiquelChips
/clippy.toml   @PiquelChips
/deny.toml     @PiquelChips
```

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
included, call other `just` recipes than the table's. A recipe compares by
its path through the justfile's modules, which `just --dump` lists, and not
by its arguments: `just web test --fast` runs `web test`, which `just web`
doesn't. [`Ci::run`] checks the same before running anything.

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

/// `cargo xtask standard [--fix | report | rules | skills]` and
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
        ["standard", "skills"] => {
            for fix in rules::sync_skills(root())? {
                println!("synced {fix}");
            }
            Ok(ExitCode::SUCCESS)
        }
        ["check-changed", base @ ..] => {
            Ok(CI.run(root(), base.first().copied().unwrap_or(DEFAULT_BASE))?)
        }
        _ => Err("usage: cargo xtask (standard [--fix | report | rules | skills] | check-changed [base])".into()),
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
