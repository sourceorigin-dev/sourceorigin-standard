//! `--fix`: the edits that resolve a rule's mechanical violations. Each
//! file's edits are made in one pass: it is read once (a missing file reads
//! as empty), its TOML document edited or its lines replaced, written, and
//! then deleted or moved. Two edits that conflict on one file are an error.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::ops::Range;
use std::path::{Path, PathBuf};

use toml_edit::{DocumentMut, Item, Value};

use super::{Error, Finding, Rule};
use crate::config::{self, Deviation, Stale};

/// A mechanical edit to one file, which `cargo xtask standard --fix` makes.
pub struct Fix {
    /// The file, from the repository's root.
    pub path: PathBuf,
    /// What it changes, such as "add `[lints] workspace = true`".
    pub what: String,
    /// The file whose violation it resolves, which a deviation scoped to
    /// that file keeps it from changing: `path`, unless it edits another.
    pub(super) resolves: PathBuf,
    edit: Edit,
}

/// An edit to a TOML document.
pub(super) type TomlEdit = Box<dyn Fn(&mut DocumentMut)>;

/// How a fix changes its file.
enum Edit {
    /// Edits it as a TOML document, keeping its layout and comments.
    Toml(TomlEdit),
    /// Replaces the lines in this range, from 1 and its end excluded, with
    /// this text, which may span lines or be empty. An empty range inserts
    /// before its line; one past the last line appends.
    Lines(Range<usize>, String),
    /// Replaces it with this text, creating it.
    Write(String),
    /// Deletes it, and the directories it leaves empty.
    Delete,
    /// Moves it here, from the repository's root, after its other edits.
    Move(PathBuf),
}

impl Fix {
    fn of(path: impl Into<PathBuf>, what: String, edit: Edit) -> Self {
        let path = path.into();
        Self {
            resolves: path.clone(),
            path,
            what,
            edit,
        }
    }

