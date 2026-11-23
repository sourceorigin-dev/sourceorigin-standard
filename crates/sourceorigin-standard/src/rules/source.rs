//! The repository's Rust files, parsed once for every rule that reads them.

use std::path::{Path, PathBuf};

use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{Attribute, Meta, Token};

use super::Error;
use super::comments::{Comment, DocBlock, Scan};
use super::level::Level;
use crate::workspace::Workspace;

/// A Rust file of the repository.
pub(super) struct Source {
    /// From the repository's root.
    pub(super) path: PathBuf,
    pub(super) text: String,
    pub(super) syntax: syn::File,
    /// Every lint attribute in it, inner and outer, at any depth.
    pub(super) lints: Vec<LintAttribute>,
    /// Whether `.gitattributes` marks it `linguist-generated`.
    pub(super) generated: bool,
    /// Its comments that aren't doc comments.
    pub(super) comments: Vec<Comment>,
    /// The docs of each of its items and modules.
    pub(super) doc_blocks: Vec<DocBlock>,
}

/// A file that doesn't parse as its format, which the rules can't read.
#[derive(Debug)]
pub(super) struct Unparsed {
    /// From the repository's root.
    pub(super) path: PathBuf,
    /// Where parsing failed, from 1.
    pub(super) line: usize,
    /// Why.
    pub(super) message: String,
}

impl Source {
    /// Every Rust file git tracks or would track under `root`: those that
    /// parse, and those that don't.
    pub(super) fn load_all(root: &Path) -> Result<(Vec<Self>, Vec<Unparsed>), Error> {
        let workspace = Workspace::new(root);
        let paths = workspace.files(&["*.rs"])?;
        let generated = workspace.with_attribute("linguist-generated", &paths)?;
        let mut sources = Vec::new();
        let mut unparsed = Vec::new();
        for path in &paths {
            let text = match std::fs::read_to_string(root.join(path)) {
                Ok(text) => text,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(source) => {
                    return Err(Error::Read {
                        path: PathBuf::from(path),
                        source,
                    });
                }
            };
            match Self::parse(PathBuf::from(path), &text) {
                Ok(mut source) => {
                    source.generated = generated.contains(path.as_str());
                    sources.push(source);
                }
                Err(failure) => unparsed.push(failure),
            }
        }
        Ok((sources, unparsed))
    }

    /// Parses `text`, the file at `path`.
    pub(super) fn parse(path: PathBuf, text: &str) -> Result<Self, Unparsed> {
        let unparsed = |path: PathBuf, start: proc_macro2::LineColumn, message: String| Unparsed {
            path,
            line: start.line,
            message,
        };
        let syntax = syn::parse_file(text)
            .map_err(|error| unparsed(path.clone(), error.span().start(), error.to_string()))?;
        let tokens: proc_macro2::TokenStream =
            text.parse().map_err(|error: proc_macro2::LexError| {
                unparsed(path.clone(), error.span().start(), error.to_string())
            })?;
        let (comments, doc_blocks) = Scan::of(text, tokens);
        let mut lints = LintAttributes(Vec::new());
        lints.visit_file(&syntax);
        Ok(Self {
            path,
            text: text.to_owned(),
            syntax,
            lints: lints.0,
            generated: false,
            comments,
            doc_blocks,
        })
    }

    /// The macros and attribute that name a file relative to the one they
    /// are in.
    const RELATIVE: [&str; 4] = ["include!", "include_str!", "include_bytes!", "#[path"];

    /// Whether `text`, part of a Rust file, may name another file by a
    /// path relative to its own: an `include!`, `include_str!` or
    /// `include_bytes!`, or a `#[path]` attribute or a `cfg_attr` holding
    /// one, even in a comment.
    pub(super) fn names_files(text: &str) -> bool {
        Self::RELATIVE.iter().any(|name| text.contains(name))
            || text.match_indices("cfg_attr(").any(|(at, _)| {
                let attribute = &text[at..];
                let attribute = &attribute[..attribute.find(']').unwrap_or(attribute.len())];
                attribute
                    .match_indices("path")
                    .any(|(path, _)| attribute[path + 4..].trim_start().starts_with('='))
            })
    }
}

