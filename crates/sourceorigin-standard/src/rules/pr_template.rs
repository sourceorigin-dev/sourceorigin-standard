//! `pr-template`: every pull request says what it changes, why, and how it
//! was tested.

use super::fix::{Checked, Fix};
use super::{Finding, Repository};

/// The template, from the repository's root.
pub(super) const PATH: &str = ".github/pull_request_template.md";

/// The headings it holds.
const HEADINGS: [&str; 2] = ["## What and why", "## Testing"];

impl Repository {
    /// Each required heading the template lacks, fixed by adding them at
    /// the end, which writes a missing template; nothing for one that isn't
    /// UTF-8 text, which is `config-syntax`'s.
    pub(super) fn pr_template(&self) -> Checked {
        if self.unreadable.contains(PATH) {
            return Checked::default();
        }
        let text = self
            .pr_template
            .as_ref()
            .map_or("", |file| file.text.as_str());
        let missing: Vec<&str> = HEADINGS
            .into_iter()
            .filter(|&heading| !text.lines().any(|line| line.trim_end() == heading))
            .collect();
        let mut checked = Checked::from(
            missing
                .iter()
                .map(|heading| Finding {
                    path: PATH.into(),
                    line: 1,
                    message: format!("the pull request template has no `{heading}`: add it"),
                })
                .collect::<Vec<_>>(),
        );
        if !missing.is_empty() {
            checked.fixes.push(Fix::append(
                PATH,
                format!("add `{}`", missing.join("`, `")),
                missing
                    .iter()
                    .map(|heading| format!("\n{heading}"))
                    .collect::<Vec<_>>()
                    .join("\n"),
            ));
        }
        checked
    }
}
