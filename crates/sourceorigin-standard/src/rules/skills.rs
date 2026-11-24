//! `skills-synced`: the agent skills the crate ships, and a repository's
//! copies of them in `.agents/skills`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use super::fix::{Checked, Fix};
use super::{Error, Finding, Repository};
use crate::workspace::Workspace;

/// Where a repository's skills live, from its root.
pub(super) const DIR: &str = ".agents/skills";

/// The line a synced `SKILL.md` carries after its frontmatter.
pub(super) const MARKER: &str = "<!-- Synced from the sourceorigin-standard crate by \
                                 `cargo xtask standard skills`: change it there, not here. -->";

/// A skill the crate ships, from its `skills/<name>` directory.
pub(super) struct Skill {
    pub(super) name: &'static str,
    /// Each file, by its path in the skill's directory, with its text.
    pub(super) files: &'static [(&'static str, &'static str)],
}

/// Declares [`SKILLS`], embedding each listed file of `skills/`.
macro_rules! skills {
    ($($name:literal => [$($file:literal),+ $(,)?],)*) => {
        /// Every skill the crate ships.
        pub(super) const SKILLS: &[Skill] = &[$(Skill {
            name: $name,
            files: &[$(($file, include_str!(concat!("../../skills/", $name, "/", $file)))),+],
        }),*];
    };
}

skills! {
    "standard" => ["SKILL.md"],
    "cleanup" => [
        "SKILL.md",
        "references/agent-brief.md",
        "references/checks.md",
        "references/review-brief.md",
        "scripts/stats.py",
    ],
    "docs" => ["SKILL.md"],
}

impl Skill {
    /// Whether the crate ships a skill named `name`.
    pub(super) fn ships(name: &str) -> bool {
        SKILLS.iter().any(|skill| skill.name == name)
    }

    /// The copy of its file `path`, whose text is `text`, that a repository
    /// holds: a `SKILL.md` carries [`MARKER`] after its frontmatter.
    fn copy(path: &str, text: &str) -> String {
        if path != "SKILL.md" {
            return text.to_owned();
        }
        match text
            .strip_prefix("---\n")
            .and_then(|rest| rest.split_once("\n---\n"))
        {
            Some((frontmatter, body)) => format!("---\n{frontmatter}\n---\n\n{MARKER}\n{body}"),
            None => format!("{MARKER}\n\n{text}"),
        }
    }

    /// Each of its files' copy, by its path from the repository's root.
    fn copies(&self) -> impl Iterator<Item = (PathBuf, String)> {
        self.files.iter().map(|(path, text)| {
            (
                Path::new(DIR).join(self.name).join(path),
                Self::copy(path, text),
            )
        })
    }
}

/// The skills a repository holds under `.agents/skills`, each directory
/// holding a file git tracks or would track, by name.
#[derive(Debug, Default)]
pub(super) struct Synced(BTreeMap<String, SkillDir>);

/// A directory of `.agents/skills`.
#[derive(Debug, Default)]
struct SkillDir {
    /// Whether its `SKILL.md` carries [`MARKER`]: only then does it come
    /// from the crate, for the rule to check and `--fix` to change.
    marked: bool,
    /// Each of its files git tracks or would track, by its path from the
    /// repository's root, with its contents.
    files: BTreeMap<PathBuf, Vec<u8>>,
    /// Those of its files git tracks, which `--fix` may delete as it can
    /// get them back.
    tracked: BTreeSet<PathBuf>,
}

impl Synced {
    /// Loads the skills under `root`'s `.agents/skills`, as git lists them,
    /// so the files it ignores, such as editors' swap files, don't count.
    pub(super) fn load(root: &Path) -> Result<Self, Error> {
        let mut synced = Self::default();
        let tracked = Workspace::new(root).tracked(&[DIR])?;
        for listed in Workspace::new(root).files(&[DIR])? {
            let path = PathBuf::from(&listed);
            let Some(name) = path
                .strip_prefix(DIR)
                .ok()
                .filter(|inside| inside.components().count() > 1)
                .and_then(|inside| inside.iter().next())
            else {
                continue;
            };
            let name = name.to_string_lossy().into_owned();
            let contents = match std::fs::read(root.join(&path)) {
                Ok(contents) => contents,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(source) => return Err(Error::Read { path, source }),
            };
            let dir = synced.0.entry(name).or_default();
            if tracked.contains(&listed) {
                dir.tracked.insert(path.clone());
            }
            dir.files.insert(path, contents);
        }
        for (name, dir) in &mut synced.0 {
            dir.marked = dir
                .files
                .get(&Path::new(DIR).join(name).join("SKILL.md"))
                .is_some_and(|text| String::from_utf8_lossy(text).contains(MARKER));
        }
        Ok(synced)
    }

