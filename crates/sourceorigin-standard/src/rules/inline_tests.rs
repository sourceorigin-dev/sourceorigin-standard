//! `inline-tests`: an inline test module that outgrows its share of a file
//! moves to a file of its own.

use std::ops::RangeInclusive;
use std::path::{Path, PathBuf};

use proc_macro2::{TokenStream, TokenTree};
use syn::spanned::Spanned;

use super::fix::{Checked, Fix};
use super::modules::Modules;
use super::source::Source;
use super::test_code::TestAttributes;
use super::{Finding, Repository};

/// The share of its file's lines, in percent, an inline test module may
/// make up.
const MAX_SHARE: usize = 30;

/// The fewest lines an inline test module spans before its share counts.
const MIN_LINES: usize = 50;

/// An inline `#[cfg(test)]` module at a file's top level that spans at
/// least [`MIN_LINES`] lines and more than [`MAX_SHARE`] of the file's.
pub(super) struct InlineTests<'a> {
    pub(super) source: &'a Source,
    item: &'a syn::ItemMod,
    /// From its first attribute's line to its closing brace's, from 1.
    lines: RangeInclusive<usize>,
}

impl Repository {
    /// Each inline test module over its share of its file, in a file a
    /// target reaches outside test code.
    pub(super) fn oversized_tests(&self) -> impl Iterator<Item = InlineTests<'_>> {
        self.sources
            .iter()
            .filter(|source| {
                !source.generated
                    && !self.modules.of_file(&source.path).is_empty()
                    && !self.modules.is_test(&source.path)
            })
            .flat_map(|source| {
                let file_lines = source.text.lines().count();
                source.syntax.items.iter().filter_map(move |item| {
                    let syn::Item::Mod(item) = item else {
                        return None;
                    };
                    let (brace, _) = item.content.as_ref()?;
                    if !item.attrs.cfg_test() {
                        return None;
                    }
                    let first = item.attrs.first().map_or_else(
                        || item.mod_token.span.start().line,
                        |attribute| attribute.span().start().line,
                    );
                    let lines = first..=brace.span.close().start().line;
                    let count = lines.clone().count();
                    (count >= MIN_LINES && count * 100 > MAX_SHARE * file_lines).then_some(
                        InlineTests {
                            source,
                            item,
                            lines,
                        },
                    )
                })
            })
    }

    /// Each inline test module over its share of its file, and the fix
    /// moving it to a file of its own.
    pub(super) fn inline_tests(&self) -> Checked {
        let mut checked = Checked::default();
        for tests in self.oversized_tests() {
            let child = self
                .child_dir(&tests.source.path)
                .map(|(dir, _)| dir.join(format!("{}.rs", tests.item.ident)));
            let target = child.as_ref().map_or_else(
                || format!("a file of its own, `mod {};`", tests.item.ident),
                |child| format!("`{}`", child.display()),
            );
            let fixes = self.move_out(&tests);
            checked.findings.push(Finding {
                path: tests.source.path.clone(),
                line: *tests.lines.start(),
                message: format!(
                    "the inline tests of `{}` span {} of the file's {} lines, over \
                     {MAX_SHARE}%: move them to {target}{}",
                    tests.item.ident,
                    tests.lines.clone().count(),
                    tests.source.text.lines().count(),
                    fixes.as_ref().err().map_or_else(String::new, |why| format!(
                        " (by hand: `--fix` doesn't, as {why})"
                    ))
                ),
            });
            checked.fixes.extend(fixes.unwrap_or_default());
        }
        checked
    }

    /// The directory the child modules of the module at `path` live in,
    /// and where the file moves first for it to have children, such as
    /// `foo.rs` to `foo/mod.rs`. `None` for a crate root other than
    /// `lib.rs` or `main.rs`, such as `src/bin/tool.rs`, whose child would
    /// be a target of its own.
    fn child_dir(&self, path: &Path) -> Option<(PathBuf, Option<PathBuf>)> {
        let name = path.file_name()?;
        let root = self.metadata.is_crate_root(path);
        let dir = Modules::child_dir(path, root);
        if root {
            return (name == "lib.rs" || name == "main.rs").then_some((dir, None));
        }
        let moved = (name != "mod.rs").then(|| dir.join("mod.rs"));
        Some((dir, moved))
    }

    /// The fixes moving `tests` to a file of its own, or why that needs
    /// judgement.
    fn move_out(&self, tests: &InlineTests<'_>) -> Result<Vec<Fix>, String> {
        let source = tests.source;
        let ident = &tests.item.ident;
        let lines: Vec<&str> = source.text.lines().collect();
        let (open, close) = tests.item.content.as_ref().map_or_else(
            || {
                (
                    tests.item.mod_token.span.start(),
                    tests.item.mod_token.span.start(),
                )
            },
            |(brace, _)| (brace.span.open().start(), brace.span.close().start()),
        );
        let line = |at: usize| lines.get(at - 1).copied().unwrap_or_default();
        let header: String = line(open.line).chars().take(open.column).collect();
        let alone = |text: String| text.trim().is_empty();
        if tests.item.mod_token.span.start().line != open.line
            || !alone(line(open.line).chars().skip(open.column + 1).collect())
            || !alone(line(close.line).chars().take(close.column).collect())
            || !alone(line(close.line).chars().skip(close.column + 1).collect())
        {
            return Err("its braces share their lines with code".to_owned());
        }
        let Some((dir, moved)) = self.child_dir(&source.path) else {
            return Err("its file is a binary's root, beside which a file is a target".to_owned());
        };
        if moved.is_some() && self.named_by_path(&source.path) {
            return Err("a `#[path]` attribute names its file, which would move".to_owned());
        }
        let child = dir.join(format!("{ident}.rs"));
        if self.files.contains(&child) {
            return Err(format!("`{}` exists", child.display()));
        }
        let child_dir = dir.join(ident.to_string());
        if self.files.iter().any(|file| file.starts_with(&child_dir)) {
            return Err(format!("`{}/` exists", child_dir.display()));
        }
        let body = Body(lines.get(open.line..close.line - 1).unwrap_or_default());
        let outside: Vec<&str> = lines
            .iter()
            .enumerate()
            .filter(|(at, _)| !tests.lines.contains(&(at + 1)))
            .map(|(_, line)| *line)
            .collect();
        if body.0.iter().any(|line| line.contains("#[path"))
            || (moved.is_some() && Source::names_files(&outside.join("\n")))
        {
            return Err("moving it would change what a relative path in it names".to_owned());
        }
        let Some(text) = body.text(moved.is_some()) else {
            return Err("an include macro in it can't be pointed one directory up".to_owned());
        };
        let mut fixes = vec![
            Fix::lines(
                &source.path,
                format!("move the tests of `{ident}` to `{}`", child.display()),
                open.line..close.line + 1,
                format!("{};", header.trim_end()),
            ),
            Fix::write(
                child,
                format!("hold the tests of `{}`", source.path.display()),
                text,
            )
            .resolving(source.path.clone()),
        ];
        if let Some(moved) = moved {
            fixes.push(Fix::move_to(
                &source.path,
                format!(
                    "move to `{}`, as it now has a child module",
                    moved.display()
                ),
                moved,
            ));
        }
        Ok(fixes)
    }
}

