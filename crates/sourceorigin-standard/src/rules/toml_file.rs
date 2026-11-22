//! The TOML files the rules read, each deserialized into the shape its rule
//! needs, keeping its text for the lines of what the rule finds.

use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use toml::Spanned;

use super::Error;
use crate::config::line_of;

/// A TOML file, as `T` reads it.
pub(super) struct TomlFile<T> {
    /// From the repository's root.
    pub(super) path: PathBuf,
    text: String,
    pub(super) value: T,
}

impl<T: DeserializeOwned> TomlFile<T> {
    /// The file at `path`, from `root`, or `None` when there is none.
    pub(super) fn load(root: &Path, path: impl Into<PathBuf>) -> Result<Option<Self>, Error> {
        let path = path.into();
        match std::fs::read_to_string(root.join(&path)) {
            Ok(text) => Self::parse(path, text).map(Some),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(source) => Err(Error::Read { path, source }),
        }
    }

    /// The file at `path`, from `root`, or an empty one when there is
    /// none, which a fix writes.
    pub(super) fn load_or_empty(root: &Path, path: impl Into<PathBuf>) -> Result<Self, Error>
    where
        T: Default,
    {
        let path = path.into();
        Ok(Self::load(root, &path)?.unwrap_or_else(|| Self {
            path,
            text: String::new(),
            value: T::default(),
        }))
    }

    /// The file at `path`, from `root`, which must exist.
    pub(super) fn require(root: &Path, path: impl Into<PathBuf>) -> Result<Self, Error> {
        let path = path.into();
        let text = std::fs::read_to_string(root.join(&path)).map_err(|source| Error::Read {
            path: path.clone(),
            source,
        })?;
        Self::parse(path, text)
    }

    fn parse(path: PathBuf, text: String) -> Result<Self, Error> {
        let value = toml::from_str(&text).map_err(|error| Error::Toml {
            path: path.clone(),
            message: error.to_string(),
        })?;
        Ok(Self { path, text, value })
    }
}

impl<T> TomlFile<T> {
    /// The line, from 1, where `spanned` starts.
    pub(super) fn line<S>(&self, spanned: &Spanned<S>) -> usize {
        line_of(&self.text, spanned.span().start)
    }
}
