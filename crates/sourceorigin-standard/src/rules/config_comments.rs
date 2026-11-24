//! `config-comments`: comments in manifests, justfiles and workflows stay
//! short.

use std::path::{Path, PathBuf};

use super::config_syntax::Unreadable;
use super::{Error, Finding, Repository};
use crate::config;
use crate::workspace::Workspace;

/// The files whose comments the rule caps, as git pathspecs.
const FILES: [&str; 5] = [
    ":(glob)**/Cargo.toml",
    ":(glob)**/justfile",
    ":(glob)**/*.just",
    ":(glob).github/**/*.yml",
    ":(glob).github/**/*.yaml",
];

/// A file whose comments are `#` lines, and its comment blocks.
pub(super) struct Commented {
    /// From the repository's root.
    path: PathBuf,
    /// Each run of consecutive comment lines: its first line, from 1, and
    /// its length.
    blocks: Vec<(usize, usize)>,
}

impl Commented {
    /// Every manifest, justfile and `.github` workflow under `root` that is
    /// UTF-8 text; `unreadable` records the others.
    pub(super) fn load_all(root: &Path, unreadable: &mut Unreadable) -> Result<Vec<Self>, Error> {
        let mut commented = Vec::new();
        for path in Workspace::new(root).files(&FILES)? {
            let path = PathBuf::from(path);
            if let Some(text) = unreadable.text(root, &path)? {
                commented.push(Self::of(path, &text));
            }
        }
        Ok(commented)
    }

    /// The comment blocks of `text`: lines starting with `#`, except a
    /// shebang (`#!`). A comment after a value on the same line isn't one.
    /// In YAML, lines inside a block scalar (`run: |`, `body: >-`) are its
    /// text, not comments.
    fn of(path: PathBuf, text: &str) -> Self {
        let yaml = path
            .extension()
            .is_some_and(|extension| extension == "yml" || extension == "yaml");
        let mut blocks: Vec<(usize, usize)> = Vec::new();
        let mut previous = false;
        let mut scalar: Option<usize> = None;
        for (index, raw) in text.lines().enumerate() {
            let line = raw.trim_start();
            let indent = raw.len() - line.len();
            if let Some(key) = scalar {
                if line.is_empty() || indent > key {
                    previous = false;
                    continue;
                }
                scalar = None;
            }
            if yaml && Self::opens_block_scalar(line) {
                scalar = Some(indent);
            }
            let comment = line.starts_with('#') && !line.starts_with("#!");
            match blocks.last_mut() {
                Some((_, length)) if comment && previous => *length += 1,
                _ if comment => blocks.push((index + 1, 1)),
                _ => {}
            }
            previous = comment;
        }
        Self { path, blocks }
    }

    /// Whether a YAML `line` ends with a block scalar's indicator, such as
    /// `run: |`, `- >-` or `body: |2`.
    fn opens_block_scalar(line: &str) -> bool {
        let line = line.trim_end();
        let (before, indicator) = line.rsplit_once(' ').unwrap_or(("", line));
        let mut chars = indicator.chars();
        matches!(chars.next(), Some('|' | '>'))
            && chars.all(|c| matches!(c, '+' | '-') || c.is_ascii_digit())
            && (before.ends_with(':') || before.ends_with('-'))
    }
}

impl Repository {
    /// Each comment block over `[comments] block-lines`, and each file
    /// whose comments total more than `[comments] file-lines`.
    pub(super) fn config_comments(&self) -> Vec<Finding> {
        let comments = &self.config.comments;
        let mut findings = Vec::new();
        for file in &self.commented {
            findings.extend(
                file.blocks
                    .iter()
                    .filter(|&&(_, length)| length > comments.block)
                    .map(|&(line, length)| Finding {
                        path: file.path.clone(),
                        line,
                        message: format!(
                            "a comment of {length} lines, over the {} of {}'s \
                             `[comments] block-lines`: keep the line it explains needs, and \
                             move the rest to the docs",
                            comments.block,
                            config::PATH
                        ),
                    }),
            );
            let total: usize = file.blocks.iter().map(|&(_, length)| length).sum();
            if total > comments.file {
                findings.push(Finding {
                    path: file.path.clone(),
                    line: 1,
                    message: format!(
                        "{total} comment lines, over the {} of {}'s `[comments] file-lines`: \
                         shorten them, and move the rest to the docs",
                        comments.file,
                        config::PATH
                    ),
                });
            }
        }
        findings
    }
}
