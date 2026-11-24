//! `wording`: the docs name things as the repository does.

use super::fix::{Checked, Fix};
use super::markdown::MarkdownLine;
use super::{Finding, Repository};
use crate::config::{self, Listed};

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
    fn rejected<'w>(&self, wording: &'w [(Listed, String)]) -> Vec<Rejected<'w>> {
        let text = self.text;
        let prose = self.prose();
        let mut found: Vec<Rejected<'w>> = wording
            .iter()
            .flat_map(|(rejected, preferred)| {
                let spelling = rejected.value.as_str();
                text.match_indices(spelling)
                    .map(move |(start, _)| Rejected {
                        start,
                        end: start + spelling.len(),
                        spelling,
                        preferred: preferred.as_str(),
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
    /// Each spelling `[wording]` rejects, as a word in the prose of a
    /// Markdown file but a synced skill's, fixed by writing the preferred
    /// one in its place.
    pub(super) fn wording(&self) -> Checked {
        let mut checked = Checked::default();
        for file in self.docs() {
            for line in file.markdown_lines() {
                let found = line.rejected(&self.config.wording);
                if found.is_empty() {
                    continue;
                }
                let mut fixed = line.text.to_owned();
                for found in found.iter().rev() {
                    fixed.replace_range(found.start..found.end, found.preferred);
                }
                checked.findings.extend(found.iter().map(|found| Finding {
                    path: file.path.clone(),
                    line: line.number,
                    message: format!("write `{}`, not `{}`", found.preferred, found.spelling),
                }));
                let number = line.number;
                checked.fixes.push(Fix::lines(
                    file.path.clone(),
                    format!("write the names `[wording]` prefers on line {number}"),
                    number..number + 1,
                    fixed,
                ));
            }
        }
        checked
    }
}
