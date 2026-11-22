//! A Rust file's comments and doc blocks, from proc-macro2's tokens: doc
//! comments are tokens (`#[doc = "…"]`), and every other comment is in a
//! gap between tokens.

use std::ops::Range;

use proc_macro2::{Delimiter, LineColumn, Span, TokenStream, TokenTree};

use super::{Finding, Repository};
use crate::config;

/// A comment that isn't a doc comment, such as `// TODO` or `/* … */`.
/// Consecutive `//` lines are one comment.
#[derive(Debug)]
pub(super) struct Comment {
    /// Its first line, from 1.
    pub(super) line: usize,
    /// Its last line, from 1.
    end_line: usize,
    /// Whether it is a `//` comment, which the next line can continue.
    slashes: bool,
    /// Each of its lines' text, without `//`, `/*` or `*/`.
    pub(super) lines: Vec<String>,
}

impl Comment {
    /// Whether its first line starts with one of the [`MARKERS`].
    pub(super) fn marked(&self) -> bool {
        self.lines
            .first()
            .is_some_and(|first| MARKERS.iter().any(|marker| first.starts_with(marker)))
    }
}

/// What a Rust comment that isn't a doc comment may start with.
pub(super) const MARKERS: [&str; 1] = ["SAFETY:"];

/// The doc comments and `#[doc]` attributes on one item, or inside one
/// module.
#[derive(Debug)]
pub(super) struct DocBlock {
    /// Its first line, from 1.
    pub(super) line: usize,
    /// How many lines of prose its docs hold: code blocks don't count.
    pub(super) lines: usize,
    /// Whether they are inner docs (`//!`), documenting the item they sit in.
    pub(super) inner: bool,
    /// Whether the docs read so far end inside a code block.
    in_code: bool,
}

impl DocBlock {
    /// Counts the prose lines of one doc attribute, whose `group` is
    /// `[doc = …]`: a string's lines outside code fences, or one line for
    /// anything else, such as `include_str!(…)`.
    fn add(&mut self, group: &proc_macro2::Group) {
        let value = group.stream().into_iter().nth(2);
        let text = match value {
            Some(TokenTree::Literal(literal)) => {
                syn::parse2::<syn::LitStr>(TokenTree::Literal(literal).into())
                    .map(|text| text.value())
                    .ok()
            }
            _ => None,
        };
        let Some(text) = text else {
            self.lines += 1;
            return;
        };
        for line in text.lines() {
            if line.trim_start().starts_with("```") {
                self.in_code = !self.in_code;
            } else if !self.in_code {
                self.lines += 1;
            }
        }
    }
}

/// Where each line of a file starts, to turn a span's line and column into
/// a byte offset.
struct Lines<'a> {
    text: &'a str,
    starts: Vec<usize>,
}

impl<'a> Lines<'a> {
    fn of(text: &'a str) -> Self {
        let starts = std::iter::once(0)
            .chain(text.match_indices('\n').map(|(offset, _)| offset + 1))
            .collect();
        Self { text, starts }
    }

    /// The byte offset of `position`, whose column counts characters.
    fn offset(&self, position: LineColumn) -> usize {
        let start = self
            .starts
            .get(position.line.saturating_sub(1))
            .copied()
            .unwrap_or(self.text.len());
        self.text[start..]
            .char_indices()
            .nth(position.column)
            .map_or(self.text.len(), |(offset, _)| start + offset)
    }

    /// The line, from 1, of byte `offset`.
    fn line(&self, offset: usize) -> usize {
        self.starts.partition_point(|&start| start <= offset)
    }
}

/// Reads comments and doc blocks out of a file's tokens.
pub(super) struct Scan<'a> {
    lines: Lines<'a>,
    /// The byte range of every token, delimiters included.
    covered: Vec<Range<usize>>,
    doc_blocks: Vec<DocBlock>,
}

impl<'a> Scan<'a> {
    /// The comments and doc blocks of `text`, which `tokens` were lexed
    /// from.
    pub(super) fn of(text: &'a str, tokens: TokenStream) -> (Vec<Comment>, Vec<DocBlock>) {
        let mut scan = Self {
            lines: Lines::of(text),
            covered: Vec::new(),
            doc_blocks: Vec::new(),
        };
        scan.walk(tokens);
        (scan.comments(), scan.doc_blocks)
    }

    /// Records the spans of `tokens`, and their doc blocks, at any depth.
    fn walk(&mut self, tokens: TokenStream) {
        let mut tokens = tokens.into_iter().peekable();
        let mut block: Option<(DocBlock, bool)> = None;
        while let Some(token) = tokens.next() {
            if !matches!(token, TokenTree::Group(_)) {
                self.cover(token.span());
            }
            let attribute = match &token {
                TokenTree::Punct(pound) if pound.as_char() == '#' => {
                    let inner = matches!(tokens.peek(), Some(TokenTree::Punct(bang)) if bang.as_char() == '!');
                    if inner && let Some(bang) = tokens.next() {
                        self.cover(bang.span());
                    }
                    match tokens.next_if(|next| {
                        matches!(next, TokenTree::Group(group) if group.delimiter() == Delimiter::Bracket)
                    }) {
                        Some(TokenTree::Group(group)) => Some((group, inner, pound.span())),
                        _ => None,
                    }
                }
                TokenTree::Group(group) => {
                    self.cover(group.span_open());
                    self.cover(group.span_close());
                    self.walk(group.stream());
                    None
                }
                TokenTree::Ident(_) | TokenTree::Punct(_) | TokenTree::Literal(_) => None,
            };
            let Some((group, inner, pound)) = attribute else {
                self.doc_blocks.extend(block.take().map(|(block, _)| block));
                continue;
            };
            self.cover(group.span_open());
            self.cover(group.span_close());
            self.walk(group.stream());
            if block.as_ref().is_some_and(|(_, style)| *style != inner) {
                self.doc_blocks.extend(block.take().map(|(block, _)| block));
            }
            let is_doc = matches!(group.stream().into_iter().next(), Some(TokenTree::Ident(ident)) if ident == "doc");
            if is_doc {
                let (doc_block, _) = block.get_or_insert((
                    DocBlock {
                        line: pound.start().line,
                        lines: 0,
                        inner,
                        in_code: false,
                    },
                    inner,
                ));
                doc_block.add(&group);
            }
        }
        self.doc_blocks.extend(block.map(|(block, _)| block));
    }

