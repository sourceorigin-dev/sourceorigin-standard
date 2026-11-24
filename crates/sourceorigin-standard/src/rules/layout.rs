//! `layout`: where a workspace's crates live, and how `cargo xtask` runs.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;
use toml::Spanned;

use super::fix::{Checked, Fix};
use super::{Finding, Repository};

/// Cargo's configuration, from the repository's root.
pub(super) const CARGO_CONFIG: &str = ".cargo/config.toml";

/// The directories crates live in: libraries, binaries and development
/// tasks.
const DIRS: [&str; 3] = ["crates", "apps", "tools"];

/// The crate `cargo xtask` runs, and its directory.
const XTASK: &str = "xtask";
const XTASK_DIR: &str = "tools/xtask";

/// What `cargo xtask` stands for.
const ALIAS: &str = "run --locked --package xtask --";

/// `.cargo/config.toml`, reduced to its aliases.
#[derive(Deserialize)]
pub(super) struct CargoConfig {
    #[serde(default)]
    alias: BTreeMap<String, Spanned<toml::Value>>,
}

impl Repository {
    /// Each crate outside the crate directories, an `xtask` crate missing
    /// from `tools/xtask`, and an `xtask` alias that isn't the standard's.
    /// A wrong or missing alias is fixed.
    pub(super) fn layout(&self) -> Checked {
        let root = &self.metadata.workspace_root;
        let mut findings: Vec<Finding> = self
            .manifests
            .iter()
            .filter(|manifest| {
                !DIRS
                    .iter()
                    .any(|dir| manifest.path.starts_with(dir) && manifest.path != Path::new(dir))
            })
            .map(|manifest| Finding {
                path: manifest.path.clone(),
                line: 1,
                message: "the crate is outside `crates/`, `apps/` and `tools/`: move it to \
                          `crates/` for a library, `apps/` for a binary, or `tools/` for a \
                          development task"
                    .to_owned(),
            })
            .collect();
        let xtask = self
            .metadata
            .packages
            .iter()
            .find(|package| package.name == XTASK);
        if xtask.is_none_or(|xtask| xtask.manifest_path.parent() != Some(&root.join(XTASK_DIR))) {
            findings.push(Finding {
                path: XTASK_DIR.into(),
                line: 1,
                message: format!(
                    "no `{XTASK}` crate in `{XTASK_DIR}`: put the repository's development \
                     tasks there, run as `cargo xtask`"
                ),
            });
        }
        let alias = self
            .cargo_config
            .as_ref()
            .and_then(|config| Some((config, config.value.alias.get(XTASK)?)));
        let wanted = format!("{XTASK} = \"{ALIAS}\"");
        match alias {
            Some((_, value)) if Self::alias_of(value.get_ref()) == ALIAS => {
                return findings.into();
            }
            Some((config, value)) => findings.push(Finding {
                path: CARGO_CONFIG.into(),
                line: config.line(value),
                message: format!(
                    "the `{XTASK}` alias is `{}`: make it `{ALIAS}`",
                    Self::alias_of(value.get_ref())
                ),
            }),
            None => findings.push(Finding {
                path: CARGO_CONFIG.into(),
                line: 1,
                message: format!("no `{XTASK}` alias: add `{wanted}` under `[alias]`"),
            }),
        }
        let fixes = vec![Fix::toml(
            CARGO_CONFIG,
            format!("alias `{XTASK}` to `{ALIAS}`"),
            |document| document["alias"][XTASK] = toml_edit::value(ALIAS),
        )];
        Checked { findings, fixes }
    }

    /// An alias's command, from its string or its list of arguments.
    fn alias_of(value: &toml::Value) -> String {
        match value {
            toml::Value::Array(arguments) => arguments
                .iter()
                .filter_map(toml::Value::as_str)
                .collect::<Vec<_>>()
                .join(" "),
            other => other.as_str().unwrap_or_default().to_owned(),
        }
    }
}
