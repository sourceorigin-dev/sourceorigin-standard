//! `duplicate-items`: a function body is written once.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::Path;

use proc_macro2::{Delimiter, LineColumn, TokenStream, TokenTree};
use syn::visit::{self, Visit};

use super::walk::{Function, Inspect, Scope, Walk};
use super::{Finding, Repository};

/// The fewest tokens a body holds for a copy of it to count: below 20,
/// piqueld's, sourceorigin's and DirkEngine's copies are one-line helpers
/// rather than logic to share.
const MIN_TOKENS: usize = 20;

/// A token of a file, at its start.
struct Token {
    at: LineColumn,
    /// Whether it is an identifier, keywords included.
    ident: bool,
    /// The token as written, or its delimiter.
    text: String,
}

impl Token {
    /// The tokens of `tokens` in order, delimiters included.
    fn flatten(tokens: TokenStream, into: &mut Vec<Self>) {
        for token in tokens {
            match token {
                TokenTree::Group(group) => {
                    let (open, close) = match group.delimiter() {
                        Delimiter::Parenthesis => ("(", ")"),
                        Delimiter::Brace => ("{", "}"),
                        Delimiter::Bracket => ("[", "]"),
                        Delimiter::None => ("", ""),
                    };
                    into.push(Self::other(group.span_open().start(), open));
                    Self::flatten(group.stream(), into);
                    into.push(Self::other(group.span_close().start(), close));
                }
                TokenTree::Ident(ident) => into.push(Self {
                    at: ident.span().start(),
                    ident: true,
                    text: ident.to_string(),
                }),
                TokenTree::Punct(punct) => {
                    into.push(Self::other(
                        punct.span().start(),
                        &punct.as_char().to_string(),
                    ));
                }
                TokenTree::Literal(literal) => {
                    into.push(Self::other(literal.span().start(), &literal.to_string()));
                }
            }
        }
    }

    fn other(at: LineColumn, text: &str) -> Self {
        Self {
            at,
            ident: false,
            text: text.to_owned(),
        }
    }
}

/// The local bindings of a function: its parameters and the names its
/// patterns bind (`let`, closures, `match`, `for`), which start lowercase,
/// unlike the unit structs and variants a pattern names; and where it names
/// a struct's field, `field: value` or shorthand, in an expression or a
/// pattern, which is part of its shape as written.
#[derive(Default)]
struct Locals {
    names: BTreeSet<String>,
    fields: BTreeSet<(usize, usize)>,
}

impl Locals {
    fn field(&mut self, member: &syn::Member) {
        if let syn::Member::Named(ident) = member {
            let at = ident.span().start();
            self.fields.insert((at.line, at.column));
        }
    }
}

impl<'ast> Visit<'ast> for Locals {
    fn visit_pat_ident(&mut self, pattern: &'ast syn::PatIdent) {
        let name = pattern.ident.to_string();
        if name.starts_with(|first: char| first.is_lowercase() || first == '_') {
            self.names.insert(name);
        }
        visit::visit_pat_ident(self, pattern);
    }

    fn visit_field_value(&mut self, field: &'ast syn::FieldValue) {
        self.field(&field.member);
        visit::visit_field_value(self, field);
    }

    fn visit_field_pat(&mut self, field: &'ast syn::FieldPat) {
        self.field(&field.member);
        visit::visit_field_pat(self, field);
    }
}

/// Where a body sits.
struct Occurrence<'a> {
    path: &'a Path,
    line: usize,
    name: String,
}

/// The function bodies of one file, by the hash of their shape: their
/// tokens, each local binding renamed by the order it first appears in.
struct Bodies<'a, 'b> {
    path: &'a Path,
    tokens: &'b [Token],
    by_shape: &'b mut BTreeMap<u64, Vec<Occurrence<'a>>>,
}

