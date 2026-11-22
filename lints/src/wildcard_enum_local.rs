//! `wildcard_enum_local`: catch-all arms on the workspace's own enums.

use clippy_utils::{is_refutable, peel_hir_pat_refs, recurse_or_patterns};
use rustc_hir::def::{CtorKind, DefKind, Res};
use rustc_hir::{Arm, Expr, ExprKind, MatchSource, Pat, PatExpr, PatExprKind, PatKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_middle::ty::{self, AdtDef, VariantDef};
use rustc_span::FileName;

use crate::report::Report;

dylint_linting::declare_lint! {
    /// Flags a `_` or catch-all binding arm (`other`, `other @ _`) in a
    /// `match` on an enum defined in this workspace. Name the remaining
    /// variants instead (`Kind::B | Kind::C =>`).
    ///
    /// Not flagged: enums of other crates (`std::io::ErrorKind`, serde,
    /// Docker types), `#[non_exhaustive]` enums of another workspace crate,
    /// guarded arms, and matches a macro expands to.
    pub WILDCARD_ENUM_LOCAL,
    Warn,
    "a catch-all arm in a `match` on an enum defined in this workspace"
}

dylint_linting::declare_lint_pass!(WildcardEnumLocal => [WILDCARD_ENUM_LOCAL]);

impl<'tcx> LateLintPass<'tcx> for WildcardEnumLocal {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if let Some(enum_match) = EnumMatch::of(cx, expr)
            && enum_match.in_workspace()
            && let Some(catch_all) = enum_match.catch_all()
        {
            let remaining = enum_match.remaining();
            if remaining.is_empty() {
                return;
            }
            let path = cx.tcx.def_path_str(enum_match.adt.did());
            let names: Vec<String> = remaining
                .iter()
                .map(|variant| format!("{path}::{}{}", variant.name, EnumMatch::fields(variant)))
                .collect();
            Report::new(
                WILDCARD_ENUM_LOCAL,
                format!("catch-all arm on `{path}`, an enum of this workspace"),
                format!(
                    "match its remaining variants by name: `{}`",
                    names.join(" | ")
                ),
            )
            .emit(cx, catch_all.hir_id, catch_all.pat.span);
        }
    }
}

/// A `match` the source writes on an enum.
struct EnumMatch<'a, 'tcx> {
    cx: &'a LateContext<'tcx>,
    adt: AdtDef<'tcx>,
    arms: &'tcx [Arm<'tcx>],
}

impl<'a, 'tcx> EnumMatch<'a, 'tcx> {
    /// `expr`, when it is a `match` on an enum or a reference to one that no
    /// macro expands to.
    fn of(cx: &'a LateContext<'tcx>, expr: &'tcx Expr<'tcx>) -> Option<Self> {
        if let ExprKind::Match(scrutinee, arms, MatchSource::Normal) = expr.kind
            && !expr.span.from_expansion()
            && let ty::Adt(adt, _) = cx.typeck_results().expr_ty(scrutinee).peel_refs().kind()
            && adt.is_enum()
        {
            Some(Self {
                cx,
                adt: *adt,
                arms,
            })
        } else {
            None
        }
    }

    /// Whether the enum is defined in the crate being linted or in another
    /// crate of its workspace or path dependency, which Cargo compiles from
    /// relative paths, unlike registry, git and vendored dependencies and
    /// `std`. A `#[non_exhaustive]` enum of another crate needs its
    /// catch-all arm.
    fn in_workspace(&self) -> bool {
        let did = self.adt.did();
        if did.is_local() {
            return true;
        }
        if self.adt.is_variant_list_non_exhaustive() {
            return false;
        }
        let source_map = self.cx.sess().source_map();
        match source_map.span_to_filename(self.cx.tcx.def_span(did)) {
            FileName::Real(file) => file.local_path().is_some_and(std::path::Path::is_relative),
            _ => false,
        }
    }

    /// The first catch-all arm, unless every arm is one.
    fn catch_all(&self) -> Option<&'tcx Arm<'tcx>> {
        let catch_all = self.arms.iter().find(|arm| Self::is_catch_all(arm))?;
        self.arms
            .iter()
            .any(|arm| !Self::is_catch_all(arm))
            .then_some(catch_all)
    }

    /// Whether `arm` matches every value, with no guard.
    fn is_catch_all(arm: &Arm<'_>) -> bool {
        arm.guard.is_none() && Self::matches_all(arm.pat)
    }

    /// Whether `pat` matches every value: `_`, or a binding (`other`,
    /// `other @ _`) of one.
    fn matches_all(pat: &Pat<'_>) -> bool {
        match peel_hir_pat_refs(pat).0.kind {
            PatKind::Wild | PatKind::Binding(_, _, _, None) => true,
            PatKind::Binding(_, _, _, Some(inner)) => Self::matches_all(inner),
            _ => false,
        }
    }

    /// The variants no unguarded arm matches in full.
    fn remaining(&self) -> Vec<&'tcx VariantDef> {
        let cx = self.cx;
        let mut remaining: Vec<&VariantDef> = self.adt.variants().iter().collect();
        for arm in self.arms.iter().filter(|arm| arm.guard.is_none()) {
            recurse_or_patterns(arm.pat, |pat| {
                let pat = peel_hir_pat_refs(pat).0;
                let covered = match pat.kind {
                    PatKind::Expr(PatExpr {
                        hir_id,
                        kind: PatExprKind::Path(path),
                        ..
                    }) => match cx.qpath_res(path, *hir_id) {
                        Res::Def(DefKind::Ctor(..), id) => Some(id),
                        _ => None,
                    },
                    PatKind::TupleStruct(path, fields, _)
                        if fields.iter().all(|field| !is_refutable(cx, field)) =>
                    {
                        cx.qpath_res(&path, pat.hir_id).opt_def_id()
                    }
                    PatKind::Struct(path, fields, _)
                        if fields.iter().all(|field| !is_refutable(cx, field.pat)) =>
                    {
                        cx.qpath_res(&path, pat.hir_id).opt_def_id()
                    }
                    _ => None,
                };
                if let Some(id) = covered {
                    remaining.retain(|variant| {
                        variant.ctor_def_id() != Some(id) && variant.def_id != id
                    });
                }
            });
        }
        remaining
    }

    /// The pattern of any of `variant`'s fields: `(_)`, `(..)`, ` { .. }`,
    /// or nothing for a unit variant.
    fn fields(variant: &VariantDef) -> &'static str {
        match variant.ctor_kind() {
            Some(CtorKind::Fn) if variant.fields.len() == 1 => "(_)",
            Some(CtorKind::Fn) => "(..)",
            Some(CtorKind::Const) => "",
            None => " { .. }",
        }
    }
}
