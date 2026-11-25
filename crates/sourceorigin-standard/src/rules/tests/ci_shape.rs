use super::{Fixture, INHERITS};
use crate::rules::Rule;

/// A CI workflow of the standard's shape.
const CI: &str = "\
on:
  pull_request:
  merge_group:
jobs:
  changes:
    steps:
      - id: filter
        uses: dorny/paths-filter@ceb8a2b8f2d89434be7ff52d3de7ec3738c5cc9d # v4.0.3
        with:
          filters: .github/filters.yml
  rust:
    needs: changes
  ci:
    if: always()
    needs: [changes, rust]
";

/// The violation of a `ci` job whose `needs`, at `line`, misses `job`.
fn unneeded(line: usize, job: &str) -> String {
    format!(
        ".github/workflows/ci.yml:{line}: the `ci` job doesn't need `{job}`: add it to `needs`, \
         or list it in standard.toml's `ci-watchers` if it watches `ci` from outside (ci-shape)"
    )
}

#[test]
fn ci_has_a_changes_job_and_one_status_the_queue_waits_on() {
    let fixture = Fixture::new();
    fixture
        .add_crate("core", INHERITS)
        .write(".github/workflows/ci.yml", CI);
    assert_eq!(fixture.violations(Rule::CiShape), Vec::<String>::new());

    fixture.write(
        ".github/workflows/ci.yml",
        "on:\n  pull_request:\njobs:\n  changes:\n    runs-on: ubuntu-latest\n  rust:\n    \
         needs: changes\n  docs:\n    needs: changes\n  ci:\n    needs:\n      - changes\n",
    );
    assert_eq!(
        fixture.violations(Rule::CiShape),
        [
            ".github/workflows/ci.yml:2: CI doesn't run on `merge_group`: add the trigger, so \
             the merge queue runs it (ci-shape)",
            ".github/workflows/ci.yml:11: the `ci` job doesn't run `if: always()`: add it, so a \
             failure upstream fails it (ci-shape)",
            &unneeded(12, "rust"),
            &unneeded(12, "docs"),
        ]
    );
    fixture.fix_rule(Rule::CiShape);
    assert_eq!(fixture.violations(Rule::CiShape).len(), 3);
}

/// A job that needs `ci`, directly or through another job, runs after it,
/// so `ci` needing it would deadlock.
#[test]
fn ci_doesnt_need_the_jobs_that_run_after_it() {
    let fixture = Fixture::new();
    fixture.add_crate("core", INHERITS).write(
        ".github/workflows/ci.yml",
        &format!(
            "{CI}  report:\n    needs: ci\n  publish:\n    needs: [rust, report]\n  docs:\n    \
             runs-on: ubuntu-latest\n"
        ),
    );
    assert_eq!(fixture.violations(Rule::CiShape), [unneeded(15, "docs")]);
}

/// A job `ci-watchers` lists watches `ci` from outside, as by polling the
/// run, so `ci` needn't need it; a listed name that is no job is stale.
#[test]
fn ci_doesnt_need_the_jobs_that_watch_it() {
    let fixture = Fixture::new();
    fixture
        .add_crate("core", INHERITS)
        .write(
            ".github/workflows/ci.yml",
            &format!("{CI}  fail-fast:\n    runs-on: ubuntu-latest\n"),
        )
        .write("standard.toml", "ci-watchers = [\"fail-fast\"]\n");
    assert_eq!(fixture.violations(Rule::CiShape), Vec::<String>::new());

    fixture.write(
        "standard.toml",
        "ci-watchers = [\"fail-fast\", \"cancel\"]\n",
    );
    assert_eq!(
        fixture.violations(Rule::CiShape),
        [
            "standard.toml:1: `ci-watchers` lists `cancel`, which is no job of \
          .github/workflows/ci.yml: remove it (ci-shape)"
        ]
    );
}

#[test]
fn a_ci_workflow_that_doesnt_parse_is_left_to_config_syntax() {
    let fixture = Fixture::new();
    fixture
        .add_crate("core", INHERITS)
        .write(".github/workflows/ci.yml", "on: [\n");
    assert_eq!(fixture.violations(Rule::CiShape), Vec::<String>::new());
    assert_eq!(fixture.violations(Rule::ConfigSyntax).len(), 1);
}