impl Bodies<'_, '_> {
    /// Whether each reference `signature` takes or returns is `&mut`: a body
    /// written once for `&` and once for `&mut`, as match ergonomics allow, is
    /// two functions, not a copy.
    fn mutability(signature: &syn::Signature) -> Vec<bool> {
        let reference = |ty: &syn::Type| match ty {
            syn::Type::Reference(reference) => Some(reference.mutability.is_some()),
            _ => None,
        };
        let inputs = signature.inputs.iter().filter_map(|input| match input {
            syn::FnArg::Receiver(receiver) => match &receiver.kind {
                syn::ReceiverKind::Reference(_, _, mutability) => Some(mutability.is_some()),
                _ => None,
            },
            syn::FnArg::Typed(typed) => reference(&typed.ty),
        });
        let output = match &signature.output {
            syn::ReturnType::Type(_, ty) => reference(ty),
            syn::ReturnType::Default => None,
        };
        inputs.chain(output).collect()
    }
}

impl Inspect for Bodies<'_, '_> {
    fn function(&mut self, function: Function<'_>, scope: Scope) {
        let (signature, block) = (function.signature, function.body);
        if scope.test {
            return;
        }
        let (open, close) = (
            block.brace_token.span.open().start(),
            block.brace_token.span.close().start(),
        );
        let position = |at: &LineColumn| (at.line, at.column);
        let start = self
            .tokens
            .partition_point(|token| position(&token.at) <= position(&open));
        let end = self
            .tokens
            .partition_point(|token| position(&token.at) < position(&close));
        let Some(body) = self.tokens.get(start..end) else {
            return;
        };
        if body.len() < MIN_TOKENS {
            return;
        }
        let mut locals = Locals::default();
        locals.visit_signature(signature);
        locals.visit_block(block);
        let mut names: HashMap<&str, usize> = HashMap::new();
        let mut shape = DefaultHasher::new();
        Self::mutability(signature).hash(&mut shape);
        for (at, token) in body.iter().enumerate() {
            let after = |text: &str| {
                at.checked_sub(1)
                    .and_then(|before| body.get(before))
                    .is_some_and(|before| before.text == text)
            };
            let path = after(".")
                || (after(":")
                    && at
                        .checked_sub(2)
                        .and_then(|before| body.get(before))
                        .is_some_and(|before| before.text == ":"));
            let field = locals.fields.contains(&(token.at.line, token.at.column));
            if token.ident && !path && !field && locals.names.contains(&token.text) {
                let next = names.len();
                Some(*names.entry(&token.text).or_insert(next)).hash(&mut shape);
            } else {
                None::<usize>.hash(&mut shape);
                token.text.hash(&mut shape);
            }
        }
        self.by_shape
            .entry(shape.finish())
            .or_default()
            .push(Occurrence {
                path: self.path,
                line: signature.ident.span().start().line,
                name: signature.ident.to_string(),
            });
    }
}

impl Repository {
    /// Each function body outside test code with at least [`MIN_TOKENS`]
    /// tokens that repeats an earlier one, local bindings renamed in the
    /// order they appear.
    pub(super) fn duplicate_items(&self) -> Vec<Finding> {
        let mut by_shape = BTreeMap::new();
        let mut tokens = Vec::new();
        for source in self.sources.iter().filter(|source| !source.generated) {
            tokens.clear();
            if let Ok(stream) = source.text.parse::<TokenStream>() {
                Token::flatten(stream, &mut tokens);
            }
            let mut bodies = Bodies {
                path: &source.path,
                tokens: &tokens,
                by_shape: &mut by_shape,
            };
            Walk::file(source, self.modules.is_test(&source.path), &mut bodies);
        }
        by_shape
            .into_values()
            .filter(|occurrences| occurrences.len() > 1)
            .flat_map(|occurrences| {
                let mut occurrences = occurrences.into_iter();
                let first = occurrences.next();
                occurrences.filter_map(move |occurrence| {
                    let first = first.as_ref()?;
                    Some(Finding {
                        path: occurrence.path.to_owned(),
                        line: occurrence.line,
                        message: format!(
                            "the body of `{}` repeats that of `{}` ({}:{}), names aside: \
                             write it once, on the type that owns it, and call it",
                            occurrence.name,
                            first.name,
                            first.path.display(),
                            first.line
                        ),
                    })
                })
            })
            .collect()
    }
}
