---
name: propagate
description: >
  Release a change to sourceorigin-standard and carry it into every repository
  that uses the standard: publish the crate, then in each consumer bump the
  version, repin the PR checks, resync the skills, fix or deviate what the
  release now flags, drop deviations it makes unnecessary, and turn on what it
  adds. Use after a change to the crate, its skills, its lints or
  `pr-checks.yml` merges, or when asked to propagate, roll out or adopt a new
  version of the standard.
---

# Propagate a release

> **Scope.** Do only what this skill's task asks. No drive-by refactors,
> renames, reformatting or extra features. Note anything else in your report
> or in an issue instead of fixing it. Ask the owner before widening the
> change. Reread your diff before handing back and drop every hunk the task
> doesn't need. See "Scope" in `AGENTS.md`.

A change to the standard is done when every consumer runs it. This skill takes
a merged change from `main` to a published release, then to one pull request
per consumer.

## The consumers

| Repository | Clone | Merges | Issues |
| --- | --- | --- | --- |
| `sourceorigin-dev/piqueld` | `~/Projects/piqueld` | squash, merge queue | Linear `PQD` team: `Part of PQD-123` |
| `sourceorigin-dev/sourceorigin` | `~/Projects/sourceorigin` | squash, merge queue | GitHub Issues |
| `sourceorigin-dev/origin` | `~/Projects/origin` | squash | GitHub Issues |
| `DirkEngine/DirkEngine` | `~/Projects/DirkEngine` | squash | GitHub Issues |

Each one wires the crate the same way:
- `tools/xtask/Cargo.toml` depends on `sourceorigin-standard`;
- `.github/workflows/pr.yml` calls `pr-checks.yml` at a pinned commit;
- `standard.toml` and `budget.toml` hold its settings, deviations and budgets;
- `.agents/skills` holds the synced skills.

Before you start, read each consumer's `standard.toml`, its `AGENTS.md`, and
its open pull requests that touch those files. They are the source of truth;
this table only says where to look. If a repository runs the standard and
isn't listed, add it to the table in the same pull request as this release.
DirkEngine takes the generic rules only, until its API overhaul.

## 1. Release

1. **Classify the change:**
   - a fix, or a rule accepting more: a patch;
   - a stricter or new rule, or a new setting: still a patch while at 0.x, but
     consumers will get new findings;
   - a change to the public API: consumers' xtask code must change.
2. **Open a release pull request:** bump `version` in the root `Cargo.toml`,
   run `cargo update -w`, and point the README's caller-workflow comment at the
   new tag. The description is the release notes. For each consumer, list what
   it must do: new findings, deviations it can drop, settings to turn on, and
   API changes.
3. **After the owner merges it**, publish only on the owner's go-ahead, from a
   clean `main` equal to `origin/main`:
   `cargo publish -p sourceorigin-standard --locked`
4. **Tag the release commit** with an annotated tag `vX.Y.Z` and push it. Note
   its full commit hash: the consumers pin `pr-checks.yml` to it.

## 2. Each consumer

Work in a worktree from the consumer's `origin/main`. One draft pull request
per consumer, titled like
`chore(standard): move to sourceorigin-standard X.Y.Z` (use the consumer's
allowed scopes).

If the consumer has an open stack that changes `standard.toml` or
`budget.toml`, such as an adoption or a sweep, put the bump on top of that
stack instead, and link it with `gh stack link`. Never rebase someone else's
stack without the owner.

1. **Bump:** set `sourceorigin-standard = "X.Y.Z"` in `tools/xtask/Cargo.toml`,
   then run `cargo update -p sourceorigin-standard --precise X.Y.Z`.
2. **Repin the PR checks:** set `pr.yml`'s
   `uses: sourceorigin-dev/standard/.github/workflows/pr-checks.yml@<hash> # vX.Y.Z`
   to the release commit.
3. **Resync the skills:** `cargo xtask standard skills`.
4. **Apply the API changes** the release notes list, in the xtask.
5. **Check:** run `cargo xtask standard --fix`, then `just standard`. For each
   finding that is left:
   - **small:** fix it in this pull request;
   - **large:** add a deviation in `standard.toml`, scoped to the exact
     `paths` or `crates`, with the reason "N findings, fixed in a follow-up".
     Open the follow-up sweep, or list it in your report;
   - **doesn't fit the repository:** this is a rule challenge. Ask the owner,
     with the rule, the finding and why. Never work around a rule.
6. **Drop what the release makes unnecessary.** A deviation that excuses
   nothing is itself a violation, so `just standard` names every one that is
   now stale. Delete them, and lower `budget.toml` with `--fix`.
7. **Turn on what the release adds.** A new setting, rule, budget table or
   check that is opt-in needs a decision. Propose a value from the repository's
   code, and ask the owner when it changes behaviour, which code it allows, or
   who approves what.
8. **Dylint lints**, once the consumer uses them: bump the `rev` pinned in its
   Dylint configuration to the release commit.
9. **Verify:** `just check-changed` and `just standard` pass locally.

Open the pull request with the `file-pr` skill, using the consumer's template
headings: a draft, with no labels and no `Co-authored-by` trailers. Link the
consumer's issue where its `AGENTS.md` asks for one.

`lint-exceptions` asks for the owner's `approved-exception` label when the pull
request adds or widens an exception, or raises a budget. Never add the label
yourself unless the owner says so. Wait for CI with `watch_pull_request`, then
unwatch.

## 3. Report

One row per consumer:
- the pull request's URL and head;
- its CI state, only once CI has really finished;
- the deviations dropped and added;
- the settings turned on;
- any rule challenge, and anything left for a follow-up.

The release isn't done until every row is merged, or the owner has deferred it.
