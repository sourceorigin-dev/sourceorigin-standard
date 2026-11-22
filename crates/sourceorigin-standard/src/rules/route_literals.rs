//! `route-literals`: the modules `standard.toml` lists own the HTTP paths.

use std::path::Path;

use syn::{Expr, Lit};

use super::walk::{Inspect, Scope, Walk};
use super::{Finding, Repository};
use crate::config;

/// The `Router` methods that take a path first.
const ROUTER_METHODS: [&str; 4] = ["route", "route_service", "nest", "nest_service"];

/// The path literals outside test code that route or redirect in one file.
struct RouteLiterals<'a> {
    path: &'a Path,
    findings: Vec<Finding>,
}

impl RouteLiterals<'_> {
    /// Records `argument` as `what`'s path when it is a string literal
    /// starting with `/`.
    fn record(&mut self, argument: Option<&Expr>, what: &str) {
        let Some(Expr::Lit(literal)) = argument else {
            return;
        };
        let Lit::Str(text) = &literal.lit else {
            return;
        };
        if !text.value().starts_with('/') {
            return;
        }
        self.findings.push(Finding {
            path: self.path.to_owned(),
            line: text.span().start().line,
            message: format!(
                "`{what}` takes the path `{}` as a literal: name it in a module \
                 `route-modules` in {} lists, and use that",
                text.value(),
                config::PATH
            ),
        });
    }
}

impl Inspect for RouteLiterals<'_> {
    fn method_call(&mut self, call: &syn::ExprMethodCall, scope: Scope) {
        let method = call.method.to_string();
        if !scope.test && ROUTER_METHODS.contains(&method.as_str()) {
            self.record(call.args.first(), &format!(".{method}"));
        }
    }

    fn call(&mut self, call: &syn::ExprCall, scope: Scope) {
        let Expr::Path(function) = &*call.func else {
            return;
        };
        let mut segments = function.path.segments.iter().rev();
        let (Some(constructor), Some(ty)) = (segments.next(), segments.next()) else {
            return;
        };
        if !scope.test && ty.ident == "Redirect" {
            self.record(
                call.args.first(),
                &format!("Redirect::{}", constructor.ident),
            );
        }
    }
}

impl Repository {
    /// Each path literal passed to a `Router` method or a `Redirect`
    /// outside test code, examples and the modules `route-modules` lists,
    /// and each listed module that no file is. Nothing when the list is
    /// empty.
    pub(super) fn route_literals(&self) -> Vec<Finding> {
        let listed = &self.config.route_modules;
        if listed.is_empty() {
            return Vec::new();
        }
        let modules = |path: &Path| {
            self.modules
                .of_file(path)
                .iter()
                .map(|module| module.join("::"))
                .collect::<Vec<_>>()
        };
        let mut findings: Vec<Finding> = listed
            .iter()
            .filter(|entry| {
                !self
                    .sources
                    .iter()
                    .any(|source| modules(&source.path).contains(&entry.value))
            })
            .map(|entry| Finding {
                path: config::PATH.into(),
                line: entry.line,
                message: format!(
                    "`{}` is in `route-modules`, but no file is that module: remove it",
                    entry.value
                ),
            })
            .collect();
        for source in &self.sources {
            let owns_routes = modules(&source.path).iter().any(|module| {
                listed.iter().any(|entry| {
                    *module == entry.value || module.starts_with(&format!("{}::", entry.value))
                })
            });
            if owns_routes || source.generated || self.modules.is_example(&source.path) {
                continue;
            }
            let mut literals = RouteLiterals {
                path: &source.path,
                findings: Vec::new(),
            };
            Walk::file(source, self.modules.is_test(&source.path), &mut literals);
            findings.extend(literals.findings);
        }
        findings
    }
}
