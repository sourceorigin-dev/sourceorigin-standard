use super::{Fixture, INHERITS};
use crate::rules::Rule;

#[test]
fn workflows_read_the_repository_and_jobs_ask_for_more() {
    let fixture = Fixture::new();
    fixture
        .add_crate("core", INHERITS)
        .write(
            ".github/workflows/ci.yml",
            "on: push\npermissions:\n  contents: read\njobs:\n  release:\n    permissions:\n      \
             contents: write\n",
        )
        .write(
            ".github/workflows/release.yml",
            "on: push\npermissions: write-all\njobs:\n  release: {}\n",
        )
        .write(
            ".github/workflows/audit.yml",
            "on: push\n\njobs:\n  audit: {}\n",
        );
    let wrong = |path: &str, line: usize| {
        format!(
            "{path}:{line}: the workflow's `permissions` aren't `contents: read`: set them to it \
             at the top, and grant more to the jobs that need it (workflow-permissions)"
        )
    };
    assert_eq!(
        fixture.violations(Rule::WorkflowPermissions),
        [
            wrong(".github/workflows/audit.yml", 1),
            wrong(".github/workflows/release.yml", 2),
        ]
    );
    fixture.fix_rule(Rule::WorkflowPermissions);
    assert_eq!(
        fixture.violations(Rule::WorkflowPermissions),
        [wrong(".github/workflows/release.yml", 2)]
    );
}
