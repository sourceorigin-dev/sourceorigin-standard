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

/// A Rust file that doesn't parse, which the rules can't read.
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
        let paths = workspace.files("*.rs")?;
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
            syntax,
            lints: lints.0,
            generated: false,
            comments,
            doc_blocks,
        })
    }
}

/// Collects a file's lint attributes.
struct LintAttributes(Vec<LintAttribute>);

impl<'ast> Visit<'ast> for LintAttributes {
    fn visit_attribute(&mut self, attribute: &'ast Attribute) {
        self.0.extend(LintAttribute::parse(attribute));
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
