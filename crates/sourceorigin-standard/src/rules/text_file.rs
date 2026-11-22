//! The text files the rules read line by line, such as justfiles and
//! workflows.

use std::path::{Path, PathBuf};

use super::Error;
use crate::workspace::Workspace;

/// A file's text.
pub(super) struct TextFile {
    /// From the repository's root.
    pub(super) path: PathBuf,
    pub(super) text: String,
}

impl TextFile {
    /// Every file git tracks or would track under `root` matching any of
    /// `pathspecs`.
    pub(super) fn load_all(root: &Path, pathspecs: &[&str]) -> Result<Vec<Self>, Error> {
        Workspace::new(root)
            .files(pathspecs)?
            .into_iter()
            .filter_map(|path| match std::fs::read_to_string(root.join(&path)) {
                Ok(text) => Some(Ok(Self {
                    path: PathBuf::from(path),
                    text,
                })),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                Err(source) => Some(Err(Error::Read {
                    path: PathBuf::from(path),
                    source,
                })),
            })
            .collect()
    }

    /// Every file git tracks or would track under `root` matching any of
    /// `globs`, such as `crates/*/migrations/*.sql`; none without globs.
    pub(super) fn load_globs(root: &Path, globs: &[String]) -> Result<Vec<Self>, Error> {
        if globs.is_empty() {
            return Ok(Vec::new());
        }
        let pathspecs: Vec<String> = globs.iter().map(|glob| format!(":(glob){glob}")).collect();
        Self::load_all(
            root,
            &pathspecs.iter().map(String::as_str).collect::<Vec<_>>(),
        )
    }
}