/// Collects a file's lint attributes, those a `macro_rules!` body writes
/// included.
struct LintAttributes(Vec<LintAttribute>);

impl<'ast> Visit<'ast> for LintAttributes {
    fn visit_attribute(&mut self, attribute: &'ast Attribute) {
        self.0.extend(LintAttribute::parse(attribute));
    }

    fn visit_item_macro(&mut self, item: &'ast syn::ItemMacro) {
        if item.mac.path.is_ident("macro_rules") {
            self.tokens(item.mac.tokens.clone());
        }
        syn::visit::visit_item_macro(self, item);
    }
}

impl LintAttributes {
    /// Collects the attributes `tokens` write, `#[…]` and `#![…]`, at any
    /// depth.
    fn tokens(&mut self, tokens: proc_macro2::TokenStream) {
        let mut tokens = tokens.into_iter().peekable();
        while let Some(token) = tokens.next() {
            match token {
                proc_macro2::TokenTree::Punct(punct) if punct.as_char() == '#' => {
                    let line = punct.span().start().line;
                    if let Some(proc_macro2::TokenTree::Punct(bang)) = tokens.peek()
                        && bang.as_char() == '!'
                    {
                        tokens.next();
                    }
                    if let Some(proc_macro2::TokenTree::Group(group)) = tokens.peek()
                        && group.delimiter() == proc_macro2::Delimiter::Bracket
                        && let Ok(meta) = syn::parse2::<Meta>(group.stream())
                    {
                        self.0.extend(
                            LintAttribute::levels_and_lints(&meta)
                                .into_iter()
                                .map(|(level, lints)| LintAttribute { level, lints, line }),
                        );
                    }
                }
                proc_macro2::TokenTree::Group(group) => self.tokens(group.stream()),
                proc_macro2::TokenTree::Punct(_)
                | proc_macro2::TokenTree::Ident(_)
                | proc_macro2::TokenTree::Literal(_) => {}
            }
        }
    }
}

/// An attribute setting lints' level, such as
/// `#[expect(clippy::panic, reason = "…")]`, directly or through
/// `cfg_attr`.
#[derive(Debug)]
pub(super) struct LintAttribute {
    pub(super) level: Level,
    /// The lints it names, such as `clippy::panic`.
    pub(super) lints: Vec<String>,
    /// Its line, from 1.
    pub(super) line: usize,
}

impl LintAttribute {
    /// The lint attributes `attribute` is: none, one, or each of those a
    /// `cfg_attr` holds.
    pub(super) fn parse(attribute: &Attribute) -> Vec<Self> {
        let line = attribute.span().start().line;
        Self::levels_and_lints(&attribute.meta)
            .into_iter()
            .map(|(level, lints)| Self { level, lints, line })
            .collect()
    }

    /// The level and lints of each lint attribute `meta` holds, looking
    /// through `cfg_attr`.
    fn levels_and_lints(meta: &Meta) -> Vec<(Level, Vec<String>)> {
        let Meta::List(list) = meta else {
            return Vec::new();
        };
        let Ok(arguments) = list.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
        else {
            return Vec::new();
        };
        let level = match list.path.get_ident().map(ToString::to_string).as_deref() {
            Some("allow") => Level::Allow,
            Some("expect") => Level::Expect,
            Some("warn") => Level::Warn,
            Some("deny") => Level::Deny,
            Some("forbid") => Level::Forbid,
            Some("cfg_attr") => {
                return arguments
                    .iter()
                    .skip(1)
                    .flat_map(Self::levels_and_lints)
                    .collect();
            }
            _ => return Vec::new(),
        };
        let lints = arguments
            .iter()
            .filter_map(|meta| match meta {
                Meta::Path(path) => Some(
                    path.segments
                        .iter()
                        .map(|segment| segment.ident.to_string())
                        .collect::<Vec<_>>()
                        .join("::"),
                ),
                Meta::List(_) | Meta::NameValue(_) => None,
            })
            .collect();
        vec![(level, lints)]
    }

    /// Whether it expects `lint`.
    pub(super) fn expects(&self, lint: &str) -> bool {
        self.level == Level::Expect && self.lints.iter().any(|name| name == lint)
    }
}
