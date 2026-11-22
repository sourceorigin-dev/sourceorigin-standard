//! `pr-template`: every pull request says what it changes, why, and how it
//! was tested.

use super::fix::{Checked, Fix};
use super::{Finding, Repository};

/// The template, from the repository's root.
pub(super) const PATH: &str = ".github/pull_request_template.md";

/// The headings it holds.
const HEADINGS: [&str; 2] = ["## What and why", "## Testing"];

impl Repository {
    /// Each required heading the template lacks, fixed by adding it at the
    /// end, which writes a missing template.
    pub(super) fn pr_template(&self) -> Checked {
        let text = self
            .pr_template
            .as_ref()
            .map_or("", |file| file.text.as_str());
        let mut checked = Checked::default();
        for heading in HEADINGS {
            if text.lines().any(|line| line.trim_end() == heading) {
                continue;
            }
            checked.findings.push(Finding {
                path: PATH.into(),
                line: 1,
                message: format!("the pull request template has no `{heading}`: add it"),
            });
            checked.fixes.push(Fix::append(
                PATH,
                format!("add `{heading}`"),
                format!("\n{heading}"),
            ));
        }
        checked
    }
}
