use super::{Fixture, INHERITS};
use crate::rules::Rule;

/// A workflow running `just audit` on `cron`.
fn audit(cron: &str) -> String {
    format!(
        "on:\n  schedule:\n    - cron: \"{cron}\"\njobs:\n  audit:\n    steps:\n      \
         - run: just audit\n"
    )
}

#[test]
fn a_workflow_audits_the_dependencies_weekly() {
    let fixture = Fixture::new();
    fixture
        .add_crate("core", INHERITS)
        .write(".github/workflows/audit.yml", &audit("0 0 1 * *"))
        .write(
            ".github/workflows/ci.yml",
            "on: push\njobs:\n  checks:\n    steps:\n      - run: just audit\n",
        );
    assert_eq!(
        fixture.violations(Rule::ScheduledAudit),
        [
            ".github/workflows:1: no workflow audits the dependencies on a schedule: add one \
          running `just audit` at least weekly, such as `cron: \"0 8 * * 1\"` (scheduled-audit)"
        ]
    );
    fixture.write(".github/workflows/audit.yml", &audit("0 8 * * 1"));
    assert_eq!(
        fixture.violations(Rule::ScheduledAudit),
        Vec::<String>::new()
    );
}
