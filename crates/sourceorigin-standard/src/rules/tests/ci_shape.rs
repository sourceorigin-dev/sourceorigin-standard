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
            ".github/workflows/ci.yml:12: the `ci` job doesn't need `rust`: add it to `needs` \
             (ci-shape)",
            ".github/workflows/ci.yml:12: the `ci` job doesn't need `docs`: add it to `needs` \
             (ci-shape)",
        ]
    );
    fixture.fix_rule(Rule::CiShape);
    assert_eq!(fixture.violations(Rule::CiShape).len(), 3);
}
