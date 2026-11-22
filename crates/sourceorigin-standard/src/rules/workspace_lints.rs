//! `workspace-lints`: every crate inherits the workspace's lints, which all
//! warn.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use toml::Spanned;

use super::level::Level;
use super::{Error, Finding, Repository};
use crate::config::line_of;
use crate::metadata::Metadata;

/// A workspace crate's manifest, reduced to its `[lints]` table.
pub(super) struct Manifest {
    /// From the repository's root.
    path: PathBuf,
    /// Its `[lints]` table's line, and whether it says `workspace = true`.
    lints: Option<(usize, bool)>,
}

/// A manifest's `[lints]` table, with where it is.
#[derive(Deserialize)]
struct Lints {
    lints: Option<Spanned<toml::Table>>,
}

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

/// A lint's setting in a manifest: its level, alone or in a table beside
/// its priority.
#[derive(Deserialize)]
#[serde(untagged)]
enum Setting {
    Level(Level),
    Table { level: Level },
}

/// The root manifest's `[workspace.lints]`, each lint with where it is.
#[derive(Deserialize)]
struct Root {
    workspace: Option<RootWorkspace>,
}

#[derive(Deserialize)]
struct RootWorkspace {
    #[serde(default)]
    lints: BTreeMap<String, BTreeMap<Spanned<String>, Setting>>,
}

impl LintLevel {
    /// The root manifest.
    pub(super) const PATH: &str = "Cargo.toml";

    /// The lints `[workspace.lints]` sets, from the root manifest under
    /// `root`.
    pub(super) fn load_all(root: &Path) -> Result<Vec<Self>, Error> {
        let path = root.join(Self::PATH);
        let text = std::fs::read_to_string(&path).map_err(|source| Error::Read { path, source })?;
        let manifest: Root = toml::from_str(&text).map_err(|error| Error::Toml {
            path: Self::PATH.into(),
            message: error.to_string(),
        })?;
        let tools = manifest
            .workspace
            .map_or_else(BTreeMap::new, |workspace| workspace.lints);
        Ok(tools
            .into_iter()
            .flat_map(|(tool, lints)| {
                let text = &text;
                lints.into_iter().map(move |(name, setting)| Self {
                    tool: tool.clone(),
                    line: line_of(text, name.span().start),
                    name: name.into_inner(),
                    level: match setting {
                        Setting::Level(level) | Setting::Table { level } => level,
                    },
                })
            })
            .collect())
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

impl Manifest {
    /// The manifest of every crate `metadata` lists.
    pub(super) fn load_all(metadata: &Metadata) -> Result<Vec<Self>, Error> {
        metadata
            .packages
            .iter()
            .map(|package| Self::load(&package.manifest_path, &metadata.workspace_root))
            .collect()
    }

    fn load(path: &Path, root: &Path) -> Result<Self, Error> {
        let text = std::fs::read_to_string(path).map_err(|source| Error::Read {
            path: path.to_owned(),
            source,
        })?;
        let path = path.strip_prefix(root).unwrap_or(path).to_owned();
        let manifest: Lints = toml::from_str(&text).map_err(|error| Error::Toml {
            path: path.clone(),
            message: error.to_string(),
        })?;
        let lints = manifest.lints.map(|lints| {
            let inherits = lints
                .get_ref()
                .get("workspace")
                .and_then(toml::Value::as_bool);
            (line_of(&text, lints.span().start), inherits == Some(true))
        });
        Ok(Self { path, lints })
    }
}

impl Repository {
    /// Each crate whose manifest lacks `[lints] workspace = true`, which
    /// Cargo allows only without lint tables of the crate's own; each lint
    /// the standard requires that the workspace doesn't set to `warn`; and
    /// each other workspace lint set to neither `warn` nor `allow`.
    pub(super) fn workspace_lints(&self) -> Vec<Finding> {
        let crates = self
            .manifests
            .iter()
            .filter_map(|manifest| match manifest.lints {
                None => Some(Finding {
                    path: manifest.path.clone(),
                    line: 1,
                    message: "no `[lints]`: add `[lints] workspace = true`".to_owned(),
                }),
                Some((line, false)) => Some(Finding {
                    path: manifest.path.clone(),
                    line,
                    message: "`[lints]` declares the crate's own lints: move them to \
                          `[workspace.lints]`, and inherit those with `workspace = true`"
                        .to_owned(),
                }),
                Some((_, true)) => None,
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
                path: LintLevel::PATH.into(),
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
                path: LintLevel::PATH.into(),
                line: 1,
                message: format!(
                    "the standard requires `{name} = \"warn\"` in `[workspace.lints.{tool}]`"
                ),
            });
        crates.chain(levels).chain(missing).collect()
    }
}
