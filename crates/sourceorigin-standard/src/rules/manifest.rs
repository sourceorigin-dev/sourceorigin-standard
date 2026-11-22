//! The workspace's manifests: the root's `[workspace]`, and each crate's,
//! reduced to what the rules read.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;
use toml::Spanned;

use super::Error;
use super::level::Level;
use super::toml_file::TomlFile;
use crate::metadata::Metadata;

/// The root manifest, from the repository's root.
pub(super) const ROOT: &str = "Cargo.toml";

/// The root manifest.
#[derive(Deserialize)]
pub(super) struct RootManifest {
    #[serde(default)]
    pub(super) workspace: Workspace,
}

/// The root manifest's `[workspace]`.
#[derive(Default, Deserialize)]
pub(super) struct Workspace {
    /// Each tool's lints, such as `clippy`'s, each with its setting.
    #[serde(default)]
    pub(super) lints: BTreeMap<String, BTreeMap<Spanned<String>, Setting>>,
}

/// A lint's setting in a manifest: its level, alone or in a table beside
/// its priority.
#[derive(Deserialize)]
#[serde(untagged)]
pub(super) enum Setting {
    Level(Level),
    Table { level: Level },
}

/// A workspace crate's manifest.
#[derive(Deserialize)]
pub(super) struct Manifest {
    pub(super) lints: Option<Spanned<toml::Table>>,
}

impl TomlFile<RootManifest> {
    /// The root manifest under `root`.
    pub(super) fn load_root(root: &Path) -> Result<Self, Error> {
        Self::require(root, ROOT)
    }
}

impl TomlFile<Manifest> {
    /// The manifest of every crate `metadata` lists.
    pub(super) fn load_all(metadata: &Metadata) -> Result<Vec<Self>, Error> {
        metadata
            .packages
            .iter()
            .map(|package| {
                let path = &package.manifest_path;
                Self::require(
                    &metadata.workspace_root,
                    path.strip_prefix(&metadata.workspace_root).unwrap_or(path),
                )
            })
            .collect()
    }
}
