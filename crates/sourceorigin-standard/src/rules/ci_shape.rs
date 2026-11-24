//! `ci-shape`: CI sums its jobs up in one `ci` status, which the merge
//! queue waits on. The `changes` job and its filters are `check-changed`'s
//! drift check's.

use std::path::Path;

use super::fix::{Checked, Fix};
use super::workflows::Workflow;
use super::yaml::Node;
use super::{Finding, Repository};

/// The CI workflow, from the repository's root.
const PATH: &str = ".github/workflows/ci.yml";

/// The job summing the others up.
const CI: &str = "ci";

impl Workflow {
    /// The job `key`.
    fn job(&self, key: &str) -> Option<&Node> {
        self.jobs()
            .iter()
            .find(|(name, _)| name.as_str() == Some(key))
            .map(|(_, job)| job)
    }

    /// Where the CI workflow breaks the standard's shape: each line with
    /// its message.
    fn shape(&self) -> Vec<(usize, String)> {
        let mut found = Vec::new();
        if !self.triggers().contains(&"merge_group") {
            found.push((
                self.document.get("on").map_or(1, |on| on.line),
                "CI doesn't run on `merge_group`: add the trigger, so the merge queue runs it"
                    .to_owned(),
            ));
        }
        let Some(ci) = self.job(CI) else {
            found.push((
                1,
                format!(
                    "no `{CI}` job: add one that needs every other job, runs `if: always()`, \
                     and fails if any of them failed; with `standard / pr-checks`, it is one \
                     of the repository's two required checks"
                ),
            ));
            return found;
        };
        if ci.get("if").and_then(Node::as_str) != Some("always()") {
            found.push((
                ci.line,
                format!(
                    "the `{CI}` job doesn't run `if: always()`: add it, so a failure upstream \
                     fails it"
                ),
            ));
        }
        let needs = ci.get("needs");
        let needed: Vec<&str> = needs
            .into_iter()
            .flat_map(|needs| {
                needs
                    .as_str()
                    .into_iter()
                    .chain(needs.items().iter().filter_map(Node::as_str))
            })
            .collect();
        let missing = self
            .jobs()
            .iter()
            .filter_map(|(name, _)| name.as_str())
            .filter(|&name| name != CI && !needed.contains(&name));
        for name in missing {
            found.push((
                needs.map_or(ci.line, |needs| needs.line),
                format!("the `{CI}` job doesn't need `{name}`: add it to `needs`"),
            ));
        }
        found
    }

    /// The fix of a missing `merge_group` trigger, when `on:` is a mapping
    /// on its own line.
    fn merge_group_fix(&self) -> Option<Fix> {
        let mapping = self
            .document
            .get("on")
            .is_some_and(|on| !on.entries().is_empty());
        if self.triggers().contains(&"merge_group") || !mapping {
            return None;
        }
        let index = self
            .text
            .lines()
            .position(|line| line.trim_end() == "on:")?;
        Some(Fix::lines(
            PATH,
            "run CI on `merge_group`".to_owned(),
            index + 1..index + 2,
            "on:\n  merge_group:".to_owned(),
        ))
    }
}

impl Repository {
    /// Where the CI workflow breaks the standard's shape, or a missing one:
    /// nothing for one that doesn't parse, which is `config-syntax`'s.
    pub(super) fn ci_shape(&self) -> Checked {
        if self.unreadable.contains(PATH) {
            return Checked::default();
        }
        let Some(ci) = self
            .workflows
            .iter()
            .find(|workflow| workflow.path == Path::new(PATH))
        else {
            return vec![Finding {
                path: PATH.into(),
                line: 1,
                message: format!("no {PATH}: add the CI workflow, with a `{CI}` job"),
            }]
            .into();
        };
        Checked {
            findings: ci
                .shape()
                .into_iter()
                .map(|(line, message)| Finding {
                    path: PATH.into(),
                    line,
                    message,
                })
                .collect(),
            fixes: ci.merge_group_fix().into_iter().collect(),
        }
    }
}
