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
            let bytes = match std::fs::read(root.join(&path)) {
                Ok(bytes) => bytes,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(source) => return Err(Error::Read { path, source }),
            };
            match String::from_utf8(bytes) {
                Ok(text) => files.push(Self { path, text }),
                Err(error) => {
                    let valid = &error.as_bytes()[..error.utf8_error().valid_up_to()];
                    let line =
                        std::str::from_utf8(valid).map_or(1, |valid| valid.split('\n').count());
                    unreadable.record(path, line, "it isn't UTF-8 text".to_owned());
                }
            }
        }
        Ok(files)
    }
}
