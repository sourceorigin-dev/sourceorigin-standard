//! `cargo metadata`, reduced to the workspace's crates and what they depend
//! on.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::Error;
use crate::workspace::Workspace;

/// The workspace's crates, as `cargo metadata --no-deps` lists them.
#[derive(Debug, Deserialize)]
pub(super) struct Metadata {
    pub(super) packages: Vec<Package>,
    /// The workspace's root, which changed paths are relative to.
    pub(super) workspace_root: PathBuf,
}

/// A workspace crate.
#[derive(Debug, Deserialize)]
pub(super) struct Package {
    pub(super) name: String,
    pub(super) manifest_path: PathBuf,
    /// Of every kind: normal, development and build.
    pub(super) dependencies: Vec<Dependency>,
    pub(super) targets: Vec<Target>,
}

/// A crate a package depends on, named as its package is.
#[derive(Debug, Deserialize)]
pub(super) struct Dependency {
    pub(super) name: String,
}

/// A package's library, binary, test, example, benchmark or build script.
#[derive(Debug, Deserialize)]
pub(super) struct Target {
    pub(super) kind: Vec<TargetKind>,
    /// Its root source file.
    pub(super) src_path: PathBuf,
}

/// What a target builds, as `cargo metadata` names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum TargetKind {
    Lib,
    Rlib,
    Dylib,
    Cdylib,
    Staticlib,
    ProcMacro,
    Bin,
    Test,
    Bench,
    Example,
    CustomBuild,
    /// A kind this crate doesn't know yet.
    #[serde(other)]
    Other,
}

impl TargetKind {
    /// Whether it is a library of any kind.
    pub(super) fn is_library(self) -> bool {
        matches!(
            self,
            Self::Lib | Self::Rlib | Self::Dylib | Self::Cdylib | Self::Staticlib | Self::ProcMacro
        )
    }
}

impl Metadata {
    /// The workspace's metadata, from cargo.
    pub(super) fn load(workspace: Workspace<'_>) -> Result<Self, Error> {
        let json = workspace.output(
            "cargo",
            &["metadata", "--locked", "--no-deps", "--format-version", "1"],
        )?;
        serde_json::from_str(&json).map_err(Error::Metadata)
    }

    /// Whether the workspace has a crate named `name`.
    pub(super) fn has(&self, name: &str) -> bool {
        self.packages.iter().any(|package| package.name == name)
    }

    /// The crate owning `path`, relative to the workspace's root: the
    /// deepest whose directory holds it, since a crate's directory may
    /// contain another's. A crate at the root owns only the entries its
    /// targets live in, such as `src/` and `build.rs`: the rest of the root,
    /// such as `Cargo.lock`, is the whole workspace's.
    pub(super) fn package_of(&self, path: &str) -> Option<&Package> {
        let file = self.workspace_root.join(path);
        self.packages
            .iter()
            .filter_map(|package| Some((package, package.manifest_path.parent()?)))
            .filter(|&(package, dir)| {
                if dir == self.workspace_root {
                    package.owns_at_root(Path::new(path), dir)
                } else {
                    file.starts_with(dir)
                }
            })
            .max_by_key(|(_, dir)| dir.components().count())
            .map(|(package, _)| package)
    }
}

impl Package {
    /// Whether the crate, whose directory `dir` is the workspace's root,
    /// owns `path`, relative to it: whether one of its targets lives under
    /// the same top-level entry.
    fn owns_at_root(&self, path: &Path, dir: &Path) -> bool {
        let entry = path.components().next();
        self.targets.iter().any(|target| {
            target
                .src_path
                .strip_prefix(dir)
                .is_ok_and(|source| source.components().next() == entry)
        })
    }

    /// Whether the crate has a library, which doc tests need.
    pub(super) fn has_library(&self) -> bool {
        self.targets
            .iter()
            .flat_map(|target| &target.kind)
            .any(|kind| kind.is_library())
    }
}
