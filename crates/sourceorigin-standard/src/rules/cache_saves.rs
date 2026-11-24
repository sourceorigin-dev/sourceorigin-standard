//! `cache-saves`: caches are saved from `main`, which every branch reads, so
//! a pull request never evicts them with entries no other branch can read.

use super::{Finding, Repository};

/// The `save-if` conditions that save from `main` only, or never, as words.
const SAVE_IFS: [&[&str]; 3] = [
    &["false"],
    &["github.ref", "==", "'refs/heads/main'"],
    &["'refs/heads/main'", "==", "github.ref"],
];

/// The events whose caches only their own ref can read.
const SCOPED: [&str; 3] = ["pull_request", "pull_request_target", "merge_group"];

impl Repository {
    /// Each `Swatinem/rust-cache` step whose `save-if` is neither `false`
    /// nor `github.ref == 'refs/heads/main'`, and each `actions/cache` step, which saves on a miss, in
    /// a composite action or a workflow pull requests or the merge queue
    /// run.
    pub(super) fn cache_saves(&self) -> Vec<Finding> {
        let mut findings = Vec::new();
        for workflow in &self.workflows {
            let scoped = workflow.is_action()
                || workflow
                    .triggers()
                    .iter()
                    .any(|trigger| SCOPED.contains(trigger));
            for step in workflow.steps() {
                let Some(uses) = step.get("uses") else {
                    continue;
                };
                let action = uses.as_str().unwrap_or_default();
                let message = if action.starts_with("Swatinem/rust-cache@") {
                    let save_if = step.get("with").and_then(|with| with.get("save-if"));
                    let main_only =
                        save_if
                            .and_then(|save_if| save_if.as_str())
                            .is_some_and(|condition| {
                                let condition = condition
                                    .trim()
                                    .strip_prefix("${{")
                                    .and_then(|inner| inner.strip_suffix("}}"))
                                    .unwrap_or(condition);
                                let words: Vec<&str> = condition.split_whitespace().collect();
                                SAVE_IFS.contains(&words.as_slice())
                            });
                    (!main_only).then_some(
                        "the Rust cache is saved off `main`: set \
                         `save-if: ${{ github.ref == 'refs/heads/main' }}`",
                    )
                } else if action.starts_with("actions/cache@") && scoped {
                    Some(
                        "`actions/cache` saves on a miss, here off `main`: restore with \
                         `actions/cache/restore`, and save with `actions/cache/save` from `main` \
                         only",
                    )
                } else {
                    None
                };
                findings.extend(message.map(|message| Finding {
                    path: workflow.path.clone(),
                    line: uses.line,
                    message: message.to_owned(),
                }));
            }
        }
        findings
    }
}
