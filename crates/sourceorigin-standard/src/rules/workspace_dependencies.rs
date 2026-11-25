//! `workspace-dependencies`: each dependency is declared once, for the
//! whole workspace.

use std::path::Path;

use super::manifest::ROOT;
use super::{Finding, Repository};

impl Repository {
    /// Each dependency a crate declares without `workspace = true`, but on
    /// itself, and each path dependency of `[workspace.dependencies]`
    /// without a `version` on a crate that is published.
    pub(super) fn workspace_dependencies(&self) -> Vec<Finding> {
        let path: fn(&toml::Value) -> Option<&str> =
            |value| value.get("path").and_then(toml::Value::as_str);
        let mut findings: Vec<Finding> = self
            .manifests
            .iter()
            .flat_map(|manifest| {
                let dir = manifest.path.parent().unwrap_or(Path::new(""));
                manifest
                    .value
                    .all_dependencies()
                    .filter(move |(_, value)| {
                        let inherited =
                            value.get("workspace").and_then(toml::Value::as_bool) == Some(true);
                        let itself = path(value).is_some_and(|path| dir.join(path) == dir);
                        !inherited && !itself
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
                        path(value).is_some_and(|path| !self.metadata.unpublished(Path::new(path)))
                            && value.get("version").is_none()
                    })
                    .map(|(name, _)| Finding {
                        path: ROOT.into(),
                        line: root.line(name),
                        message: format!(
                            "`{}` has a `path` and no `version`: add the crate's version, so a \
                             dependent can be published, or set `publish = false` on a crate \
                             that isn't",
                            name.get_ref()
                        ),
                    }),
            );
        }
        findings
    }
}
