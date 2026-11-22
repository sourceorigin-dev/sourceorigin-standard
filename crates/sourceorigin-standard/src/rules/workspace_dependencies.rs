//! `workspace-dependencies`: each dependency is declared once, for the
//! whole workspace.

use super::manifest::ROOT;
use super::{Finding, Repository};

impl Repository {
    /// Each dependency a crate declares without `workspace = true`, and
    /// each path dependency of `[workspace.dependencies]` without a
    /// `version`.
    pub(super) fn workspace_dependencies(&self) -> Vec<Finding> {
        let mut findings: Vec<Finding> = self
            .manifests
            .iter()
            .flat_map(|manifest| {
                manifest
                    .value
                    .all_dependencies()
                    .filter(|(_, value)| {
                        value.get("workspace").and_then(toml::Value::as_bool) != Some(true)
                    })
                    .map(move |(name, _)| Finding {
                        path: manifest.path.clone(),
                        line: manifest.line(name),
                        message: format!(
                            "`{name}` is declared here: declare it once in the root's \
                             `[workspace.dependencies]`, and inherit it with \
                             `{name}.workspace = true`",
                            name = name.get_ref()
                        ),
                    })
            })
            .collect();
        if let Some(root) = &self.root_manifest {
            findings.extend(
                root.value
                    .workspace
                    .dependencies
                    .iter()
                    .filter(|(_, value)| {
                        value.get("path").is_some() && value.get("version").is_none()
                    })
                    .map(|(name, _)| Finding {
                        path: ROOT.into(),
                        line: root.line(name),
                        message: format!(
                            "`{}` has a `path` and no `version`: add the crate's version, so a \
                             dependent can be published",
                            name.get_ref()
                        ),
                    }),
            );
        }
        findings
    }
}
