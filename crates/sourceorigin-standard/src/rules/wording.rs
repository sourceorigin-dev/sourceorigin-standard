//! `wording`: the docs name things as the repository does, in plain words.

use std::path::Path;

use super::fix::{Checked, Fix};
use super::markdown::MarkdownLine;
use super::{Finding, Repository};
use crate::config::{self, Wording};

/// An occurrence of a rejected spelling in a line.
struct Rejected<'w> {
    /// Its bytes in the line.
    start: usize,
    end: usize,
    spelling: &'w str,
    preferred: &'w str,
}

impl MarkdownLine<'_> {
    /// Each occurrence, as a word in prose, of a rejected spelling of
    /// `wording`: its bytes, and the spelling to write instead. Occurrences
    /// don't overlap; the first wins.
    fn rejected<'w>(&self, wording: &'w Wording) -> Vec<Rejected<'w>> {
        let text = self.text;
        let prose = self.prose();
        let mut found: Vec<Rejected<'w>> = wording
            .iter()
            .flat_map(|(spelling, preferred)| {
                text.match_indices(spelling)
                    .map(move |(start, _)| Rejected {
                        start,
                        end: start + spelling.len(),
                        spelling,
                        preferred,
                    })
            })
            .filter(|found| {
                prose[found.start..found.end].iter().all(|&byte| byte)
                    && !config::joins_word(text, found.start, found.end)
            })
            .collect();
        found.sort_by_key(|found| found.start);
        let mut end = 0;
        found.retain(|found| {
            let free = found.start >= end;
            if free {
                end = found.end;
            }
            free
        });
        found
    }
}

impl Repository {
    /// The lines whose prose `wording` reads: each Markdown file's but a
    /// synced skill's, outside fenced code, and each `///` and `//!` line of
    /// a Rust file that isn't generated, outside its docs' code blocks.
    fn worded_lines(&self) -> impl Iterator<Item = (&Path, MarkdownLine<'_>)> {
        let markdown = self.docs().flat_map(|file| {
            file.markdown_lines()
                .map(move |line| (file.path.as_path(), line))
        });
        let rust = self
            .sources
            .iter()
            .filter(|source| !source.generated)
            .flat_map(|source| {
                let lines: Vec<&str> = source.text.lines().collect();
                source
                    .doc_blocks
                    .iter()
                    .flat_map(|block| &block.prose)
                    .filter_map(move |&number| {
                        let text = lines.get(number - 1)?;
                        Some((source.path.as_path(), MarkdownLine { number, text }))
                    })
            });
        markdown.chain(rust)
    }

    /// Each spelling the defaults or `[wording]` reject, as a word in the
    /// prose of a Markdown file but a synced skill's, or of a doc comment,
    /// fixed by writing the preferred one in its place.
    pub(super) fn wording(&self) -> Checked {
        let mut checked = Checked::default();
        for (path, line) in self.worded_lines() {
            let found = line.rejected(&self.config.wording);
            if found.is_empty() {
                continue;
            }
            let mut fixed = line.text.to_owned();
            for found in found.iter().rev() {
                fixed.replace_range(found.start..found.end, found.preferred);
            }
            checked.findings.extend(found.iter().map(|found| Finding {
                path: path.to_owned(),
                line: line.number,
                message: format!("write `{}`, not `{}`", found.preferred, found.spelling),
            }));
            let number = line.number;
            checked.fixes.push(Fix::lines(
                path.to_owned(),
                format!("write the words `[wording]` prefers on line {number}"),
                number..number + 1,
                fixed,
            ));
        }
        checked
    }
}
