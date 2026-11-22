//! Each Rust file's module paths, from walking every target's module tree
//! as rustc does: from its root file through each `mod` item, `#[path]`
//! included.

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

use syn::{Expr, Lit, Meta};

use super::source::Source;
use crate::metadata::Metadata;

/// The module paths of each Rust file a target reaches, such as
/// `["my_crate", "ffi"]` for `src/ffi.rs`. A target's root module is its
/// crate's name: the library's, a binary's or a test's, or
/// `build_script_build` for a build script. A file a module tree shares,
/// through `#[path]`, has several.
#[derive(Debug, Default)]
pub(super) struct Modules(BTreeMap<PathBuf, Vec<Vec<String>>>);

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
            let dir = root.parent().map_or_else(PathBuf::new, Path::to_owned);
            modules.walk(&sources, root, &[target.name.replace('-', "_")], &dir);
        }
        modules
    }

    /// The module paths of `path`, from the repository's root: none for a
    /// file no target reaches.
    pub(super) fn of_file(&self, path: &Path) -> &[Vec<String>] {
        self.0.get(path).map_or(&[], Vec::as_slice)
    }

    /// Records `file` as `module`, whose child modules' files live in
    /// `dir`, and walks its `mod` items.
    fn walk(
        &mut self,
        sources: &BTreeMap<&Path, &Source>,
        file: &Path,
        module: &[String],
        dir: &Path,
    ) {
        let Some(source) = sources.get(file) else {
            return;
        };
        let paths = self.0.entry(file.to_owned()).or_default();
        if paths.iter().any(|path| path == module) {
            return;
        }
        paths.push(module.to_vec());
        self.items(sources, &source.syntax.items, file, module, dir, false);
    }

    /// Walks the `mod` items of `items`, in `file`, inside `module`, whose
    /// child modules' files live in `dir`; `inline` inside a `mod { … }`.
    fn items(
        &mut self,
        sources: &BTreeMap<&Path, &Source>,
        items: &[syn::Item],
        file: &Path,
        module: &[String],
        dir: &Path,
        inline: bool,
    ) {
        for item in items {
            let syn::Item::Mod(item) = item else {
                continue;
            };
            let name = item.ident.to_string();
            let mut child = module.to_vec();
            child.push(name.clone());
            if let Some((_, items)) = &item.content {
                self.items(sources, items, file, &child, &dir.join(&name), true);
                continue;
            }
            let attribute = Self::path_attribute(&item.attrs).map(|path| {
                let base = if inline {
                    dir.to_owned()
                } else {
                    file.parent().map_or_else(PathBuf::new, Path::to_owned)
                };
                Self::normalize(&base.join(path))
            });
            let found = attribute
                .into_iter()
                .chain([
                    dir.join(format!("{name}.rs")),
                    dir.join(&name).join("mod.rs"),
                ])
                .find(|candidate| sources.contains_key(candidate.as_path()));
            if let Some(found) = found {
                let child_dir = if found.file_name().is_some_and(|file| file == "mod.rs") {
                    found.parent().map_or_else(PathBuf::new, Path::to_owned)
                } else {
                    found.with_extension("")
                };
                self.walk(sources, &found, &child, &child_dir);
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
