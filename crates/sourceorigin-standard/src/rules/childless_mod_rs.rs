//! `childless-mod-rs`: a module is `foo/mod.rs` only when it has children.

use std::path::Path;

use super::fix::{Checked, Fix};
use super::source::Source;
use super::{Finding, Repository};
use crate::metadata::TargetKind;

impl Repository {
    /// Each `mod.rs` whose directory holds no other Rust file, and the fix
    /// moving it to `foo.rs` when the move can't change what the module
    /// reads. A `mod.rs` beside an integration test's, benchmark's or
    /// example's root passes, since cargo would take `foo.rs` there for a
    /// target of its own, and so does one under `src/bin`.
    pub(super) fn childless_mod_rs(&self) -> Checked {
        let mut checked = Checked::default();
        for path in self.rust_files() {
            let Some(dir) = path
                .file_name()
                .filter(|name| *name == "mod.rs")
                .and_then(|_| path.parent())
            else {
                continue;
            };
            let has_child = self
                .rust_files()
                .any(|other| other != path && other.starts_with(dir));
            let shared = dir.ancestors().any(|dir| dir.ends_with("src/bin"))
                || dir.parent().is_some_and(|up| {
                    self.metadata.holds_roots(
                        up,
                        &[TargetKind::Test, TargetKind::Bench, TargetKind::Example],
                    )
                });
            if has_child || shared {
                continue;
            }
            let to = dir.with_extension("rs");
            let withheld = self.withheld_move(path, dir, &to);
            checked.findings.push(Finding {
                path: path.to_owned(),
                line: 1,
                message: format!(
                    "`{}` has no child module: move it to `{}`{}",
                    path.display(),
                    to.display(),
                    withheld.map_or_else(String::new, |why| format!(
                        " (by hand: `--fix` doesn't, as {why})"
                    ))
                ),
            });
            if withheld.is_none() {
                checked.fixes.push(Fix::move_to(
                    path,
                    format!("move to `{}`, as it has no child module", to.display()),
                    to,
                ));
            }
        }
        checked
    }

    /// Why moving the childless `path`, in `dir`, to `to` needs judgement,
    /// if it does: it would leave other files behind, change what a
    /// relative path in it names, put a target's root beside its module,
    /// or break the `#[path]` or manifest that names it.
    fn withheld_move(&self, path: &Path, dir: &Path, to: &Path) -> Option<&'static str> {
        let source = self.sources.iter().find(|source| source.path == path);
        if self.metadata.is_crate_root(path) {
            Some("it is a target's root, which its manifest names")
        } else if self.modules.named_by_path(path) {
            Some("a `#[path]` attribute names it")
        } else if self
            .files
            .iter()
            .any(|file| file != path && file.starts_with(dir))
        {
            Some("its directory holds other files")
        } else if source.is_none_or(|source| Source::names_files(&source.text)) {
            Some("it may name a file by a relative path")
        } else if self.metadata.holds_roots(dir, &[TargetKind::Bin]) {
            Some("its directory holds a binary's root")
        } else if self.files.contains(to) {
            Some("the file it would move to exists")
        } else {
            None
        }
    }
}
