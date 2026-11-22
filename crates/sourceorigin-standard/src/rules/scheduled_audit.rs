//! `scheduled-audit`: advisories published after a change still surface.

use super::workflows::Workflow;
use super::yaml::Node;
use super::{Finding, Repository};

/// The commands that audit the dependencies.
const AUDITS: [&str; 2] = ["just audit", "cargo deny"];

impl Workflow {
    /// Whether a `schedule` trigger runs it at least weekly: a cron whose
    /// day of the month and month are `*`.
    fn weekly(&self) -> bool {
        let schedules = self
            .document
            .get("on")
            .and_then(|on| on.get("schedule"))
            .map_or(&[][..], Node::items);
        schedules
            .iter()
            .filter_map(|schedule| schedule.get("cron")?.as_str())
            .any(|cron| {
                matches!(
                    cron.split_whitespace().collect::<Vec<_>>()[..],
                    [_, _, "*", "*", _]
                )
            })
    }

    /// Whether a step runs an audit.
    fn audits(&self) -> bool {
        self.steps()
            .filter_map(|step| step.get("run")?.as_str())
            .any(|run| AUDITS.iter().any(|audit| run.contains(audit)))
    }
}

impl Repository {
    /// A repository where no workflow audits the dependencies at least
    /// weekly.
    pub(super) fn scheduled_audit(&self) -> Vec<Finding> {
        if self
            .workflows
            .iter()
            .any(|workflow| workflow.weekly() && workflow.audits())
        {
            return Vec::new();
        }
        vec![Finding {
            path: ".github/workflows".into(),
            line: 1,
            message: "no workflow audits the dependencies on a schedule: add one running \
                      `just audit` at least weekly, such as `cron: \"0 8 * * 1\"`"
                .to_owned(),
        }]
    }
}
