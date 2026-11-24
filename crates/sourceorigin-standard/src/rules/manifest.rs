//! The workspace's manifests: the root's `[workspace]`, and each crate's,
//! reduced to what the rules read.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;
use toml::Spanned;

use super::Error;
use super::config_syntax::Unreadable;
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
    pub(super) resolver: Option<Spanned<String>>,
    pub(super) package: Option<Spanned<WorkspacePackage>>,
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

/// The root manifest's `[workspace.package]`, which crates inherit from.
#[derive(Deserialize)]
pub(super) struct WorkspacePackage {
    pub(super) edition: Option<Spanned<String>>,
}

/// A workspace crate's manifest.
#[derive(Deserialize)]
pub(super) struct Manifest {
    pub(super) package: Option<Spanned<Package>>,
    pub(super) lints: Option<Spanned<toml::Table>>,
}

/// A crate's `[package]`.
#[derive(Deserialize)]
pub(super) struct Package {
    /// A string, or `{ workspace = true }`.
    pub(super) edition: Option<Spanned<toml::Value>>,
}

impl TomlFile<RootManifest> {
    /// The root manifest under `root`, if it parses.
    pub(super) fn load_root(
        root: &Path,
        unreadable: &mut Unreadable,
    ) -> Result<Option<Self>, Error> {
        Self::load(root, ROOT, unreadable)
    }
}

impl TomlFile<Manifest> {
    /// The manifest of every crate `metadata` lists that parses.
    pub(super) fn load_all(
        metadata: &Metadata,
        unreadable: &mut Unreadable,
    ) -> Result<Vec<Self>, Error> {
        let mut manifests = Vec::new();
        for package in &metadata.packages {
            let path = &package.manifest_path;
            let path = path.strip_prefix(&metadata.workspace_root).unwrap_or(path);
            manifests.extend(Self::load(&metadata.workspace_root, path, unreadable)?);
        }
        Ok(manifests)
    }
}
