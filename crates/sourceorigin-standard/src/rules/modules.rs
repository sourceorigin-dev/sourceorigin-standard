//! Each Rust file's module paths, from walking every target's module tree
//! as rustc does: from its root file through each `mod` item, `#[path]`
//! included, and which files are test code.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

use syn::{Expr, Lit, Meta};

use super::source::Source;
use super::test_code::TestAttributes;
use crate::metadata::{Metadata, TargetKind};

/// The module paths of each Rust file a target reaches, such as
/// `["my_crate", "ffi"]` for `src/ffi.rs`. A target's root module is its
/// crate's name: the library's, a binary's or a test's, or
/// `build_script_build` for a build script. A file a module tree shares,
/// through `#[path]`, has several.
#[derive(Debug, Default)]
pub(super) struct Modules {
    paths: BTreeMap<PathBuf, Vec<Vec<String>>>,
    /// What each file is to the targets reaching it.
    codes: BTreeMap<PathBuf, BTreeSet<Code>>,
    /// The files a `mod` item's `#[path]` names, which moving would break.
    named: BTreeSet<PathBuf>,
}

/// What a file is to a target reaching it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Code {
    /// Part of a library, binary or build script.
    Production,
    /// Part of an integration test or benchmark, or behind a module whose
    /// `cfg` holds only in tests.
    Test,
    /// Part of an example.
    Example,
}

/// Where a walk over `mod` items is.
#[derive(Clone, Copy)]
struct Place<'a> {
    /// The file the items are in.
    file: &'a Path,
    /// Where the files of their child modules live.
    dir: &'a Path,
    /// Whether they are inside a `mod { … }`.
    inline: bool,
    /// What they are to the target.
    code: Code,
}

impl Modules {
    /// The module trees of every target of `metadata`'s crates, over
    /// `sources`.
    pub(super) fn of(metadata: &Metadata, sources: &[Source]) -> Self {
        let sources: BTreeMap<&Path, &Source> = sources
            .iter()
            .map(|source| (source.path.as_path(), source))
            .collect();
        let mut modules = Self::default();
        for target in metadata
            .packages
            .iter()
            .flat_map(|package| &package.targets)
        {
            let Ok(root) = target.src_path.strip_prefix(&metadata.workspace_root) else {
                continue;
            };
            let code = if target
                .kind
                .iter()
                .all(|kind| matches!(kind, TargetKind::Test | TargetKind::Bench))
            {
                Code::Test
            } else if target.kind.contains(&TargetKind::Example) {
                Code::Example
            } else {
                Code::Production
            };
            let module = [target.name.replace('-', "_")];
            modules.walk(&sources, root, &module, &Self::child_dir(root, true), code);
        }
        modules
    }

    /// The module paths of `path`, from the repository's root: none for a
    /// file no target reaches.
    pub(super) fn of_file(&self, path: &Path) -> &[Vec<String>] {
        self.paths.get(path).map_or(&[], Vec::as_slice)
    }

    /// Whether a `mod` item's `#[path]` names `path`, from the
    /// repository's root.
    pub(super) fn named_by_path(&self, path: &Path) -> bool {
        self.named.contains(path)
    }

    /// Whether every target reaching `path` reaches it as test code: it is
    /// part of an integration test or benchmark, or a module whose `cfg`
    /// holds only in tests, such as `#[cfg(test)]`, declares it or a module
    /// above it.
    pub(super) fn is_test(&self, path: &Path) -> bool {
        self.only(path, Code::Test)
    }

    /// Whether only examples reach `path`.
    pub(super) fn is_example(&self, path: &Path) -> bool {
        self.only(path, Code::Example)
    }

    /// Whether some target reaches `path`, and every one as `code`.
    fn only(&self, path: &Path, code: Code) -> bool {
        self.codes
            .get(path)
            .is_some_and(|codes| codes.iter().all(|&reached| reached == code))
    }

    /// The directory the child modules of `file` live in: a target's
    /// `root`'s or a `mod.rs`'s own, and `foo/` for `foo.rs`.
    pub(super) fn child_dir(file: &Path, root: bool) -> PathBuf {
        if root || file.file_name().is_some_and(|name| name == "mod.rs") {
            file.parent().map_or_else(PathBuf::new, Path::to_owned)
        } else {
            file.with_extension("")
        }
    }

    /// Records `file` as `module`, whose child modules' files live in
    /// `dir`, which is `code` to its target, and walks its `mod` items.
    fn walk(
        &mut self,
        sources: &BTreeMap<&Path, &Source>,
        file: &Path,
        module: &[String],
        dir: &Path,
        code: Code,
    ) {
        let Some(source) = sources.get(file) else {
            return;
        };
        self.codes.entry(file.to_owned()).or_default().insert(code);
        let paths = self.paths.entry(file.to_owned()).or_default();
        if paths.iter().any(|path| path == module) {
            return;
        }
        paths.push(module.to_vec());
        let place = Place {
            file,
            dir,
            inline: false,
            code,
        };
        self.items(sources, &source.syntax.items, module, &place);
    }

    /// Walks the `mod` items of `items`, inside `module`, at `place`.
    fn items(
        &mut self,
        sources: &BTreeMap<&Path, &Source>,
        items: &[syn::Item],
        module: &[String],
        place: &Place<'_>,
    ) {
        for item in items {
            let syn::Item::Mod(item) = item else {
                continue;
            };
            let name = item.ident.to_string();
            let mut child = module.to_vec();
            child.push(name.clone());
            let code = if item.attrs.cfg_test() {
                Code::Test
            } else {
                place.code
            };
            let dir = place.dir;
            if let Some((_, items)) = &item.content {
                let inner = Place {
                    dir: &dir.join(&name),
                    inline: true,
                    code,
                    ..*place
                };
                self.items(sources, items, &child, &inner);
                continue;
            }
            let attribute = Self::path_attribute(&item.attrs).map(|path| {
                let base = if place.inline {
                    dir.to_owned()
                } else {
                    place
                        .file
                        .parent()
                        .map_or_else(PathBuf::new, Path::to_owned)
                };
                Self::normalize(&base.join(path))
            });
            let found = attribute
                .clone()
                .into_iter()
                .chain([
                    dir.join(format!("{name}.rs")),
                    dir.join(&name).join("mod.rs"),
                ])
                .find(|candidate| sources.contains_key(candidate.as_path()));
            if let Some(found) = found {
                if attribute.as_ref() == Some(&found) {
                    self.named.insert(found.clone());
                }
                self.walk(
                    sources,
                    &found,
                    &child,
                    &Self::child_dir(&found, false),
                    code,
                );
            }
        }
    }

    /// The file a `#[path = "…"]` attribute names, if one does.
    fn path_attribute(attributes: &[syn::Attribute]) -> Option<String> {
        attributes
            .iter()
            .find_map(|attribute| match &attribute.meta {
                Meta::NameValue(pair) if pair.path.is_ident("path") => match &pair.value {
                    Expr::Lit(literal) => match &literal.lit {
                        Lit::Str(path) => Some(path.value()),
                        _ => None,
                    },
                    _ => None,
                },
                _ => None,
            })
    }

    /// `path` with its `.` and `..` resolved.
    fn normalize(path: &Path) -> PathBuf {
        let mut normal = PathBuf::new();
        for component in path.components() {
            match component {
                Component::CurDir => {}
                Component::ParentDir => {
                    normal.pop();
                }
                other => normal.push(other),
            }
        }
        normal
    }
}
