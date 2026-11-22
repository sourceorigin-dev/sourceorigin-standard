//! `job-timeouts`: a hung job fails rather than running for six hours.

use super::{Finding, Repository};

impl Repository {
    /// Each job without `timeout-minutes`, except one calling a reusable
    /// workflow, which can't set it.
    pub(super) fn job_timeouts(&self) -> Vec<Finding> {
        self.workflows
            .iter()
            .flat_map(|workflow| {
                workflow
                    .jobs()
                    .iter()
                    .filter(|(_, job)| {
                        job.get("timeout-minutes").is_none() && job.get("uses").is_none()
                    })
                    .map(|(name, _)| Finding {
                        path: workflow.path.clone(),
                        line: name.line,
                        message: format!(
                            "the job `{}` has no `timeout-minutes`: give it a little over its \
                             longest run",
                            name.as_str().unwrap_or_default()
                        ),
                    })
            })
            .collect()
    }
}
