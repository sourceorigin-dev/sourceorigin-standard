//! `string_literal_logic`: program logic on string literals and constants.

use clippy_utils::is_lang_item_or_ctor;
use rustc_ast::LitKind;
use rustc_hir::attrs::lang_items::LangItem;
use rustc_hir::def::{DefKind, Res};
use rustc_hir::def_id::DefId;
use rustc_hir::{Arm, BinOpKind, Expr, ExprKind, HirId, Pat, PatExpr, PatExprKind, PatKind, QPath};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_middle::ty::{self, Clause, Ty};
use rustc_span::{Span, sym};

use crate::code::Code;
use crate::report::Report;

dylint_linting::declare_lint! {
    /// Flags comparing a value against a string literal or a `&str`/`String`
    /// `const` or `static`: `==`/`!=` (also `== Some("…")`), `match` arms,
    /// and `starts_with`, `ends_with`, `contains`, `eq_ignore_ascii_case`,
    /// `strip_prefix`, `strip_suffix`, `find` or `split_once` with one.
    /// Parse the string into a type (an enum, a newtype) where it enters.
    ///
    /// Not flagged: parsing and display code (see `Exemption`), test code,
    /// empty literals, and what the crate's own macros write.
    pub STRING_LITERAL_LOGIC,
    Warn,
    "program logic that compares a value against a string literal"
}

dylint_linting::declare_lint_pass!(StringLiteralLogic => [STRING_LITERAL_LOGIC]);

/// The `str` methods that test or split a string by another.
const METHODS: [&str; 8] = [
    "starts_with",
    "ends_with",
    "contains",
    "eq_ignore_ascii_case",
    "strip_prefix",
    "strip_suffix",
    "find",
    "split_once",
];

impl<'tcx> LateLintPass<'tcx> for StringLiteralLogic {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if expr.span.from_expansion() {
            return;
        }
        let compared = match expr.kind {
            ExprKind::Binary(op, left, right)
                if matches!(op.node, BinOpKind::Eq | BinOpKind::Ne) =>
            {
                [left, right]
                    .into_iter()
                    .find_map(|operand| Self::constant(cx, operand))
            }
            ExprKind::MethodCall(method, receiver, [argument], _)
                if METHODS.contains(&method.ident.name.as_str())
                    && cx
                        .typeck_results()
                        .expr_ty_adjusted(receiver)
                        .peel_refs()
                        .is_str() =>
            {
                Self::constant(cx, argument)
            }
            _ => None,
        };
        if let Some((span, what)) = compared {
            Self::lint(
                cx,
                expr.hir_id,
                span,
                format!("comparing a value against {what}"),
            );
        }
    }

    fn check_arm(&mut self, cx: &LateContext<'tcx>, arm: &'tcx Arm<'tcx>) {
        let local_macro =
            arm.span.from_expansion() && !arm.span.in_external_macro(cx.sess().source_map());
        if !local_macro && let Some((span, what)) = Self::pattern(cx, arm.pat) {
            Self::lint(
                cx,
                arm.hir_id,
                span,
                format!("matching a value against {what}"),
            );
        }
    }
}

impl StringLiteralLogic {
    /// Reports `span`, a string the code at `hir_id` tests a value against,
    /// unless that code parses or displays values, or is a test.
    fn lint(cx: &LateContext<'_>, hir_id: HirId, span: Span, message: String) {
        if span.from_expansion() || cx.in_tests(hir_id) || Exemption::covers(cx, hir_id) {
            return;
        }
        Report::new(
            STRING_LITERAL_LOGIC,
            message,
            "parse the string into a type (an enum, a newtype) where it enters, in `FromStr`, \
             `TryFrom<&str>`, `From<&str>` or serde, and compare that",
        )
        .emit(cx, hir_id, span);
    }

    /// Where `expr` is a string that isn't empty, and what it is: a literal,
    /// a `&str` or `String` `const` or `static`, or `Some` of either.
    fn constant(cx: &LateContext<'_>, expr: &Expr<'_>) -> Option<(Span, &'static str)> {
        match expr.kind {
            ExprKind::Lit(lit) if matches!(lit.node, LitKind::Str(text, _) if !text.is_empty()) => {
                Some((expr.span, "a string literal"))
            }
            ExprKind::Path(ref path)
                if matches!(
                    cx.qpath_res(path, expr.hir_id),
                    Res::Def(
                        DefKind::Const { .. } | DefKind::AssocConst { .. } | DefKind::Static { .. },
                        _
                    )
                ) && Self::is_string(cx, cx.typeck_results().expr_ty(expr)) =>
            {
                Some((expr.span, "a string constant"))
            }
            ExprKind::Call(callee, [inner])
                if let ExprKind::Path(QPath::Resolved(None, path)) = callee.kind
                    && path.res.opt_def_id().is_some_and(|ctor| {
                        is_lang_item_or_ctor(cx, ctor, LangItem::OptionSome)
                    }) =>
            {
                Self::constant(cx, inner).map(|(_, what)| (expr.span, what))
            }
            _ => None,
        }
    }

