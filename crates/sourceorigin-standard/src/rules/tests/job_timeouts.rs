use super::{Fixture, INHERITS};
use crate::rules::Rule;

#[test]
fn every_job_times_out() {
    let fixture = Fixture::new();
    fixture.add_crate("core", INHERITS).write(
        ".github/workflows/ci.yml",
        "jobs:\n  rust:\n    timeout-minutes: 20\n  docs:\n    runs-on: ubuntu-latest\n  \
         release:\n    uses: ./.github/workflows/release.yml\n",
    );
    assert_eq!(
        fixture.violations(Rule::JobTimeouts),
        [
            ".github/workflows/ci.yml:4: the job `docs` has no `timeout-minutes`: give it a \
          little over its longest run (job-timeouts)"
        ]
    );
}
