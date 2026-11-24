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

/// Clippy's groups among them, which sit at priority -1 so single lints
/// can override them.
const GROUPS: [(&str, &str); 2] = [("clippy", "all"), ("clippy", "pedantic")];

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
    /// a missing `[lints]`, a `deny` or `forbid`, and a missing required lint,
    /// which it adds at `warn`; never an `allow`.
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
        let inherit = self
            .manifests
            .iter()
            .filter(|manifest| manifest.value.lints.is_none())
            .map(|manifest| {
                Fix::toml(
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
                Fix::toml(
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
        let unset = self.unset_lints();
        Checked {
            findings: crates.chain(levels).chain(unset.findings).collect(),
            fixes: inherit.chain(warn).chain(unset.fixes).collect(),
        }
    }

    /// Each lint the standard requires that `[workspace.lints]` doesn't
    /// set, fixed by adding it at `warn` under its tool's table.
    fn unset_lints(&self) -> Checked {
        let unset: Vec<(&str, &str)> = REQUIRED
            .into_iter()
            .filter(|(tool, name)| {
                !self
                    .lint_levels
                    .iter()
                    .any(|lint| lint.tool == *tool && lint.name == *name)
            })
            .collect();
        let missing = unset.iter().map(|(tool, name)| Finding {
            path: ROOT.into(),
            line: 1,
            message: format!(
                "the standard requires `{name} = \"warn\"` in `[workspace.lints.{tool}]`"
            ),
        });
        let add = unset.iter().map(|&(tool, name)| {
            Fix::toml(
                ROOT,
                format!("add `{name} = \"warn\"` to `[workspace.lints.{tool}]`"),
                move |document| {
                    let lints = &mut document["workspace"]["lints"];
                    if lints.is_none() {
                        let mut implicit = toml_edit::Table::new();
                        implicit.set_implicit(true);
                        *lints = toml_edit::Item::Table(implicit);
                    }
                    if lints.get(tool).is_none() {
                        lints[tool] = toml_edit::table();
                    }
                    lints[tool][name] = if GROUPS.contains(&(tool, name)) {
                        let mut group = toml_edit::InlineTable::new();
                        group.insert("level", "warn".into());
                        group.insert("priority", (-1).into());
                        toml_edit::value(group)
                    } else {
                        toml_edit::value("warn")
                    };
                },
            )
        });
        Checked {
            findings: missing.collect(),
            fixes: add.collect(),
        }
    }
}