/// The lines between an inline module's braces.
struct Body<'a>(&'a [&'a str]);

impl Body<'_> {
    /// The macros that read a file relative to the one they are in.
    const INCLUDES: [&str; 3] = ["include", "include_str", "include_bytes"];

    /// The text of the file the module moves to, `deeper` in a directory of
    /// its own when its parent is `foo.rs`: the lines without the module's
    /// indentation, and each relative path an include macro reads, as a
    /// plain or raw string, one directory up when `deeper`. `None` when the
    /// lines don't read as Rust tokens, so an include can't be found.
    fn text(&self, deeper: bool) -> Option<String> {
        let text = self.dedented();
        if !deeper {
            return Some(text);
        }
        let mut includes = Vec::new();
        Self::includes(text.parse::<TokenStream>().ok()?, &mut includes);
        includes.sort_unstable();
        let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
        for (line, column) in includes.into_iter().rev() {
            if let Some(line) = lines.get_mut(line - 1) {
                let at = line
                    .char_indices()
                    .nth(column)
                    .map_or(line.len(), |(at, _)| at);
                line.insert_str(at, "../");
            }
        }
        Some(lines.join("\n") + "\n")
    }

    /// Where each relative path an include macro of `tokens` reads starts:
    /// its line, from 1, and character, after the literal's quote.
    fn includes(tokens: TokenStream, found: &mut Vec<(usize, usize)>) {
        let mut tokens = tokens.into_iter().peekable();
        while let Some(token) = tokens.next() {
            match token {
                TokenTree::Ident(ident) if Self::INCLUDES.iter().any(|name| ident == name) => {
                    if !matches!(tokens.peek(), Some(TokenTree::Punct(bang)) if bang.as_char() == '!')
                    {
                        continue;
                    }
                    tokens.next();
                    let Some(TokenTree::Group(arguments)) = tokens.next() else {
                        continue;
                    };
                    if let Some(TokenTree::Literal(literal)) = arguments.stream().into_iter().next()
                    {
                        let written = literal.to_string();
                        let quote = match written.strip_prefix('r') {
                            Some(raw) => 2 + raw.len() - raw.trim_start_matches('#').len(),
                            None => 1,
                        };
                        let start = literal.span().start();
                        if written.starts_with(['"', 'r'])
                            && !written[quote.min(written.len())..].starts_with('/')
                        {
                            found.push((start.line, start.column + quote));
                        }
                    }
                }
                TokenTree::Group(group) => Self::includes(group.stream(), found),
                TokenTree::Ident(_) | TokenTree::Punct(_) | TokenTree::Literal(_) => {}
            }
        }
    }

    /// The lines without the module's indentation, except those inside a
    /// string literal, whose text the indentation is part of.
    fn dedented(&self) -> String {
        let literal = self.literal_lines();
        let kept = |at: &usize| !literal.contains(&(at + 1));
        let indent = self
            .0
            .iter()
            .enumerate()
            .filter(|(at, line)| kept(at) && !line.trim().is_empty())
            .map(|(_, line)| line.len() - line.trim_start().len())
            .min()
            .unwrap_or(0);
        let lines: Vec<&str> = self
            .0
            .iter()
            .enumerate()
            .map(|(at, line)| {
                if !kept(&at) {
                    line
                } else if line.trim().is_empty() {
                    ""
                } else {
                    line.get(indent..).unwrap_or(line)
                }
            })
            .collect();
        lines.join("\n") + "\n"
    }

    /// The lines, from 1, that continue a string literal started on an
    /// earlier line.
    fn literal_lines(&self) -> Vec<usize> {
        let mut lines = Vec::new();
        if let Ok(tokens) = self.0.join("\n").parse::<TokenStream>() {
            Self::collect_literal_lines(tokens, &mut lines);
        }
        lines
    }

    fn collect_literal_lines(tokens: TokenStream, lines: &mut Vec<usize>) {
        for token in tokens {
            match token {
                TokenTree::Group(group) => Self::collect_literal_lines(group.stream(), lines),
                TokenTree::Literal(literal) => {
                    let span = literal.span();
                    lines.extend(span.start().line + 1..=span.end().line);
                }
                TokenTree::Ident(_) | TokenTree::Punct(_) => {}
            }
        }
    }
}
