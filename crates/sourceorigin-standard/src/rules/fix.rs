//! `--fix`: the edits that resolve a rule's mechanical violations, each made
//! to a TOML document so the file keeps its layout and comments.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Path, PathBuf};

use toml_edit::{DocumentMut, Item, Value};

use super::{Error, Finding, Rule};
use crate::config::{self, Deviation, Stale};

/// A mechanical edit to one TOML file, which `cargo xtask standard --fix`
/// makes.
pub struct Fix {
    /// The file, from the repository's root.
    pub path: PathBuf,
    /// What it changes, such as "add `[lints] workspace = true`".
    pub what: String,
    edit: Box<dyn Fn(&mut DocumentMut)>,
}

impl Fix {
    /// A fix of `path` that `edit` makes to its document.
    pub(super) fn new(
        path: impl Into<PathBuf>,
        what: String,
        edit: impl Fn(&mut DocumentMut) + 'static,
    ) -> Self {
        Self {
            path: path.into(),
            what,
            edit: Box::new(edit),
        }
    }

    /// Replaces the value `item` holds with `new`, keeping the comments and
    /// spacing around it.
    pub(super) fn set(item: &mut Item, new: impl Into<Value>) {
        if let Some(value) = item.as_value_mut() {
            let decor = value.decor().clone();
            *value = new.into();
            *value.decor_mut() = decor;
        }
    }

    /// Removes `key` from the table `item` is, inline or not, and lays an
    /// inline table out again so its braces keep their spaces.
    pub(super) fn remove(item: &mut Item, key: &str) {
        if let Some(table) = item.as_table_like_mut() {
            table.remove(key);
        }
        if let Some(inline) = item.as_inline_table_mut() {
            inline.fmt();
        }
    }

    /// Makes `fixes` under `root`: each file's once, in order, and drops a
    /// fix that repeats another.
    pub(super) fn apply_all(root: &Path, fixes: &mut Vec<Self>) -> Result<(), Error> {
        let mut seen = BTreeSet::new();
        fixes.retain(|fix| seen.insert((fix.path.clone(), fix.what.clone())));
        let mut by_path: BTreeMap<&Path, Vec<&Self>> = BTreeMap::new();
        for fix in fixes.iter() {
            by_path.entry(&fix.path).or_default().push(fix);
        }
        for (path, fixes) in by_path {
            let file = root.join(path);
            let text = std::fs::read_to_string(&file).map_err(|source| Error::Read {
                path: file.clone(),
                source,
            })?;
            let mut document: DocumentMut =
                text.parse()
                    .map_err(|error: toml_edit::TomlError| Error::Toml {
                        path: path.to_owned(),
                        message: error.to_string(),
                    })?;
            for fix in fixes {
                (fix.edit)(&mut document);
            }
            let mut written = document.to_string();
            if text.contains("\r\n") {
                written = written.replace("\r\n", "\n").replace('\n', "\r\n");
            }
            std::fs::write(&file, written).map_err(|source| Error::Write { path: file, source })?;
        }
        Ok(())
    }

    /// The removal of the `stale` entries of `rule`'s `deviation`: the whole
    /// deviation, when none of it excuses anything.
    pub(super) fn stale(rule: Rule, deviation: &Deviation, stale: &[Stale]) -> Self {
        let name = rule.name();
        let entries = deviation
            .scope
            .as_ref()
            .map_or(0, |scope| scope.paths.len() + scope.crates.len());
        if stale.iter().any(|stale| matches!(stale, Stale::Whole(_))) || stale.len() == entries {
            return Self::new(
                config::PATH,
                format!("remove the stale deviation from `{name}`"),
                move |document| Self::remove(&mut document["deviations"], name),
            );
        }
        let gone: Vec<String> = stale
            .iter()
            .filter_map(|stale| match stale {
                Stale::Path(listed) | Stale::Crate(listed) => Some(listed.value.clone()),
                Stale::Whole(_) => None,
            })
            .collect();
        Self::new(
            config::PATH,
            format!(
                "remove `{}` from the deviation from `{name}`",
                gone.join("`, `")
            ),
            move |document| {
                for key in ["paths", "crates"] {
                    if let Some(values) = document
                        .get_mut("deviations")
                        .and_then(|deviations| deviations.get_mut(name))
                        .and_then(|deviation| deviation.get_mut(key))
                        .and_then(Item::as_array_mut)
                    {
                        values.retain(|value| {
                            value
                                .as_str()
                                .is_none_or(|value| !gone.iter().any(|gone| gone == value))
                        });
                    }
                }
            },
        )
    }
}

impl fmt::Debug for Fix {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Fix")
            .field("path", &self.path)
            .field("what", &self.what)
            .finish_non_exhaustive()
    }
}

impl fmt::Display for Fix {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.path.display(), self.what)
    }
}

/// What a rule's check finds: its violations, and the fixes `--fix` can
/// make, which may resolve some of them or tighten what already passes.
#[derive(Debug, Default)]
pub(super) struct Checked {
    pub(super) findings: Vec<Finding>,
    pub(super) fixes: Vec<Fix>,
}

impl From<Vec<Finding>> for Checked {
    fn from(findings: Vec<Finding>) -> Self {
        Self {
            findings,
            fixes: Vec::new(),
        }
    }
}
