//! `anyhow-edges`: libraries return typed errors; `anyhow` stays at a
//! binary's edge.

use super::{Finding, Repository};
use crate::config;

/// The crate that erases error types.
const ANYHOW: &str = "anyhow";

impl Repository {
    /// Each library depending on `anyhow` that `anyhow-crates` doesn't
    /// list, and each listed name that is no workspace crate.
    pub(super) fn anyhow_edges(&self) -> Vec<Finding> {
        let listed = &self.config.anyhow_crates;
        let mut findings: Vec<Finding> = self
            .metadata
            .packages
            .iter()
            .filter(|package| {
                package.has_library()
                    && package.depends_on(ANYHOW)
                    && !listed.iter().any(|listed| listed.value == package.name)
            })
            .map(|package| Finding {
                path: package
                    .manifest_path
                    .strip_prefix(&self.metadata.workspace_root)
                    .unwrap_or(&package.manifest_path)
                    .to_owned(),
                line: 1,
                message: format!(
                    "the library `{}` depends on `{ANYHOW}`: return a `thiserror` enum instead, \
                     or list the crate in {}'s `anyhow-crates` if it is a binary's edge",
                    package.name,
                    config::PATH
                ),
            })
            .collect();
        findings.extend(
            listed
                .iter()
                .filter(|listed| !self.metadata.has(&listed.value))
                .map(|listed| Finding {
                    path: config::PATH.into(),
                    line: listed.line,
                    message: format!(
                        "`anyhow-crates` lists `{}`, no workspace crate: remove it",
                        listed.value
                    ),
                }),
        );
        findings
    }
}
