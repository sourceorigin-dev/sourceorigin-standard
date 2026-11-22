//! `workspace-lints`: every crate inherits the workspace's lints, which all
//! warn.

use super::fix::{Checked, Fix};
use super::level::Level;
use super::manifest::{ROOT, RootManifest, Setting};
use super::toml_file::TomlFile;
use super::{Finding, Repository};

/// The lints the standard requires the workspace to set to `warn`, as
/// `[workspace.lints.<tool>] <lint>`.
const REQUIRED: [(&str, &str); 14] = [
    ("rust", "unsafe_code"),
    ("rust", "missing_docs"),
    ("clippy", "all"),
    ("clippy", "pedantic"),
    ("clippy", "unwrap_used"),
    ("clippy", "expect_used"),
    ("clippy", "panic"),
    ("clippy", "todo"),
    ("clippy", "unimplemented"),
    ("clippy", "dbg_macro"),
    ("clippy", "allow_attributes"),
    ("clippy", "allow_attributes_without_reason"),
    ("clippy", "undocumented_unsafe_blocks"),
    ("clippy", "self_named_module_files"),
];

/// A lint the root manifest's `[workspace.lints]` sets.
pub(super) struct LintLevel {
    /// Its tool's table, such as `rust` or `clippy`.
    tool: String,
    /// Its name in that table, such as `panic` or `pedantic`.
    name: String,
    pub(super) level: Level,
    /// Its line in the root manifest, from 1.
    line: usize,
}

impl LintLevel {
    /// The lints the root manifest's `[workspace.lints]` sets.
    pub(super) fn all(root: &TomlFile<RootManifest>) -> Vec<Self> {
        root.value
            .workspace
            .lints
            .iter()
            .flat_map(|(tool, lints)| {
                lints.iter().map(move |(name, setting)| Self {
                    tool: tool.clone(),
                    line: root.line(name),
                    name: name.get_ref().clone(),
                    level: match setting {
                        Setting::Level(level) | Setting::Table { level } => *level,
                    },
                })
            })
            .collect()
    }

    /// Its name as attributes write it, such as `clippy::panic`.
    pub(super) fn lint(&self) -> String {
        if self.tool == "rust" {
            self.name.clone()
        } else {
            format!("{}::{}", self.tool, self.name)
        }
    }

    /// Whether the standard requires it.
    fn required(&self) -> bool {
        REQUIRED.contains(&(self.tool.as_str(), self.name.as_str()))
    }
}

impl Repository {
    /// Each crate whose manifest lacks `[lints] workspace = true`, which
    /// Cargo allows only without lint tables of the crate's own; each lint
    /// the standard requires that the workspace doesn't set to `warn`; and
    /// each other workspace lint set to neither `warn` nor `allow`. It fixes
    /// a missing `[lints]` and a `deny` or `forbid`, never an `allow`.
    pub(super) fn workspace_lints(&self) -> Checked {
        let crates = self
            .manifests
            .iter()
            .filter_map(|manifest| match &manifest.value.lints {
                None => Some(Finding {
                    path: manifest.path.clone(),
                    line: 1,
                    message: "no `[lints]`: add `[lints] workspace = true`".to_owned(),
                }),
                Some(lints)
                    if lints
                        .get_ref()
                        .get("workspace")
                        .and_then(toml::Value::as_bool)
                        != Some(true) =>
                {
                    Some(Finding {
                        path: manifest.path.clone(),
                        line: manifest.line(lints),
                        message: "`[lints]` declares the crate's own lints: move them to \
                              `[workspace.lints]`, and inherit those with `workspace = true`"
                            .to_owned(),
                    })
                }
                Some(_) => None,
            });
        let levels = self.lint_levels.iter().filter_map(|lint| {
            let message = match lint.level {
                Level::Warn => return None,
                Level::Allow if !lint.required() => return None,
                Level::Allow => format!(
                    "`{}` is `allow`, but the standard requires it: make it `warn`",
                    lint.lint()
                ),
                level @ (Level::Expect | Level::Deny | Level::Forbid) => format!(
                    "`{}` is `{level}`: make it `warn`, which `-D warnings` makes an error",
                    lint.lint()
                ),
            };
            Some(Finding {
                path: ROOT.into(),
                line: lint.line,
                message,
            })
        });
        let missing = REQUIRED
            .iter()
            .filter(|(tool, name)| {
                !self
                    .lint_levels
                    .iter()
                    .any(|lint| lint.tool == *tool && lint.name == *name)
            })
            .map(|(tool, name)| Finding {
                path: ROOT.into(),
                line: 1,
                message: format!(
                    "the standard requires `{name} = \"warn\"` in `[workspace.lints.{tool}]`"
                ),
            });
        let inherit = self
            .manifests
            .iter()
            .filter(|manifest| manifest.value.lints.is_none())
            .map(|manifest| {
                Fix::new(
                    manifest.path.clone(),
                    "add `[lints] workspace = true`".to_owned(),
                    |document| document["lints"]["workspace"] = toml_edit::value(true),
                )
            });
        let warn = self
            .lint_levels
            .iter()
            .filter(|lint| matches!(lint.level, Level::Deny | Level::Forbid))
            .map(|lint| {
                let (tool, name) = (lint.tool.clone(), lint.name.clone());
                Fix::new(
                    ROOT,
                    format!("make `{}` `warn`", lint.lint()),
                    move |document| {
                        let item = &mut document["workspace"]["lints"][&tool][&name];
                        if item.is_str() {
                            Fix::set(item, "warn");
                        } else {
                            Fix::set(&mut item["level"], "warn");
                        }
                    },
                )
            });
        Checked {
            findings: crates.chain(levels).chain(missing).collect(),
            fixes: inherit.chain(warn).collect(),
        }
    }
}
