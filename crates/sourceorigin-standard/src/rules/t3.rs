//! `t3.json`: the scripts T3 Code runs in a worktree.

use std::path::Path;

use serde::Deserialize;

use super::Error;

/// The file, from the repository's root.
pub(super) const PATH: &str = "t3.json";

/// `t3.json`, with its text for the lines of its scripts.
pub(super) struct T3 {
    text: String,
    pub(super) scripts: Vec<Script>,
}

/// A script T3 Code offers, or runs on an event.
#[derive(Deserialize)]
pub(super) struct Script {
    pub(super) command: String,
}

/// `t3.json` as written.
#[derive(Deserialize)]
struct File {
    #[serde(default)]
    scripts: Vec<Script>,
}

impl T3 {
    /// `t3.json` under `root`, or `None` when there is none.
    pub(super) fn load(root: &Path) -> Result<Option<Self>, Error> {
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
        let file: File = serde_json::from_str(&text).map_err(|source| Error::Json {
            path: PATH.into(),
            source,
        })?;
        Ok(Some(Self {
            text,
            scripts: file.scripts,
        }))
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
