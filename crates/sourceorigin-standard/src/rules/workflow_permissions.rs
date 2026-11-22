//! `workflow-permissions`: a workflow's token reads the repository, and a
//! job that needs more asks for it.

use super::fix::{Checked, Fix};
use super::yaml::Value;
use super::{Finding, Repository};

impl Repository {
    /// Each workflow whose top-level `permissions` is neither exactly
    /// `contents: read` nor `{}`, which grants nothing. A missing one is fixed by adding it before `jobs:`.
    pub(super) fn workflow_permissions(&self) -> Checked {
        let mut checked = Checked::default();
        for workflow in self
            .workflows
            .iter()
            .filter(|workflow| !workflow.is_action())
        {
            let permissions = workflow.document.get("permissions");
            let read_only = permissions.is_some_and(|permissions| match &permissions.value {
                Value::Mapping(entries) => match entries.as_slice() {
                    [] => true,
                    [(key, value)] => {
                        key.as_str() == Some("contents") && value.as_str() == Some("read")
                    }
                    _ => false,
                },
                Value::Scalar(..) | Value::Sequence(_) => false,
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
                checked.fixes.push(Fix::lines(
                    workflow.path.clone(),
                    "set `permissions: contents: read`".to_owned(),
                    index + 1..index + 1,
                    "permissions:\n  contents: read\n".to_owned(),
                ));
            }
        }
        checked
    }
}
