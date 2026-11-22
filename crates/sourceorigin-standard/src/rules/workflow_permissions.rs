//! `workflow-permissions`: a workflow's token reads the repository, and a
//! job that needs more asks for it.

use super::fix::{Checked, Fix};
use super::{Finding, Repository};

impl Repository {
    /// Each workflow whose top-level `permissions` isn't exactly
    /// `contents: read`. A missing one is fixed by adding it before `jobs:`.
    pub(super) fn workflow_permissions(&self) -> Checked {
        let mut checked = Checked::default();
        for workflow in self
            .workflows
            .iter()
            .filter(|workflow| !workflow.is_action())
        {
            let permissions = workflow.document.get("permissions");
            let read_only = permissions.is_some_and(|permissions| {
                matches!(
                    permissions.entries(),
                    [(key, value)] if key.as_str() == Some("contents") && value.as_str() == Some("read")
                )
            });
            if read_only {
                continue;
            }
            checked.findings.push(Finding {
                path: workflow.path.clone(),
                line: permissions.map_or(1, |permissions| permissions.line),
                message: "the workflow's `permissions` aren't `contents: read`: set them to it \
                          at the top, and grant more to the jobs that need it"
                    .to_owned(),
            });
            let jobs = workflow
                .text
                .lines()
                .position(|line| line.trim_end() == "jobs:");
            if let (None, Some(index)) = (permissions, jobs) {
                checked.fixes.push(Fix::line(
                    workflow.path.clone(),
                    "set `permissions: contents: read`".to_owned(),
                    index + 1,
                    "permissions:\n  contents: read\n\njobs:".to_owned(),
                ));
            }
        }
        checked
    }
}
