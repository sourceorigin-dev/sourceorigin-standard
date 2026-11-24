//! The TOML files the rules read, each deserialized into the shape its rule
//! needs, keeping its text for the lines of what the rule finds.

use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use toml::Spanned;

use super::Error;
use super::config_syntax::Unreadable;
use crate::config::line_of;

/// A TOML file, as `T` reads it.
pub(super) struct TomlFile<T> {
    /// From the repository's root.
    pub(super) path: PathBuf,
    text: String,
    pub(super) value: T,
}

impl<T: DeserializeOwned> TomlFile<T> {
    /// The file at `path`, from `root`: `None` when there is none, or when
    /// it doesn't parse as `T`, which `unreadable` records as a
    /// `config-syntax` violation.
    pub(super) fn load(
        root: &Path,
        path: impl Into<PathBuf>,
        unreadable: &mut Unreadable,
    ) -> Result<Option<Self>, Error> {
        let path = path.into();
        let text = match std::fs::read_to_string(root.join(&path)) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(source) => return Err(Error::Read { path, source }),
        };
        match toml::from_str(&text) {
            Ok(value) => Ok(Some(Self { path, text, value })),
            Err(error) => {
                let line = error.span().map_or(1, |span| line_of(&text, span.start));
                unreadable.record(path, line, error.message().to_owned());
                Ok(None)
            }
        }
    }
}

impl<T: DeserializeOwned + Default> TomlFile<T> {
    /// The file at `path`, from `root`, or an empty one when there is none,
    /// which a fix writes; `None` when it doesn't parse, which `unreadable`
    /// records.
    pub(super) fn load_or_empty(
        root: &Path,
        path: impl Into<PathBuf>,
        unreadable: &mut Unreadable,
    ) -> Result<Option<Self>, Error> {
        let path = path.into();
        if !root.join(&path).exists() {
            return Ok(Some(Self {
                path,
                text: String::new(),
                value: T::default(),
            }));
        }
        Self::load(root, path, unreadable)
    }
}

impl<T> TomlFile<T> {
    /// The line, from 1, where `spanned` starts.
    pub(super) fn line<S>(&self, spanned: &Spanned<S>) -> usize {
        self.line_at(spanned.span().start)
    }

    /// The line, from 1, holding byte `offset`.
    pub(super) fn line_at(&self, offset: usize) -> usize {
        line_of(&self.text, offset)
    }
}