    /// A fix that `edit` makes to `path`'s TOML document.
    pub(super) fn toml(
        path: impl Into<PathBuf>,
        what: String,
        edit: impl Fn(&mut DocumentMut) + 'static,
    ) -> Self {
        Self::of(path, what, Edit::Toml(Box::new(edit)))
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

    /// Makes `fixes` under `root`, dropping a fix that repeats another: each
    /// file's in one pass, once every file's edits are known not to
    /// conflict.
    pub(super) fn apply_all(root: &Path, fixes: &mut Vec<Self>) -> Result<(), Error> {
        let mut seen = BTreeSet::new();
        fixes.retain(|fix| seen.insert((fix.path.clone(), fix.what.clone())));
        let mut by_path: BTreeMap<&Path, Vec<&Self>> = BTreeMap::new();
        for fix in fixes.iter() {
            by_path.entry(&fix.path).or_default().push(fix);
        }
        for (path, edits) in &by_path {
            Self::check(path, edits)?;
            for fix in edits {
                if let Edit::Move(to) = &fix.edit
                    && let Some(other) = by_path.get(to.as_path()).and_then(|other| other.first())
                {
                    return Err(Self::conflict(to, fix, other));
                }
            }
        }
        for (path, edits) in by_path {
            Self::apply(root, path, &edits)?;
        }
        Ok(())
    }

    /// Checks that no two of `edits`, all of `path`, conflict.
    fn check(path: &Path, edits: &[&Self]) -> Result<(), Error> {
        for (index, first) in edits.iter().enumerate() {
            for second in &edits[index + 1..] {
                let conflicts = match (&first.edit, &second.edit) {
                    (Edit::Toml(_), Edit::Toml(_)) => false,
                    (Edit::Lines(a, _), Edit::Lines(b, _)) => {
                        (a.start < b.end && b.start < a.end) || a.start == b.start
                    }
                    (Edit::Move(_), other) | (other, Edit::Move(_)) => {
                        !matches!(other, Edit::Toml(_) | Edit::Lines(..) | Edit::Write(_))
                    }
                    _ => true,
                };
                if conflicts {
                    return Err(Self::conflict(path, first, second));
                }
            }
        }
        Ok(())
    }

    fn conflict(path: &Path, first: &Self, second: &Self) -> Error {
        Error::Conflict {
            path: path.to_owned(),
            first: first.what.clone(),
            second: second.what.clone(),
        }
    }

    /// Makes `edits`, which don't conflict, to `path`: its content, then its
    /// deletion or move.
    fn apply(root: &Path, path: &Path, edits: &[&Self]) -> Result<(), Error> {
        let file = root.join(path);
        let writes_content = edits
            .iter()
            .any(|fix| matches!(fix.edit, Edit::Toml(_) | Edit::Lines(..) | Edit::Write(_)));
        if writes_content {
            let text = match std::fs::read_to_string(&file) {
                Ok(text) => text,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
                Err(source) => {
                    return Err(Error::Read {
                        path: file.clone(),
                        source,
                    });
                }
            };
            let mut written = Self::content(path, &text, edits)?;
            if text.contains("\r\n") {
                written = written.replace("\r\n", "\n").replace('\n', "\r\n");
            }
            Self::create_parent(&file)?;
            std::fs::write(&file, written).map_err(|source| Error::Write {
                path: file.clone(),
                source,
            })?;
        }
        for fix in edits {
            match &fix.edit {
                Edit::Delete => {
                    std::fs::remove_file(&file).map_err(|source| Error::Write {
                        path: file.clone(),
                        source,
                    })?;
                    Self::remove_empty_dirs(root, &file)?;
                }
                Edit::Move(to) => {
                    let to = root.join(to);
                    if to.exists() {
                        return Err(Error::Write {
                            path: to,
                            source: std::io::ErrorKind::AlreadyExists.into(),
                        });
                    }
                    Self::create_parent(&to)?;
                    std::fs::rename(&file, &to)
                        .map_err(|source| Error::Write { path: to, source })?;
                    Self::remove_empty_dirs(root, &file)?;
                }
                Edit::Toml(_) | Edit::Lines(..) | Edit::Write(_) => {}
            }
        }
        Ok(())
    }

    /// The text `path` holds once `edits` have changed `text`.
    fn content(path: &Path, text: &str, edits: &[&Self]) -> Result<String, Error> {
        if let Some(written) = edits.iter().find_map(|fix| match &fix.edit {
            Edit::Write(written) => Some(written.clone()),
            Edit::Toml(_) | Edit::Lines(..) | Edit::Delete | Edit::Move(_) => None,
        }) {
            return Ok(written);
        }
        let toml: Vec<&TomlEdit> = edits
            .iter()
            .filter_map(|fix| match &fix.edit {
                Edit::Toml(edit) => Some(edit),
                Edit::Lines(..) | Edit::Write(_) | Edit::Delete | Edit::Move(_) => None,
            })
            .collect();
        if !toml.is_empty() {
            let mut document: DocumentMut =
                text.parse()
                    .map_err(|error: toml_edit::TomlError| Error::Toml {
                        path: path.to_owned(),
                        message: error.to_string(),
                    })?;
            for edit in toml {
                edit(&mut document);
            }
            return Ok(document.to_string());
        }
        let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
        let mut replacements: Vec<(&Range<usize>, &String)> = edits
            .iter()
            .filter_map(|fix| match &fix.edit {
                Edit::Lines(range, new) => Some((range, new)),
                Edit::Toml(_) | Edit::Write(_) | Edit::Delete | Edit::Move(_) => None,
            })
            .collect();
        replacements.sort_by_key(|(range, _)| std::cmp::Reverse(range.start));
        for (range, new) in replacements {
            let end = lines.len() + 1;
            let range = range.start.clamp(1, end) - 1..range.end.clamp(1, end) - 1;
            lines.splice(range, new.lines().map(str::to_owned));
        }
        Ok(lines.into_iter().map(|line| line + "\n").collect())
    }

    /// Creates the directory `file` goes in.
    fn create_parent(file: &Path) -> Result<(), Error> {
        let Some(dir) = file.parent() else {
            return Ok(());
        };
        std::fs::create_dir_all(dir).map_err(|source| Error::Write {
            path: dir.to_owned(),
            source,
        })
    }

    /// Removes each directory between `file`, gone, and `root` that is left
    /// empty.
    fn remove_empty_dirs(root: &Path, file: &Path) -> Result<(), Error> {
        for dir in file.ancestors().skip(1).take_while(|dir| *dir != root) {
            match std::fs::remove_dir(dir) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::DirectoryNotEmpty => break,
                Err(source) => {
                    return Err(Error::Write {
                        path: dir.to_owned(),
                        source,
                    });
                }
            }
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
            return Self::toml(
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
        Self::toml(
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

impl Fix {
    /// A fix replacing `lines` of `path`, from 1 and the end excluded, with
    /// `text`.
    pub(super) fn lines(
        path: impl Into<PathBuf>,
        what: String,
        lines: Range<usize>,
        text: String,
    ) -> Self {
        Self::of(path, what, Edit::Lines(lines, text))
    }

    /// A fix adding `text` after the last line of `path`.
    pub(super) fn append(path: impl Into<PathBuf>, what: String, text: String) -> Self {
        Self::lines(path, what, usize::MAX..usize::MAX, text)
    }

    /// A fix replacing `path` with `text`, creating it.
    pub(super) fn write(path: impl Into<PathBuf>, what: String, text: String) -> Self {
        Self::of(path, what, Edit::Write(text))
    }

    /// A fix deleting `path`.
    pub(super) fn delete(path: impl Into<PathBuf>, what: String) -> Self {
        Self::of(path, what, Edit::Delete)
    }

    /// A fix moving `path` to `to`, from the repository's root.
    pub(super) fn move_to(path: impl Into<PathBuf>, what: String, to: PathBuf) -> Self {
        Self::of(path, what, Edit::Move(to))
    }

    /// The fix, resolving a violation in `resolves` rather than in its own
    /// file.
    #[must_use]
    pub(super) fn resolving(self, resolves: PathBuf) -> Self {
        Self { resolves, ..self }
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
