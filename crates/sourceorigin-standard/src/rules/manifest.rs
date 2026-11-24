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
    /// The dependencies crates inherit, each with its requirement.
    #[serde(default)]
    pub(super) dependencies: DependencyTable,
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

/// A table of dependencies, each a version or a table.
pub(super) type DependencyTable = BTreeMap<Spanned<String>, toml::Value>;

/// A workspace crate's manifest.
#[derive(Deserialize)]
pub(super) struct Manifest {
    pub(super) package: Option<Spanned<Package>>,
    pub(super) lints: Option<Spanned<toml::Table>>,
    #[serde(default)]
    dependencies: DependencyTable,
    #[serde(default, rename = "dev-dependencies")]
    dev_dependencies: DependencyTable,
    #[serde(default, rename = "build-dependencies")]
    build_dependencies: DependencyTable,
    /// Each target's own dependencies, such as `cfg(unix)`'s.
    #[serde(default)]
    target: BTreeMap<String, DependencyTables>,
}

/// A target's tables of dependencies, of every kind.
#[derive(Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(super) struct DependencyTables {
    #[serde(default)]
    dependencies: DependencyTable,
    #[serde(default)]
    dev_dependencies: DependencyTable,
    #[serde(default)]
    build_dependencies: DependencyTable,
}

impl Manifest {
    /// Every dependency it declares, of every kind and target.
    pub(super) fn all_dependencies(
        &self,
    ) -> impl Iterator<Item = (&Spanned<String>, &toml::Value)> {
        let own = [
            &self.dependencies,
            &self.dev_dependencies,
            &self.build_dependencies,
        ];
        let targets = self.target.values().flat_map(|target| {
            [
                &target.dependencies,
                &target.dev_dependencies,
                &target.build_dependencies,
            ]
        });
        own.into_iter().chain(targets).flatten()
    }
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
