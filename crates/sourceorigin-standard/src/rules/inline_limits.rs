//! `inline-limits`: limits, timeouts and sizes are named constants.

use std::path::Path;

use syn::{Expr, ExprCall, Lit};

use super::walk::{Inspect, Scope, Walk};
use super::{Finding, Repository};

/// The inline `Duration`s of one file.
struct Durations<'a> {
    path: &'a Path,
    findings: Vec<Finding>,
}

impl Inspect for Durations<'_> {
    fn call(&mut self, call: &ExprCall, scope: Scope) {
        if scope.test || scope.constant {
            return;
        }
        let Expr::Path(function) = &*call.func else {
            return;
        };
        let segments: Vec<String> = function
            .path
            .segments
            .iter()
            .map(|segment| segment.ident.to_string())
            .collect();
        let [.., ty, constructor] = segments.as_slice() else {
            return;
        };
        if ty != "Duration"
            || !constructor.starts_with("from_")
            || !call.args.iter().all(Self::is_literal)
        {
            return;
        }
        self.findings.push(Finding {
            path: self.path.to_owned(),
            line: function
                .path
                .segments
                .first()
                .map_or(1, |segment| segment.ident.span().start().line),
            message: format!(
                "`Duration::{constructor}` with a literal is an inline limit: make it a \
                 named `const` with a doc comment"
            ),
        });
    }
}

impl Durations<'_> {
    /// Whether `expr` is a number, or arithmetic on numbers.
    fn is_literal(expr: &Expr) -> bool {
        match expr {
            Expr::Lit(literal) => matches!(literal.lit, Lit::Int(_) | Lit::Float(_)),
            Expr::Binary(binary) => {
                Self::is_literal(&binary.left) && Self::is_literal(&binary.right)
            }
            Expr::Unary(unary) => Self::is_literal(&unary.expr),
            Expr::Paren(paren) => Self::is_literal(&paren.expr),
            _ => false,
        }
    }
}

impl Repository {
    /// Each `Duration::from_*` with a literal outside a `const` or
    /// `static` item, a `const fn` and test code.
    pub(super) fn inline_limits(&self) -> Vec<Finding> {
        let mut findings = Vec::new();
        for source in self.sources.iter().filter(|source| !source.generated) {
            let mut durations = Durations {
                path: &source.path,
                findings: Vec::new(),
            };
            Walk::file(source, self.modules.is_test(&source.path), &mut durations);
            findings.extend(durations.findings);
        }
        findings
    }
}