    /// The line, from 1, where `found` first differs from `copy`.
    fn first_difference(found: &[u8], copy: &str) -> usize {
        let found = String::from_utf8_lossy(found);
        let (mut found, mut copy) = (found.lines(), copy.lines());
        let mut line = 1;
        loop {
            match (found.next(), copy.next()) {
                (None, None) => return line,
                (found, copied) if found != copied => return line,
                _ => line += 1,
            }
        }
    }

    /// Each copy that is missing or differs from the crate's, and each file
    /// of a marked directory the crate doesn't ship, with the fix that
    /// writes or deletes it; one git doesn't track is reported without a
    /// fix, as deleting it would lose it. A directory of the repository's
    /// own that shares a skill's name is reported, and left alone.
    pub(super) fn checked(&self) -> Checked {
        let sync = "run `cargo xtask standard skills`, and change the skill in \
                    sourceorigin-standard instead";
        let mut checked = Checked::default();
        let mut copies = BTreeSet::new();
        for skill in SKILLS {
            let dir = self.0.get(skill.name);
            if dir.is_some_and(|dir| !dir.marked) {
                checked.findings.push(Finding {
                    path: Path::new(DIR).join(skill.name).join("SKILL.md"),
                    line: 1,
                    message: format!(
                        "`{DIR}/{0}` is this repository's own skill, as its `SKILL.md` doesn't \
                         say it is synced, but sourceorigin-standard ships a `{0}` skill: rename \
                         the directory, or delete it for `cargo xtask standard skills` to write \
                         the crate's",
                        skill.name
                    ),
                });
                continue;
            }
            for (path, copy) in skill.copies() {
                let found = dir.and_then(|dir| dir.files.get(&path));
                let (line, state) = match found {
                    Some(found) if *found == copy.as_bytes() => {
                        copies.insert(path);
                        continue;
                    }
                    Some(found) => (
                        Self::first_difference(found, &copy),
                        "differs from sourceorigin-standard's copy",
                    ),
                    None => (1, "is missing"),
                };
                checked.findings.push(Finding {
                    path: path.clone(),
                    line,
                    message: format!("the `{}` skill's file {state}: {sync}", skill.name),
                });
                checked.fixes.push(Fix::write(
                    path.clone(),
                    format!("write the `{}` skill's file", skill.name),
                    copy,
                ));
                copies.insert(path);
            }
        }
        for (name, dir) in self.0.iter().filter(|(_, dir)| dir.marked) {
            let what = if Skill::ships(name) {
                format!("the `{name}` skill doesn't ship this file")
            } else {
                format!("sourceorigin-standard no longer ships the `{name}` skill")
            };
            for path in dir.files.keys().filter(|path| !copies.contains(*path)) {
                if !dir.tracked.contains(path) {
                    checked.findings.push(Finding {
                        path: path.clone(),
                        line: 1,
                        message: format!(
                            "{what}, and git doesn't track it, so `--fix` leaves it: move it out \
                             of the synced skill, or delete it"
                        ),
                    });
                    continue;
                }
                checked.findings.push(Finding {
                    path: path.clone(),
                    line: 1,
                    message: format!("{what}: {sync}"),
                });
                checked
                    .fixes
                    .push(Fix::delete(path.clone(), format!("delete it: {what}")));
            }
        }
        checked
    }
}

impl Repository {
    /// Each synced skill's file that differs from the crate's copy, is
    /// missing, or isn't the crate's, with its fix, and each directory of
    /// the repository's own named for a skill the crate ships. A file that
    /// isn't UTF-8 text is `config-syntax`'s to report; its fix stays.
    pub(super) fn skills_synced(&self) -> Checked {
        let mut checked = self.synced.checked();
        checked
            .findings
            .retain(|finding| !self.unreadable.contains(&finding.path.to_string_lossy()));
        checked
    }
}
