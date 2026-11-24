//! `free_fn_with_owner`: free functions that belong to a type.

use clippy_utils::paths::{PathNS, lookup_path_str};
use rustc_hir::{Item, ItemKind};
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::ty::{self, Ty};
use rustc_span::sym;

use crate::code::Code;
use crate::report::Report;

dylint_linting::declare_lint! {
    /// Flags a free function whose first parameter is a type of this crate
    /// (`T`, `&T`, `&mut T` or `Box<T>`). Make it a method on `T`, taking
    /// `self`, `&self`, `&mut self` or `self: Box<Self>`.
    ///
    /// Not flagged: `main`, axum handlers (the first parameter is an
    /// extractor), test code, and functions a macro expands to.
    pub FREE_FN_WITH_OWNER,
    Warn,
    "a free function whose first parameter is a type of this crate"
}

dylint_linting::declare_lint_pass!(FreeFnWithOwner => [FREE_FN_WITH_OWNER]);

impl<'tcx> LateLintPass<'tcx> for FreeFnWithOwner {
    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
        if let ItemKind::Fn { ident, .. } = item.kind
            && ident.name != sym::main
            && !item.span.from_expansion()
            && !cx.in_tests(item.hir_id())
            && let Some(&first) = cx
                .tcx
                .fn_sig(item.owner_id)
                .instantiate_identity()
                .skip_norm_wip()
                .skip_binder()
                .inputs()
                .first()
            && let Some(owner) = Self::owner(first)
            && !Self::is_extractor(cx, owner)
        {
            let owner = cx.tcx.def_path_str(owner.did());
            Report::new(
                FREE_FN_WITH_OWNER,
                format!("`{ident}` takes `{owner}` first but isn't its method"),
                format!("make it a method in `impl {owner}`, taking `self` in the first parameter's place"),
            )
            .emit(cx, item.hir_id(), ident.span);
        }
    }
}

impl FreeFnWithOwner {
    /// Whether `adt` implements axum's `FromRequestParts` or `FromRequest`:
    /// a handler takes it, and a handler can't be a method.
    fn is_extractor(cx: &LateContext<'_>, adt: ty::AdtDef<'_>) -> bool {
        [
            "axum_core::extract::FromRequestParts",
            "axum_core::extract::FromRequest",
        ]
        .into_iter()
        .flat_map(|path| lookup_path_str(cx.tcx, PathNS::Type, path))
        .flat_map(|extractor| cx.tcx.all_impls(extractor))
        .any(|implementation| {
            cx.tcx
                .type_of(implementation)
                .instantiate_identity()
                .skip_norm_wip()
                .ty_adt_def()
                .is_some_and(|implementer| implementer.did() == adt.did())
        })
    }

    /// The type of this crate a first parameter of type `ty` passes: `T`,
    /// `&T`, `&mut T` or `Box<T>`.
    fn owner(ty: Ty<'_>) -> Option<ty::AdtDef<'_>> {
        let ty = match ty.kind() {
            ty::Ref(_, inner, _) => *inner,
            _ if ty.is_box() => ty.boxed_ty()?,
            _ => ty,
        };
        match ty.kind() {
            ty::Adt(adt, _) if adt.did().is_local() => Some(*adt),
            _ => None,
        }
    }
}