    fn cover(&mut self, span: Span) {
        let range = self.lines.offset(span.start())..self.lines.offset(span.end());
        if !range.is_empty() {
            self.covered.push(range);
        }
    }

    /// The comments in the gaps between tokens, which hold only whitespace
    /// and comments.
    fn comments(&mut self) -> Vec<Comment> {
        self.covered.sort_by_key(|range| range.start);
        let mut comments = Vec::new();
        let mut cursor = 0;
        let ends = self
            .covered
            .iter()
            .map(|range| (range.start, range.end))
            .chain(std::iter::once((
                self.lines.text.len(),
                self.lines.text.len(),
            )));
        for (start, end) in ends {
            if start > cursor {
                self.gap(cursor..start, &mut comments);
            }
            cursor = cursor.max(end);
        }
        comments
    }

    /// The comments in `gap`, a byte range between two tokens.
    fn gap(&self, gap: Range<usize>, comments: &mut Vec<Comment>) {
        let text = self.lines.text;
        let first = comments.len();
        let mut at = gap.start;
        while at < gap.end {
            let rest = &text[at..gap.end];
            let Some(skip) = rest.find(|c: char| !c.is_whitespace()) else {
                return;
            };
            at += skip;
            let rest = &text[at..gap.end];
            let (body, length) = if let Some(line) = rest.strip_prefix("//") {
                let body = line.split('\n').next().unwrap_or_default();
                (body, body.len() + 2)
            } else if rest.starts_with("/*") {
                let length = Self::block_length(rest);
                (
                    rest[2..length]
                        .strip_suffix("*/")
                        .unwrap_or(&rest[2..length]),
                    length,
                )
            } else {
                let length = rest.chars().next().map_or(1, char::len_utf8);
                at += length;
                continue;
            };
            let line = self.lines.line(at);
            let continues = rest.starts_with("//")
                && comments.len() > first
                && comments
                    .last()
                    .is_some_and(|last: &Comment| last.slashes && last.end_line + 1 == line);
            match comments.last_mut() {
                Some(last) if continues => {
                    last.end_line = line;
                    last.lines.push(body.trim().to_owned());
                }
                _ => comments.push(Comment {
                    line,
                    end_line: line,
                    slashes: rest.starts_with("//"),
                    lines: body.lines().map(|line| line.trim().to_owned()).collect(),
                }),
            }
            at += length;
        }
    }

    /// The length of the block comment `text` starts with, `*/` included,
    /// counting nested ones as Rust does.
    fn block_length(text: &str) -> usize {
        let mut depth = 0;
        let mut at = 0;
        while at < text.len() {
            let rest = &text[at..];
            if rest.starts_with("/*") {
                depth += 1;
                at += 2;
            } else if rest.starts_with("*/") {
                depth -= 1;
                at += 2;
                if depth == 0 {
                    return at;
                }
            } else {
                at += rest.chars().next().map_or(1, char::len_utf8);
            }
        }
        text.len()
    }
}

impl Repository {
    /// Each comment, in a file that isn't generated, that is neither a doc
    /// comment nor starts with one of the [`MARKERS`].
    pub(super) fn rust_comments(&self) -> Vec<Finding> {
        self.sources
            .iter()
            .filter(|source| !source.generated)
            .flat_map(|source| {
                source
                    .comments
                    .iter()
                    .filter(|comment| !comment.marked())
                    .map(|comment| Finding {
                        path: source.path.clone(),
                        line: comment.line,
                        message: format!(
                            "a plain comment: move it into the item's `///` docs, or remove it; \
                             only {} comments stay",
                            MARKERS.join(" and ")
                        ),
                    })
            })
            .collect()
    }

    /// Each item's or module's docs, in a file that isn't generated, whose
    /// prose spans more lines than `[comments] doc-lines`. A crate root's
    /// inner docs, the crate's own documentation, are exempt.
    pub(super) fn doc_length(&self) -> Vec<Finding> {
        let limit = self.config.comments.docs;
        self.sources
            .iter()
            .filter(|source| !source.generated)
            .flat_map(|source| {
                let crate_root = self.metadata.is_crate_root(&source.path);
                source
                    .doc_blocks
                    .iter()
                    .filter(move |block| block.lines > limit && !(block.inner && crate_root))
                    .map(|block| Finding {
                        path: source.path.clone(),
                        line: block.line,
                        message: format!(
                            "docs of {} lines, code blocks aside, over the {limit} of {}'s \
                             `[comments] doc-lines`: keep what a reader of the item needs, and \
                             move the rest to the crate's docs",
                            block.lines,
                            config::PATH
                        ),
                    })
            })
            .collect()
    }
}
