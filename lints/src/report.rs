//! How every lint reports: what is wrong, what to write instead, and how to
//! challenge the lint when it is wrong.

use clippy_utils::diagnostics::span_lint_hir_and_then;
use rustc_hir::HirId;
use rustc_lint::{LateContext, Lint};
use rustc_span::Span;

/// A lint's finding, which [`Report::emit`] reports.
pub(crate) struct Report {
    lint: &'static Lint,
    message: String,
    help: String,
}

impl Report {
    /// `message` says what is wrong, `help` what to write instead.
    pub(crate) fn new(
        lint: &'static Lint,
        message: impl Into<String>,
        help: impl Into<String>,
    ) -> Self {
        Self {
            lint,
            message: message.into(),
            help: help.into(),
        }
    }

    /// Reports at `span`, at the lint level of `hir_id`, so an `#[expect]`
    /// there covers it.
    pub(crate) fn emit(self, cx: &LateContext<'_>, hir_id: HirId, span: Span) {
        let name = self.lint.name_lower();
        span_lint_hir_and_then(cx, self.lint, hir_id, span, self.message, |diag| {
            diag.help(self.help);
            diag.note(format!(
                "if the lint is wrong here, say why with `#[cfg_attr(dylint_lib = \
                 \"standard_lints\", expect({name}, reason = \"...\"))]`, or change the lint in \
                 sourceorigin-standard"
            ));
        });
    }
}
