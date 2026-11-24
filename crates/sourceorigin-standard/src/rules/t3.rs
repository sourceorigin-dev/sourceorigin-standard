//! `t3.json`: the scripts T3 Code runs in a worktree.

use std::path::Path;

use serde::Deserialize;

use super::Error;
use super::config_syntax::Unreadable;

/// The file, from the repository's root.
pub(super) const PATH: &str = "t3.json";

/// `t3.json`, with its text for the lines of its scripts.
pub(super) struct T3 {
    text: String,
    pub(super) scripts: Vec<Script>,
}

/// A script T3 Code offers, or runs on an event.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Script {
    pub(super) command: String,
    /// Whether it runs when a worktree is created.
    #[serde(default)]
    pub(super) run_on_worktree_create: bool,
}

/// `t3.json` as written.
#[derive(Deserialize)]
struct File {
    #[serde(default)]
    scripts: Vec<Script>,
}

impl T3 {
    /// `t3.json` under `root`, or `None` when there is none or it doesn't
    /// parse, which `unreadable` records.
    pub(super) fn load(root: &Path, unreadable: &mut Unreadable) -> Result<Option<Self>, Error> {
        let text = match std::fs::read_to_string(root.join(PATH)) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(source) => {
                return Err(Error::Read {
                    path: PATH.into(),
                    source,
                });
            }
        };
        match serde_json::from_str::<File>(&text) {
            Ok(file) => Ok(Some(Self {
                text,
                scripts: file.scripts,
            })),
            Err(error) => {
                unreadable.record(PATH.into(), error.line(), error.to_string());
                Ok(None)
            }
        }
    }

    /// The text of `line`, from 1.
    pub(super) fn text_of(&self, line: usize) -> &str {
        self.text
            .lines()
            .nth(line.saturating_sub(1))
            .unwrap_or_default()
    }

    /// The line, from 1, of `script`'s command.
    pub(super) fn line(&self, script: &Script) -> usize {
        let quoted = serde_json::to_string(&script.command).unwrap_or_default();
        self.text
            .lines()
            .position(|line| line.contains("\"command\"") && line.contains(&quoted))
            .map_or(1, |index| index + 1)
    }
}
