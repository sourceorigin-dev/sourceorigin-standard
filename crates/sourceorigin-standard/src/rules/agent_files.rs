//! `agent-files`: where agents find their instructions and skills.

use std::path::{Path, PathBuf};

use std::collections::BTreeSet;

use super::config_syntax::Unreadable;
use super::fix::{Checked, Fix};
use super::skills::{DIR as SKILLS, MARKER, Skill};
use super::text_file::TextFile;
use super::{Error, Finding, Repository};
use crate::workspace::Workspace;

/// The agents' instructions, from the repository's root.
pub(super) const AGENTS_MD: &str = "AGENTS.md";

/// What a `CLAUDE.md` holds that only imports AGENTS.md.
const IMPORT: &str = "@AGENTS.md";

/// The symlink Claude Code reads the skills' directory through.
const CLAUDE_SKILLS: &str = ".claude/skills";

/// The files agents read.
pub(super) struct AgentFiles {
    /// `AGENTS.md`, if there is one.
    pub(super) agents_md: Option<TextFile>,
    /// Every `CLAUDE.md`.
    claude_md: Vec<TextFile>,
    /// Every `SKILL.md` under `.agents/skills`.
    skills: Vec<TextFile>,
    /// What `.claude/skills` links to, if it is a symlink.
    claude_skills: Option<PathBuf>,
    /// The files under `.claude/skills` when it is a directory, each with
    /// whether its place under `.agents/skills` is free.
    claude_skill_files: Vec<(String, bool)>,
    /// The skills under `.claude/skills` whose `SKILL.md` says it is synced
    /// from the crate.
    claude_synced: BTreeSet<String>,
}

impl AgentFiles {
    /// The agents' files under `root`; `unreadable` records those that
    /// aren't UTF-8 text.
    pub(super) fn load(root: &Path, unreadable: &mut Unreadable) -> Result<Self, Error> {
        let workspace = Workspace::new(root);
        Ok(Self {
            agents_md: TextFile::load_all(root, &[AGENTS_MD], unreadable)?.pop(),
            claude_md: TextFile::load_all(root, &[":(glob)**/CLAUDE.md"], unreadable)?,
            skills: TextFile::load_all(root, &[":(glob).agents/skills/*/SKILL.md"], unreadable)?,
            claude_skills: std::fs::read_link(root.join(CLAUDE_SKILLS)).ok(),
            claude_skill_files: workspace
                .files(&[":(glob).claude/skills/**"])?
                .into_iter()
                .filter(|path| path != CLAUDE_SKILLS)
                .map(|path| {
                    let free = !root.join(Self::moved(&path)).exists();
                    (path, free)
                })
                .collect(),
            claude_synced: workspace
                .files(&[":(glob).claude/skills/*/SKILL.md"])?
                .into_iter()
                .filter(|path| {
                    std::fs::read_to_string(root.join(path)).is_ok_and(|text| text.contains(MARKER))
                })
                .filter_map(|path| Self::skill_of(&path).map(str::to_owned))
                .collect(),
        })
    }
}

impl AgentFiles {
    /// The skill `path`, a file under `.claude/skills`, is part of.
    fn skill_of(path: &str) -> Option<&str> {
        path.strip_prefix(CLAUDE_SKILLS)?
            .trim_start_matches('/')
            .split('/')
            .next()
            .filter(|name| !name.is_empty())
    }

    /// Where `path`, a file under `.claude/skills`, belongs under
    /// `.agents/skills`.
    fn moved(path: &str) -> PathBuf {
        Path::new(SKILLS).join(
            Path::new(path)
                .strip_prefix(CLAUDE_SKILLS)
                .unwrap_or(Path::new(path)),
        )
    }
}

impl TextFile {
    /// The `name` of a skill's frontmatter, between its opening `---`
    /// lines.
    fn skill_name(&self) -> Option<&str> {
        let mut lines = self.text.lines();
        if lines.next()?.trim_end() != "---" {
            return None;
        }
        lines
            .take_while(|line| line.trim_end() != "---")
            .find_map(|line| line.strip_prefix("name:"))
            .map(|name| name.trim().trim_matches(['"', '\'']))
    }
}

impl Repository {
    /// A missing `AGENTS.md`, each `CLAUDE.md`, each skill whose
    /// frontmatter names another, and a `.claude/skills` that isn't the
    /// symlink to `.agents/skills`.
    pub(super) fn agent_files(&self) -> Checked {
        let agents = &self.agent_files;
        let mut findings = Vec::new();
        if agents.agents_md.is_none() {
            findings.push(Finding {
                path: AGENTS_MD.into(),
                line: 1,
                message: "no AGENTS.md: write the agents' instructions there".to_owned(),
            });
        }
        let mut fixes = Vec::new();
        for claude_md in &agents.claude_md {
            findings.push(Finding {
                path: claude_md.path.clone(),
                line: 1,
                message: "a CLAUDE.md: move what it says to AGENTS.md, which Claude Code reads \
                          too, and delete it"
                    .to_owned(),
            });
            if claude_md.text.trim() == IMPORT {
                fixes.push(Fix::delete(
                    claude_md.path.clone(),
                    "delete the CLAUDE.md that only imports AGENTS.md".to_owned(),
                ));
            }
        }
        for skill in &agents.skills {
            let directory = skill
                .path
                .parent()
                .and_then(Path::file_name)
                .map(|name| name.to_string_lossy());
            let name = skill.skill_name();
            if name.is_none() || name != directory.as_deref() {
                findings.push(Finding {
                    path: skill.path.clone(),
                    line: 1,
                    message: format!(
                        "the skill's frontmatter names it `{}`: start the file with `---`, \
                         `name: {}`, `---`",
                        name.unwrap_or_default(),
                        directory.unwrap_or_default()
                    ),
                });
            }
        }
        for (path, free) in &agents.claude_skill_files {
            let skill = AgentFiles::skill_of(path).filter(|name| Skill::ships(name));
            if let Some(skill) = skill {
                let synced = agents.claude_synced.contains(skill);
                findings.push(Finding {
                    path: path.into(),
                    line: 1,
                    message: format!(
                        "a copy of sourceorigin-standard's `{skill}` skill under {CLAUDE_SKILLS}, \
                         which `cargo xtask standard skills` writes under {SKILLS}: delete it{}",
                        if synced {
                            ""
                        } else {
                            ", or rename it if it is the repository's own"
                        }
                    ),
                });
                if synced {
                    fixes.push(Fix::delete(
                        path,
                        format!("delete the old copy of the `{skill}` skill"),
                    ));
                }
                continue;
            }
            findings.push(Finding {
                path: path.into(),
                line: 1,
                message: format!("a skill under {CLAUDE_SKILLS}: move it to {SKILLS}"),
            });
            if *free {
                let to = AgentFiles::moved(path);
                fixes.push(Fix::move_to(
                    path,
                    format!("move it to {}", to.display()),
                    to,
                ));
            }
        }
        let linked = agents.claude_skills.as_deref() == Some(Path::new("../.agents/skills"));
        if !agents.skills.is_empty() && !linked {
            findings.push(Finding {
                path: CLAUDE_SKILLS.into(),
                line: 1,
                message: format!(
                    "{CLAUDE_SKILLS} isn't a symlink to `../{SKILLS}`: make it one, with \
                     `ln -s ../{SKILLS} {CLAUDE_SKILLS}`"
                ),
            });
        }
        Checked { findings, fixes }
    }
}
