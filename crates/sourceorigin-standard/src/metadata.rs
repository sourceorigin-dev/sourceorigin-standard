//! `cargo metadata`, reduced to the workspace's crates and what they depend
//! on.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::CommandError;
use crate::workspace::Workspace;

/// Why `cargo metadata` couldn't describe the workspace.
#[derive(Debug, thiserror::Error)]
pub enum MetadataError {
    /// It failed to run.
    #[error(transparent)]
    Command(#[from] CommandError),
    /// Its output couldn't be parsed.
    #[error("parse `cargo metadata`")]
    Parse(#[source] serde_json::Error),
}

/// The workspace's crates, as `cargo metadata --no-deps` lists them.
#[derive(Debug, Deserialize)]
pub(crate) struct Metadata {
    pub(crate) packages: Vec<Package>,
    /// The workspace's root, which changed paths are relative to.
    pub(crate) workspace_root: PathBuf,
}

/// A workspace crate.
#[derive(Debug, Deserialize)]
pub(crate) struct Package {
    pub(crate) name: String,
    pub(crate) manifest_path: PathBuf,
    /// Of every kind: normal, development and build.
    pub(crate) dependencies: Vec<Dependency>,
    pub(crate) targets: Vec<Target>,
}

/// A crate a package depends on, named as its package is.
#[derive(Debug, Deserialize)]
pub(crate) struct Dependency {
    pub(crate) name: String,
    /// Its kind; `None` for a normal dependency.
    pub(crate) kind: Option<DependencyKind>,
}

/// A dependency other than a normal one, as `cargo metadata` names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum DependencyKind {
    Dev,
    Build,
}

/// A package's library, binary, test, example, benchmark or build script.
#[derive(Debug, Deserialize)]
pub(crate) struct Target {
    /// Its crate's name, such as `sourceorigin-standard` for a library.
    pub(crate) name: String,
    pub(crate) kind: Vec<TargetKind>,
    /// Its root source file.
    pub(crate) src_path: PathBuf,
}

/// What a target builds, as `cargo metadata` names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum TargetKind {
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
    pub(crate) fn is_library(self) -> bool {
        matches!(
            self,
            Self::Lib | Self::Rlib | Self::Dylib | Self::Cdylib | Self::Staticlib | Self::ProcMacro
        )
    }
}

impl Metadata {
    /// The workspace's metadata, from cargo.
    pub(crate) fn load(workspace: Workspace<'_>) -> Result<Self, MetadataError> {
        let json = workspace.output(
            "cargo",
            &["metadata", "--locked", "--no-deps", "--format-version", "1"],
        )?;
        serde_json::from_str(&json).map_err(MetadataError::Parse)
    }

    /// Whether the workspace has a crate named `name`.
    pub(crate) fn has(&self, name: &str) -> bool {
        self.packages.iter().any(|package| package.name == name)
    }

    /// The crate owning `path`, relative to the workspace's root: the
    /// deepest whose directory holds it, since a crate's directory may
    /// contain another's. A crate at the root owns only the entries its
    /// targets live in, such as `src/` and `build.rs`: the rest of the root,
    /// such as `Cargo.lock`, is the whole workspace's.
    pub(crate) fn package_of(&self, path: &str) -> Option<&Package> {
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

    /// Whether `path`, from the workspace's root, is a crate's root file,
    /// such as `src/lib.rs`, a binary's `main.rs` or an integration test.
    pub(crate) fn is_crate_root(&self, path: &Path) -> bool {
        let file = self.workspace_root.join(path);
        self.packages
            .iter()
            .flat_map(|package| &package.targets)
            .any(|target| target.src_path == file)
    }
}

impl Target {
    /// Whether it is a library of any kind.
    fn is_library(&self) -> bool {
        self.kind.iter().any(|kind| kind.is_library())
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

    /// Whether the crate has a normal dependency on `name`.
    pub(crate) fn depends_on(&self, name: &str) -> bool {
        self.dependencies
            .iter()
            .any(|dependency| dependency.name == name && dependency.kind.is_none())
    }

    /// Whether the crate has a library, which doc tests need.
    pub(crate) fn has_library(&self) -> bool {
        self.targets.iter().any(Target::is_library)
    }
}
