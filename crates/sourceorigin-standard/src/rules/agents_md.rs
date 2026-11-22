//! `agents-md`: AGENTS.md maps the workspace, briefly.

use std::path::Path;

use super::agent_files::AGENTS_MD;
use super::{Finding, Repository};

/// The most lines AGENTS.md may hold.
const LINES: usize = 150;

impl Repository {
    /// Each workspace crate AGENTS.md names neither by its name in
    /// backticks nor by its directory, and an AGENTS.md over [`LINES`].
    /// Without AGENTS.md, `agent-files` fails instead.
    pub(super) fn agents_md(&self) -> Vec<Finding> {
        let Some(agents_md) = &self.agent_files.agents_md else {
            return Vec::new();
        };
        let text = &agents_md.text;
        let root = &self.metadata.workspace_root;
        let mut findings: Vec<Finding> = self
            .metadata
            .packages
            .iter()
            .filter_map(|package| {
                let dir = package.manifest_path.parent()?.strip_prefix(root).ok()?;
                let named = text.contains(&format!("`{}`", package.name))
                    || (dir != Path::new("") && text.contains(&*dir.to_string_lossy()));
                (!named).then(|| Finding {
                    path: AGENTS_MD.into(),
                    line: 1,
                    message: format!(
                        "AGENTS.md doesn't name the crate `{}`: add it to the map of the \
                         workspace's crates, with a line saying what it is",
                        package.name
                    ),
                })
            })
            .collect();
        let lines = text.lines().count();
        if lines > LINES {
            findings.push(Finding {
                path: AGENTS_MD.into(),
                line: LINES + 1,
                message: format!(
                    "AGENTS.md holds {lines} lines, over {LINES}: keep what an agent can't infer \
                     from the code, and move the rest to the docs it links"
                ),
            });
        }
        findings
    }
}
