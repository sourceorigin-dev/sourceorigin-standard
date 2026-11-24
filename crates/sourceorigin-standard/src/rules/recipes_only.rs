//! `recipes-only`: T3 Code and CI run the justfile's recipes rather than
//! repeating their commands.

use super::t3;
use super::{Finding, Repository};
use crate::config;

impl Repository {
    /// Each `t3.json` command that isn't a recipe, each line of a workflow's
    /// or action's `run:` starting with `cargo` that `cargo-in-ci` doesn't
    /// list, and each listed line no `run:` holds.
    pub(super) fn recipes_only(&self) -> Vec<Finding> {
        let mut findings = Vec::new();
        if let Some(t3) = &self.t3 {
            findings.extend(
                t3.scripts
                    .iter()
                    .filter(|script| script.command.split_whitespace().next() != Some("just"))
                    .map(|script| Finding {
                        path: t3::PATH.into(),
                        line: t3.line(script),
                        message: format!(
                            "`{}` isn't a recipe: add it to the justfile, and run it with `just`",
                            script.command
                        ),
                    }),
            );
        }
        let allowed = &self.config.cargo_in_ci;
        let mut used = vec![false; allowed.len()];
        for workflow in &self.workflows {
            let runs = workflow.steps().filter_map(|step| step.get("run"));
            for (line, command) in runs.flat_map(|run| run.lines()) {
                let command = command.trim();
                if command.split_whitespace().next() != Some("cargo") {
                    continue;
                }
                match allowed.iter().position(|listed| listed.value == command) {
                    Some(index) => used[index] = true,
                    None => findings.push(Finding {
                        path: workflow.path.clone(),
                        line,
                        message: format!(
                            "`{command}` runs Cargo rather than a recipe: run a recipe of the \
                             justfile, or list the line in {}'s `cargo-in-ci` if no recipe fits",
                            config::PATH
                        ),
                    }),
                }
            }
        }
        findings.extend(
            allowed
                .iter()
                .zip(used)
                .filter(|(_, used)| !used)
                .map(|(listed, _)| Finding {
                    path: config::PATH.into(),
                    line: listed.line,
                    message: format!(
                        "`cargo-in-ci` lists `{}`, which no workflow runs: remove it",
                        listed.value
                    ),
                }),
        );
        findings
    }
}
