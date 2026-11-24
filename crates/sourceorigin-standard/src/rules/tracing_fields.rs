//! `tracing-fields`: telemetry never records the fields `standard.toml`
//! denies.

use std::path::Path;

use proc_macro2::{Spacing, TokenStream, TokenTree};
use syn::spanned::Spanned;

use super::walk::{Function, Inspect, Scope, Walk};
use super::{Finding, Repository};
use crate::config::{self, Listed};

/// The `tracing` macros that record fields.
const MACROS: &[&str] = &[
    "trace",
    "debug",
    "info",
    "warn",
    "error",
    "event",
    "span",
    "trace_span",
    "debug_span",
    "info_span",
    "warn_span",
    "error_span",
];

/// The denied fields outside test code that one file's events, spans and
/// `#[instrument]` functions record.
struct Fields<'a> {
    path: &'a Path,
    denied: &'a [Listed],
    findings: Vec<Finding>,
}

impl Inspect for Fields<'_> {
    fn macro_call(&mut self, call: &syn::Macro, scope: Scope) {
        let Some(last) = call.path.segments.last() else {
            return;
        };
        let name = last.ident.to_string();
        let other_crate = call.path.segments.len() > 1
            && call
                .path
                .segments
                .first()
                .is_some_and(|first| first.ident != "tracing");
        if scope.test || other_crate || !MACROS.contains(&name.as_str()) {
            return;
        }
        let line = last.ident.span().start().line;
        let mut span_name = name.ends_with("span");
        for argument in Self::arguments(call.tokens.clone()) {
            if let Some(field) = Self::field(&argument) {
                self.check(&field, &format!("`{name}!` records the field"), line);
            } else if matches!(argument.first(), Some(TokenTree::Literal(_)))
                && !std::mem::take(&mut span_name)
            {
                break;
            }
        }
    }

    fn function(&mut self, function: Function<'_>, scope: Scope) {
        let Some(attribute) = function.attributes.iter().find(|attribute| {
            attribute
                .path()
                .segments
                .last()
                .is_some_and(|segment| segment.ident == "instrument")
        }) else {
            return;
        };
        if scope.test {
            return;
        }
        let line = attribute.span().start().line;
        let mut skipped = Vec::new();
        let mut skip_all = false;
        if let syn::Meta::List(list) = &attribute.meta {
            for argument in Self::arguments(list.tokens.clone()) {
                match argument.as_slice() {
                    [TokenTree::Ident(ident)] if ident == "skip_all" => skip_all = true,
                    [TokenTree::Ident(ident), TokenTree::Group(group)] if ident == "skip" => {
                        skipped.extend(group.stream().into_iter().filter_map(
                            |token| match token {
                                TokenTree::Ident(ident) => Some(ident.to_string()),
                                _ => None,
                            },
                        ));
                    }
                    [TokenTree::Ident(ident), TokenTree::Group(group)] if ident == "fields" => {
                        for field in Self::arguments(group.stream()) {
                            if let Some(field) = Self::field(&field) {
                                self.check(&field, "`#[instrument]` records the field", line);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        if skip_all {
            return;
        }
        for input in &function.signature.inputs {
            let syn::FnArg::Typed(typed) = input else {
                continue;
            };
            let mut bindings = Bindings(Vec::new());
            syn::visit::Visit::visit_pat(&mut bindings, &typed.pat);
            for binding in bindings.0 {
                if !skipped.contains(&binding) {
                    self.check(&binding, "`#[instrument]` records the argument", line);
                }
            }
        }
    }
}

/// The names an argument's pattern binds, each of which `#[instrument]`
/// records: `token` for `token: &str`, and both of `(token, id): (_, _)`.
struct Bindings(Vec<String>);

impl<'ast> syn::visit::Visit<'ast> for Bindings {
    fn visit_pat_ident(&mut self, pattern: &'ast syn::PatIdent) {
        self.0.push(pattern.ident.to_string());
        syn::visit::visit_pat_ident(self, pattern);
    }
}

impl Fields<'_> {
    /// `tokens` split at their top-level commas.
    fn arguments(tokens: TokenStream) -> Vec<Vec<TokenTree>> {
        let tokens: Vec<TokenTree> = tokens.into_iter().collect();
        tokens
            .split(|token| matches!(token, TokenTree::Punct(punct) if punct.as_char() == ','))
            .map(<[TokenTree]>::to_vec)
            .collect()
    }

    /// Records `field`, which `what` records at `line`, when a denied entry
    /// is a run of its whole dotted parts: `query` denies `db.query.text`,
    /// and `url.query` denies `http.url.query` but not `db.query.text`.
    fn check(&mut self, field: &str, what: &str, line: usize) {
        let dotted = format!(".{field}.");
        let denied = self
            .denied
            .iter()
            .find(|entry| dotted.contains(&format!(".{}.", entry.value)))
            .map(|entry| entry.value.as_str());
        if let Some(denied) = denied {
            self.findings.push(Finding {
                path: self.path.to_owned(),
                line,
                message: format!(
                    "{what} `{field}`, and telemetry never holds a `{denied}` \
                     (`denied-fields` in {}): leave it out, or skip it",
                    config::PATH
                ),
            });
        }
    }

    /// The name of the field `argument` records, such as `http.path` for
    /// `http.path = %path`, `token` for `?token`, or `user_agent` for
    /// `"user_agent" = %agent`; `None` for a span's name, the message and
    /// its format arguments, and `target:`, `parent:` or `name:`.
    fn field(argument: &[TokenTree]) -> Option<String> {
        let mut tokens = argument
            .iter()
            .skip_while(|token| {
                matches!(token, TokenTree::Punct(punct) if matches!(punct.as_char(), '%' | '?'))
            })
            .peekable();
        let assigns = |token: Option<&&TokenTree>| {
            matches!(token, Some(TokenTree::Punct(punct))
                if punct.as_char() == '=' && punct.spacing() == Spacing::Alone)
        };
        if let Some(TokenTree::Literal(literal)) = tokens.peek() {
            let name =
                syn::parse2::<syn::LitStr>(TokenTree::Literal(literal.clone()).into()).ok()?;
            tokens.next();
            return assigns(tokens.peek()).then(|| name.value());
        }
        let mut name = String::new();
        for token in tokens {
            match token {
                TokenTree::Ident(ident) => name.push_str(&ident.to_string()),
                TokenTree::Punct(punct) if punct.as_char() == '.' => name.push('.'),
                TokenTree::Punct(_) if assigns(Some(&token)) => break,
                TokenTree::Group(_) | TokenTree::Literal(_) | TokenTree::Punct(_) => return None,
            }
        }
        (!name.is_empty()).then_some(name)
    }
}

impl Repository {
    /// Each field outside test code a `tracing` event, span or
    /// `#[instrument]` function records whose name, or a run of its dotted
    /// parts, `denied-fields` lists.
    pub(super) fn tracing_fields(&self) -> Vec<Finding> {
        let denied = &self.config.denied_fields;
        if denied.is_empty() {
            return Vec::new();
        }
        let mut findings = Vec::new();
        for source in self.sources.iter().filter(|source| !source.generated) {
            let mut fields = Fields {
                path: &source.path,
                denied,
                findings: Vec::new(),
            };
            Walk::file(source, self.modules.is_test(&source.path), &mut fields);
            findings.extend(fields.findings);
        }
        findings
    }
}
