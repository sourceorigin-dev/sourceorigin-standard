//! The text files the rules read line by line, such as justfiles and
//! workflows.

use std::path::{Path, PathBuf};

use super::Error;
use super::config_syntax::Unreadable;
use crate::workspace::Workspace;

/// A file's text.
pub(super) struct TextFile {
    /// From the repository's root.
    pub(super) path: PathBuf,
    pub(super) text: String,
}

impl TextFile {
    /// Every file git tracks or would track under `root` matching any of
    /// `pathspecs` that is UTF-8 text; `unreadable` records the others.
    pub(super) fn load_all(
        root: &Path,
        pathspecs: &[&str],
        unreadable: &mut Unreadable,
    ) -> Result<Vec<Self>, Error> {
        let mut files = Vec::new();
        for path in Workspace::new(root).files(pathspecs)? {
            let path = PathBuf::from(path);
            if let Some(text) = unreadable.text(root, &path)? {
                files.push(Self { path, text });
            }
        }
        Ok(files)
    }
}
