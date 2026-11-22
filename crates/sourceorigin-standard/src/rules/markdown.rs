//! The repository's Markdown files as the docs rules read them: their lines
//! outside fenced code, and the text of each outside inline code.

use std::path::Path;

use super::Repository;
use super::text_file::TextFile;

/// What a synced skill's `SKILL.md` holds, as `skills-synced` writes it: the
/// crate owns its files, so the docs rules leave them alone.
const SYNCED: &str = "<!-- Synced from the sourceorigin-standard crate";

/// A line of a Markdown file outside fenced code.
pub(super) struct MarkdownLine<'a> {
    /// From 1.
    pub(super) number: usize,
    pub(super) text: &'a str,
}

impl TextFile {
    /// Its lines outside fenced code blocks.
    pub(super) fn markdown_lines(&self) -> impl Iterator<Item = MarkdownLine<'_>> {
        let mut fenced = false;
        self.text
            .lines()
            .enumerate()
            .filter_map(move |(index, text)| {
                let trimmed = text.trim_start();
                let fence = trimmed.starts_with("```") || trimmed.starts_with("~~~");
                if fence {
                    fenced = !fenced;
                }
                (!fence && !fenced).then_some(MarkdownLine {
                    number: index + 1,
                    text,
                })
            })
    }
}

impl MarkdownLine<'_> {
    /// Which of its bytes are inline code, backticks included.
    fn code(&self) -> Vec<bool> {
        let mut code = vec![false; self.text.len()];
        let mut open = None;
        for (at, _) in self.text.match_indices('`') {
            match open.take() {
                Some(start) => code[start..=at].fill(true),
                None => open = Some(at),
            }
        }
        code
    }

    /// The line without its inline code.
    pub(super) fn outside_code(&self) -> String {
        let code = self.code();
        self.text
            .char_indices()
            .filter(|&(at, _)| !code[at])
            .map(|(_, c)| c)
            .collect()
    }
}

impl Repository {
    /// The Markdown files the docs rules read: all but a synced skill's.
    pub(super) fn docs(&self) -> impl Iterator<Item = &TextFile> {
        let synced: Vec<&Path> = self
            .markdown
            .iter()
            .filter(|file| file.path.ends_with("SKILL.md") && file.text.contains(SYNCED))
            .filter_map(|file| file.path.parent())
            .collect();
        self.markdown
            .iter()
            .filter(move |file| !synced.iter().any(|dir| file.path.starts_with(dir)))
    }
}
