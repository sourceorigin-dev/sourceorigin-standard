//! `actions-pinned`: a workflow runs the exact code of each action it
//! reviewed.

use super::{Finding, Repository};

impl Repository {
    /// Each `uses:` of a step or a job naming an action or workflow of
    /// another repository by anything but a commit SHA with its version in
    /// a comment.
    pub(super) fn actions_pinned(&self) -> Vec<Finding> {
        let mut findings = Vec::new();
        for workflow in &self.workflows {
            let jobs = workflow.jobs().iter().map(|(_, job)| job);
            let uses = workflow
                .steps()
                .chain(jobs)
                .filter_map(|node| node.get("uses"));
            for used in uses {
                let Some(action) = used.as_str() else {
                    continue;
                };
                if action.starts_with("./") || action.starts_with("docker://") {
                    continue;
                }
                let pinned = action.rsplit_once('@').is_some_and(|(_, sha)| {
                    sha.len() == 40 && sha.bytes().all(|b| b.is_ascii_hexdigit())
                });
                let line = workflow.text.lines().nth(used.line - 1).unwrap_or_default();
                let versioned = line
                    .split_once(" #")
                    .is_some_and(|(_, comment)| comment.trim_start().starts_with('v'));
                if !(pinned && versioned) {
                    findings.push(Finding {
                        path: workflow.path.clone(),
                        line: used.line,
                        message: format!(
                            "`{action}` isn't pinned: name the commit of its release, \
                             `{}@<40-character SHA> # v<version>`",
                            action.split('@').next().unwrap_or(action)
                        ),
                    });
                }
            }
        }
        findings
    }
}