    /// The first string literal or string constant `pat` matches, at any
    /// depth, that isn't an empty literal.
    fn pattern(cx: &LateContext<'_>, pat: &Pat<'_>) -> Option<(Span, &'static str)> {
        let mut found = None;
        pat.walk_short(|pat| {
            if let PatKind::Expr(PatExpr { kind, span, hir_id }) = pat.kind {
                found = match kind {
                    PatExprKind::Lit { lit, .. }
                        if matches!(lit.node, LitKind::Str(text, _) if !text.is_empty()) =>
                    {
                        Some((*span, "a string literal"))
                    }
                    PatExprKind::Path(path)
                        if matches!(
                            cx.qpath_res(path, *hir_id),
                            Res::Def(DefKind::Const { .. } | DefKind::AssocConst { .. }, _)
                        ) && Self::is_string(cx, cx.typeck_results().pat_ty(pat)) =>
                    {
                        Some((*span, "a string constant"))
                    }
                    _ => None,
                };
            }
            found.is_none()
        });
        found
    }

    /// Whether `ty` is `str`, `String`, or a reference to either.
    fn is_string(cx: &LateContext<'_>, ty: Ty<'_>) -> bool {
        let ty = ty.peel_refs();
        ty.is_str()
            || ty
                .ty_adt_def()
                .is_some_and(|adt| is_lang_item_or_ctor(cx, adt.did(), LangItem::String))
    }
}

/// The code that turns strings into values and back, where comparing them
/// is the point: impls of `Display`, `Debug`, `FromStr`, `TryFrom<&str>`,
/// `TryFrom<String>`, `From<&str>`, `From<String>` and serde's traits;
/// functions bounded by a serde trait (`deserialize_with`); inherent
/// functions that take a `&str` or `String` and return `Self`,
/// `Option<Self>` or `Result<Self, _>`; and `main`, which reads arguments.
struct Exemption;

impl Exemption {
    /// Whether the code at `hir_id` sits in such code.
    fn covers(cx: &LateContext<'_>, hir_id: HirId) -> bool {
        let mut item = Some(cx.tcx.hir_enclosing_body_owner(hir_id).to_def_id());
        while let Some(def_id) = item {
            let exempt = match cx.tcx.def_kind(def_id) {
                DefKind::Impl { of_trait: true } => Self::parses_or_displays(cx, def_id),
                DefKind::Fn => Self::is_main(cx, def_id) || Self::bounded_by_serde(cx, def_id),
                DefKind::AssocFn => {
                    Self::constructor(cx, def_id) || Self::bounded_by_serde(cx, def_id)
                }
                _ => false,
            };
            if exempt {
                return true;
            }
            item = cx.tcx.opt_parent(def_id);
        }
        false
    }

    /// Whether `fn_id` is the crate root's `main`, also in a test build,
    /// whose entry point is the test harness's.
    fn is_main(cx: &LateContext<'_>, fn_id: DefId) -> bool {
        cx.tcx.item_name(fn_id) == sym::main
            && cx.tcx.opt_parent(fn_id).is_some_and(DefId::is_crate_root)
    }

    /// Whether `impl_id` implements a parsing or display trait.
    fn parses_or_displays(cx: &LateContext<'_>, impl_id: DefId) -> bool {
        let trait_ref = cx.tcx.impl_trait_ref(impl_id).skip_binder();
        let trait_id = trait_ref.def_id;
        if Self::is_serde(cx, trait_id) {
            return true;
        }
        if cx.tcx.crate_name(trait_id.krate) != sym::core {
            return false;
        }
        match cx.tcx.item_name(trait_id).as_str() {
            "Display" | "Debug" | "FromStr" => true,
            "TryFrom" | "From" => StringLiteralLogic::is_string(cx, trait_ref.args.type_at(1)),
            _ => false,
        }
    }

    /// Whether `fn_id` is an inherent function that takes a `&str` or
    /// `String` and returns `Self`, `Option<Self>` or `Result<Self, _>`.
    fn constructor(cx: &LateContext<'_>, fn_id: DefId) -> bool {
        let Some(impl_id) = cx.tcx.inherent_impl_of_assoc(fn_id) else {
            return false;
        };
        let owner = cx
            .tcx
            .type_of(impl_id)
            .instantiate_identity()
            .skip_norm_wip();
        let sig = cx
            .tcx
            .fn_sig(fn_id)
            .instantiate_identity()
            .skip_norm_wip()
            .skip_binder();
        let output = sig.output();
        let returns_self = output == owner
            || matches!(output.kind(), ty::Adt(adt, args)
                if [sym::Option, sym::Result].into_iter().any(|wrapper| cx.tcx.is_diagnostic_item(wrapper, adt.did()))
                    && args.type_at(0) == owner);
        returns_self
            && sig
                .inputs()
                .iter()
                .any(|&input| StringLiteralLogic::is_string(cx, input))
    }

    /// Whether `fn_id` has a bound on a serde trait.
    fn bounded_by_serde(cx: &LateContext<'_>, fn_id: DefId) -> bool {
        cx.tcx
            .param_env(fn_id)
            .caller_bounds()
            .iter()
            .filter_map(Clause::as_trait_clause)
            .any(|bound| Self::is_serde(cx, bound.def_id()))
    }

    /// Whether `def_id` is serde's, which re-exports `serde_core`'s traits.
    fn is_serde(cx: &LateContext<'_>, def_id: DefId) -> bool {
        matches!(
            cx.tcx.crate_name(def_id.krate).as_str(),
            "serde" | "serde_core"
        )
    }
}
