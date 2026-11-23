//! `docs-summary`: every page of the book is reachable from its table of
//! contents.

use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};

use serde::Deserialize;

use super::text_file::TextFile;
use super::{Finding, Repository};

/// mdBook's configuration, from the repository's root.
pub(super) const BOOK: &str = "book.toml";

/// `book.toml`, reduced to where the pages are.
#[derive(Deserialize)]
pub(super) struct Book {
    book: Option<BookTable>,
}

#[derive(Deserialize)]
struct BookTable {
    src: Option<String>,
}

impl Book {
    /// The directory of its pages, from the repository's root.
    fn src(&self) -> PathBuf {
        PathBuf::from(
            self.book
                .as_ref()
                .and_then(|book| book.src.as_deref())
                .unwrap_or("src"),
        )
    }
}

impl TextFile {
    /// The pages the file links, with `](…)`, from the repository's root.
    fn links(&self) -> Vec<PathBuf> {
        let targets = self
            .text
            .match_indices("](")
            .filter_map(|(at, _)| self.text[at + 2..].split([')', '#', ' ']).next());
        self.resolve(targets)
    }

    /// The files the page includes, with `{{#include …}}`, from the
    /// repository's root.
    fn includes(&self) -> Vec<PathBuf> {
        let targets = self
            .text
            .match_indices("{{#include ")
            .filter_map(|(at, directive)| {
                self.text[at + directive.len()..].split(['}', ':']).next()
            });
        self.resolve(targets)
    }

    /// `targets`, relative to the file, from the repository's root, with
    /// their `.` and `..` resolved; URLs and empty targets dropped.
    fn resolve<'a>(&self, targets: impl Iterator<Item = &'a str>) -> Vec<PathBuf> {
        let dir = self.path.parent().unwrap_or(Path::new(""));
        targets
            .map(str::trim)
            .filter(|target| !target.is_empty() && !target.contains("://"))
            .map(|target| {
                let mut path = PathBuf::new();
                for component in dir.join(target).components() {
                    match component {
                        Component::ParentDir => {
                            path.pop();
                        }
                        Component::CurDir => {}
                        other => path.push(other),
                    }
                }
                path
            })
            .collect()
    }
}

impl Repository {
    /// Each page under the book's `src` that its `SUMMARY.md` doesn't link
    /// and no page includes; none while a file under `src` isn't UTF-8
    /// text and `config-syntax`'s deviation doesn't excuse it, as its links
    /// and includes are unknown.
    pub(super) fn docs_summary(&self) -> Vec<Finding> {
        let Some(book) = &self.book else {
            return Vec::new();
        };
        let src = book.value.src();
        let summary = src.join("SUMMARY.md");
        if self
            .unreadable_unexcused()
            .any(|path| path.starts_with(&src))
        {
            return Vec::new();
        }
        let pages: Vec<&TextFile> = self
            .markdown
            .iter()
            .filter(|page| page.path.starts_with(&src))
            .collect();
        let mut reached: BTreeSet<PathBuf> =
            pages.iter().flat_map(|page| page.includes()).collect();
        if let Some(summary) = pages.iter().find(|page| page.path == summary) {
            reached.extend(summary.links());
        }
        pages
            .iter()
            .filter(|page| page.path != summary && !reached.contains(&page.path))
            .map(|page| Finding {
                path: page.path.clone(),
                line: 1,
                message: format!(
                    "no page reaches this one: link it from {}, or include it in a page with \
                     `{{{{#include …}}}}`",
                    summary.display()
                ),
            })
            .collect()
    }
}
