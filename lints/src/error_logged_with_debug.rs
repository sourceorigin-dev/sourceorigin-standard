//! `error_logged_with_debug`: errors a `tracing` event logs through `Debug`.

use clippy_utils::fn_def_id;
use rustc_hir::def_id::DefId;
use rustc_hir::{Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::ty::{self, Ty};
use rustc_span::{Span, Symbol, sym};

use crate::error::ErrorTrait;
use crate::report::Report;

dylint_linting::declare_lint! {
    /// Flags an error (see `ErrorTrait::is_error`) a `tracing` event formats
    /// through `Debug`: a `?error` or `name = ?error` field, or a `{:?}` or
    /// `{error:?}` message argument (also an `Option` or `Result` of one).
    /// Log it through `Display` with `%`, wrapped in the repository's helper
    /// that prints its source chain (`chain_helper` in `dylint.toml`).
    ///
    /// Not flagged: `?` fields holding an `Option` of an error, which has no
    /// `Display`, and `Debug` formatting outside `tracing` macros.
    pub ERROR_LOGGED_WITH_DEBUG,
    Warn,
    "an error a `tracing` event logs through `Debug`"
}

/// The pass, with the repository's source-chain helper.
pub(crate) struct ErrorLoggedWithDebug {
    /// What the help tells to wrap the error in.
    help: String,
}

dylint_linting::impl_lint_pass!(ErrorLoggedWithDebug => [ERROR_LOGGED_WITH_DEBUG]);

/// The lint's table in the linted workspace's `dylint.toml`.
#[derive(Default, serde::Deserialize)]
struct Config {
    /// The type that displays an error with its source chain, such as
    /// sourceorigin's `ErrorChain`, which the help names.
    chain_helper: Option<String>,
}

impl ErrorLoggedWithDebug {
    /// The pass, configured from `[error_logged_with_debug]` in
    /// `dylint.toml`.
    pub(crate) fn configured() -> Self {
        let config: Config = dylint_linting::config_or_default("error_logged_with_debug");
        let help = match config.chain_helper {
            Some(helper) => format!("log it through `Display` with `%{helper}(&error)`, which prints its source chain"),
            None => "log it through `Display` with `%`, wrapped in the repository's helper that prints \
                     its source chain (`chain_helper` under `[error_logged_with_debug]` in `dylint.toml` \
                     names it here)"
                .to_owned(),
        };
        Self { help }
    }
}

impl<'tcx> LateLintPass<'tcx> for ErrorLoggedWithDebug {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        let ExprKind::Call(_, [value]) = expr.kind else {
            return;
        };
        let (Some(function), Some(error)) = (fn_def_id(cx, expr), ErrorTrait::of(cx)) else {
            return;
        };
        let ty = cx.typeck_results().expr_ty(value).peel_refs();
        let (span, message) = if Self::is_tracing_debug(cx, function) && error.is_error(cx, ty) {
            (Self::written(value), "this error is logged through `Debug`")
        } else if Self::is_format_debug(cx, function)
            && Self::in_tracing(cx, expr.span)
            && Self::is_logged_error(cx, error, ty)
        {
            (
                expr.span.source_callsite(),
                "this error is formatted with `{:?}` in a `tracing` event",
            )
        } else {
            return;
        };
        Report::new(ERROR_LOGGED_WITH_DEBUG, message, self.help.clone()).emit(
            cx,
            expr.hir_id,
            span,
        );
    }
}

impl ErrorLoggedWithDebug {
    /// Where the event writes `value`, which the macro borrows: a lint on a
    /// span the macro writes wouldn't show. A shorthand field (`?error`)
    /// has no span of its own, so the whole event stands for it.
    fn written(value: &Expr<'_>) -> Span {
        match value.kind {
            ExprKind::AddrOf(_, _, inner) if value.span.from_expansion() => Self::written(inner),
            _ if value.span.from_expansion() => value.span.source_callsite(),
            _ => value.span,
        }
    }

    /// Whether `def_id` is `tracing::field::debug`, which a `?` field calls.
    fn is_tracing_debug(cx: &LateContext<'_>, def_id: DefId) -> bool {
        Self::is_tracing(cx, def_id)
            && cx.tcx.item_name(def_id) == sym::debug
            && cx
                .tcx
                .opt_parent(def_id)
                .is_some_and(|module| cx.tcx.item_name(module) == Symbol::intern("field"))
    }

    /// Whether `def_id` is `core::fmt::rt::Argument::new_debug`, which a
    /// `{:?}` argument of `format_args!` calls.
    fn is_format_debug(cx: &LateContext<'_>, def_id: DefId) -> bool {
        cx.tcx.crate_name(def_id.krate) == sym::core
            && cx.tcx.item_name(def_id) == Symbol::intern("new_debug")
    }

    /// Whether `span` comes from a `tracing` macro.
    fn in_tracing(cx: &LateContext<'_>, span: Span) -> bool {
        span.macro_backtrace().any(|expansion| {
            expansion
                .macro_def_id
                .is_some_and(|def_id| Self::is_tracing(cx, def_id))
        })
    }

    /// Whether `def_id` is `tracing`'s, or `tracing_core`'s, which it
    /// re-exports.
    fn is_tracing(cx: &LateContext<'_>, def_id: DefId) -> bool {
        matches!(
            cx.tcx.crate_name(def_id.krate).as_str(),
            "tracing" | "tracing_core"
        )
    }

    /// Whether `ty` is an error, or an `Option` or `Result` of one.
    fn is_logged_error<'tcx>(cx: &LateContext<'tcx>, error: ErrorTrait, ty: Ty<'tcx>) -> bool {
        match ty.kind() {
            ty::Adt(adt, args) if cx.tcx.is_diagnostic_item(sym::Result, adt.did()) => {
                error.is_error(cx, args.type_at(1))
            }
            _ => error.is_optional_error(cx, ty),
        }
    }
}
