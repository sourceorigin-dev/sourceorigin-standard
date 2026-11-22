//! `agent-files`: where agents find their instructions and skills.

use std::path::{Path, PathBuf};

use super::text_file::TextFile;
use super::{Error, Finding, Repository};
use crate::workspace::Workspace;

/// The agents' instructions, from the repository's root.
pub(super) const AGENTS_MD: &str = "AGENTS.md";

/// The skills' directory, and the symlink Claude Code reads it through.
const SKILLS: &str = ".agents/skills";
const CLAUDE_SKILLS: &str = ".claude/skills";

/// The files agents read.
pub(super) struct AgentFiles {
    /// `AGENTS.md`, if there is one.
    pub(super) agents_md: Option<TextFile>,
    /// Every `CLAUDE.md`, from the root.
    claude_md: Vec<String>,
    /// Every `SKILL.md` under `.agents/skills`.
    skills: Vec<TextFile>,
    /// What `.claude/skills` links to, if it is a symlink.
    claude_skills: Option<PathBuf>,
    /// The files under `.claude/skills` when it is a directory.
    claude_skill_files: Vec<String>,
}

impl AgentFiles {
    /// The agents' files under `root`.
    pub(super) fn load(root: &Path) -> Result<Self, Error> {
        let workspace = Workspace::new(root);
        Ok(Self {
            agents_md: TextFile::load_all(root, &[AGENTS_MD])?.pop(),
            claude_md: workspace
                .files(&[":(glob)**/CLAUDE.md"])?
                .into_iter()
                .collect(),
            skills: TextFile::load_all(root, &[":(glob).agents/skills/*/SKILL.md"])?,
            claude_skills: std::fs::read_link(root.join(CLAUDE_SKILLS)).ok(),
            claude_skill_files: workspace
                .files(&[":(glob).claude/skills/**"])?
                .into_iter()
                .filter(|path| path != CLAUDE_SKILLS)
                .collect(),
        })
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
    pub(super) fn agent_files(&self) -> Vec<Finding> {
        let files = &self.agent_files;
        let mut findings = Vec::new();
        if files.agents_md.is_none() {
            findings.push(Finding {
                path: AGENTS_MD.into(),
                line: 1,
                message: "no AGENTS.md: write the agents' instructions there".to_owned(),
            });
        }
        findings.extend(files.claude_md.iter().map(|path| {
            Finding {
                path: path.into(),
                line: 1,
                message:
                    "a CLAUDE.md: move what it says to AGENTS.md, which Claude Code reads too, \
                      and delete it"
                        .to_owned(),
            }
        }));
        for skill in &files.skills {
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
        findings.extend(files.claude_skill_files.iter().map(|path| Finding {
            path: path.into(),
            line: 1,
            message: format!("a skill under {CLAUDE_SKILLS}: move it to {SKILLS}"),
        }));
        let linked = files.claude_skills.as_deref() == Some(Path::new("../.agents/skills"));
        if !files.skills.is_empty() && !linked {
            findings.push(Finding {
                path: CLAUDE_SKILLS.into(),
                line: 1,
                message: format!(
                    "{CLAUDE_SKILLS} isn't a symlink to `../{SKILLS}`: make it one, with \
                     `ln -s ../{SKILLS} {CLAUDE_SKILLS}`"
                ),
            });
        }
        findings
    }
}
