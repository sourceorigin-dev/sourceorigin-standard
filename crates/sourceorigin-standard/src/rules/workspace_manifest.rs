//! `workspace-manifest`: one resolver and one edition, set by the root
//! manifest.

use super::fix::{Checked, Fix};
use super::manifest::ROOT;
use super::{Finding, Repository};

/// The resolver the root manifest sets.
const RESOLVER: &str = "3";

/// The edition `[workspace.package]` sets.
const EDITION: &str = "2024";

impl Repository {
    /// The root manifest's resolver and edition when they aren't the
    /// standard's, and each crate that doesn't inherit the edition. A crate
    /// setting the workspace's edition itself is fixed to inherit it.
    pub(super) fn workspace_manifest(&self) -> Checked {
        let root = &self.root_manifest;
        let workspace = &root.value.workspace;
        let finding = |line, message: String| Finding {
            path: ROOT.into(),
            line,
            message,
        };
        let edition = workspace
            .package
            .as_ref()
            .and_then(|package| package.get_ref().edition.as_ref());
        let mut findings = Vec::new();
        match &workspace.resolver {
            Some(resolver) if resolver.get_ref() == RESOLVER => {}
            Some(resolver) => findings.push(finding(
                root.line(resolver),
                format!(
                    "`resolver` is \"{}\": make it \"{RESOLVER}\"",
                    resolver.get_ref()
                ),
            )),
            None => findings.push(finding(
                1,
                format!("`[workspace]` sets no `resolver`: add `resolver = \"{RESOLVER}\"`"),
            )),
        }
        match edition {
            Some(edition) if edition.get_ref() == EDITION => {}
            Some(edition) => findings.push(finding(
                root.line(edition),
                format!(
                    "`[workspace.package]` sets `edition = \"{}\"`: make it \"{EDITION}\", \
                     migrating with `cargo fix --edition`",
                    edition.get_ref()
                ),
            )),
            None => findings.push(finding(
                1,
                format!("no `[workspace.package]` `edition`: add `edition = \"{EDITION}\"` to it"),
            )),
        }
        let inheritable = edition.is_some_and(|edition| edition.get_ref() == EDITION);
        let mut fixes = Vec::new();
        for manifest in &self.manifests {
            let Some(package) = &manifest.value.package else {
                continue;
            };
            let edition = package.get_ref().edition.as_ref();
            let inherits = edition
                .and_then(|edition| edition.get_ref().get("workspace"))
                .and_then(toml::Value::as_bool)
                == Some(true);
            if inherits {
                continue;
            }
            let line =
                edition.map_or_else(|| manifest.line(package), |edition| manifest.line(edition));
            findings.push(Finding {
                path: manifest.path.clone(),
                line,
                message: "the crate sets its own edition: inherit the workspace's with \
                          `edition.workspace = true`"
                    .to_owned(),
            });
            let repeats = edition
                .and_then(|edition| edition.get_ref().as_str())
                .is_some_and(|edition| edition == EDITION);
            if repeats && inheritable {
                fixes.push(Fix::new(
                    manifest.path.clone(),
                    "inherit the workspace's edition".to_owned(),
                    |document| {
                        let mut inherited = toml_edit::Table::new();
                        inherited.set_dotted(true);
                        inherited["workspace"] = toml_edit::value(true);
                        document["package"]["edition"] = toml_edit::Item::Table(inherited);
                    },
                ));
            }
        }
        Checked { findings, fixes }
    }
}
