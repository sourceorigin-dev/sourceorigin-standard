//! `boxed_dyn_error_in_api`: owned `dyn Error` in a library's public API.

use rustc_hir::def_id::LocalDefId;
use rustc_hir::{FieldDef, ImplItem, ImplItemKind, Item, ItemKind, TraitItem, TraitItemKind};
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::ty::{self, Ty, TyCtxt, TypeSuperVisitable, TypeVisitable, TypeVisitor};
use rustc_session::config::CrateType;

use crate::error::ErrorTrait;
use crate::report::Report;

dylint_linting::declare_lint! {
    /// Flags an owned `dyn Error` (`Box<dyn Error + Send + Sync>`,
    /// `Arc<dyn Error>`, or of a trait with `Error` as a supertrait) in an
    /// exported function's signature, field, type alias, or associated type
    /// of an exported type, in a library crate, through type aliases. Use
    /// the crate's own error type: a `thiserror` enum callers can match on.
    ///
    /// Not flagged: a borrowed `&dyn Error`, which takes any error (what its
    /// methods return is checked), binaries, private items, and trait impls'
    /// method signatures, which the trait sets.
    pub BOXED_DYN_ERROR_IN_API,
    Warn,
    "an owned `dyn Error` trait object in a library's public API"
}

dylint_linting::declare_lint_pass!(BoxedDynErrorInApi => [BOXED_DYN_ERROR_IN_API]);

impl<'tcx> LateLintPass<'tcx> for BoxedDynErrorInApi {
    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
        let def_id = item.owner_id.def_id;
        match item.kind {
            ItemKind::Fn { .. } => Self::check_fn(cx, def_id),
            ItemKind::TyAlias(..) => Self::check_type(cx, def_id),
            _ => {}
        }
    }

    fn check_impl_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx ImplItem<'tcx>) {
        let def_id = item.owner_id.def_id;
        match item.kind {
            ImplItemKind::Fn(..) if cx.tcx.trait_impl_of_assoc(def_id.to_def_id()).is_none() => {
                Self::check_fn(cx, def_id);
            }
            ImplItemKind::Type(..) => {
                let impl_id = cx.tcx.local_parent(def_id);
                let owner = cx
                    .tcx
                    .type_of(impl_id)
                    .instantiate_identity()
                    .skip_norm_wip();
                if owner
                    .ty_adt_def()
                    .and_then(|adt| adt.did().as_local())
                    .is_some_and(|adt| cx.effective_visibilities.is_exported(adt))
                {
                    Self::report(cx, def_id, Self::type_of(cx, def_id));
                }
            }
            _ => {}
        }
    }

    fn check_trait_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx TraitItem<'tcx>) {
        if let TraitItemKind::Fn(..) = item.kind {
            Self::check_fn(cx, item.owner_id.def_id);
        }
    }

    fn check_field_def(&mut self, cx: &LateContext<'tcx>, field: &'tcx FieldDef<'tcx>) {
        Self::check_type(cx, field.def_id);
    }
}

impl BoxedDynErrorInApi {
    /// Checks the parameters and return type of the function `def_id`.
    fn check_fn(cx: &LateContext<'_>, def_id: LocalDefId) {
        if Self::exported(cx, def_id) {
            let sig = cx
                .tcx
                .fn_sig(def_id)
                .instantiate_identity()
                .skip_norm_wip()
                .skip_binder();
            Self::report(cx, def_id, sig.inputs_and_output.iter());
        }
    }

    /// Checks the type of the field or type alias `def_id`.
    fn check_type(cx: &LateContext<'_>, def_id: LocalDefId) {
        if Self::exported(cx, def_id) {
            Self::report(cx, def_id, Self::type_of(cx, def_id));
        }
    }

    /// The type of `def_id`, as an iterator of one.
    fn type_of<'tcx>(cx: &LateContext<'tcx>, def_id: LocalDefId) -> std::iter::Once<Ty<'tcx>> {
        std::iter::once(
            cx.tcx
                .type_of(def_id)
                .instantiate_identity()
                .skip_norm_wip(),
        )
    }

    /// Whether a library crate exports `def_id`.
    fn exported(cx: &LateContext<'_>, def_id: LocalDefId) -> bool {
        cx.tcx
            .crate_types()
            .iter()
            .any(|crate_type| matches!(crate_type, CrateType::Rlib | CrateType::Dylib))
            && cx.effective_visibilities.is_exported(def_id)
    }

    /// Reports the item `def_id` when `tys`, the types it shows, hold an
    /// owned `dyn Error`.
    fn report<'tcx>(
        cx: &LateContext<'tcx>,
        def_id: LocalDefId,
        tys: impl IntoIterator<Item = Ty<'tcx>>,
    ) {
        let Some(error) = ErrorTrait::of(cx) else {
            return;
        };
        let mut objects = DynErrors {
            cx,
            error,
            found: None,
        };
        for ty in tys {
            ty.visit_with(&mut objects);
        }
        if let Some(found) = objects.found
            && let ty::Dynamic(predicates, ..) = found.kind()
            && let Some(principal) = predicates.principal_def_id()
        {
            let found = cx.tcx.def_path_str(principal);
            let name = cx.tcx.def_path_str(def_id);
            let span = cx
                .tcx
                .def_ident_span(def_id)
                .unwrap_or_else(|| cx.tcx.def_span(def_id));
            Report::new(
                BOXED_DYN_ERROR_IN_API,
                format!("`{name}` exposes an owned `dyn {found}` in the public API"),
                "use the crate's own error type, a `thiserror` enum callers can match on",
            )
            .emit(cx, cx.tcx.local_def_id_to_hir_id(def_id), span);
        }
    }
}

/// The first owned `dyn Error` in the types it visits.
struct DynErrors<'a, 'tcx> {
    cx: &'a LateContext<'tcx>,
    error: ErrorTrait,
    found: Option<Ty<'tcx>>,
}

impl<'tcx> TypeVisitor<TyCtxt<'tcx>> for DynErrors<'_, 'tcx> {
    fn visit_ty(&mut self, ty: Ty<'tcx>) {
        if let ty::Ref(_, borrowed, _) = ty.kind()
            && self.error.is_dyn(self.cx, *borrowed)
        {
            borrowed.super_visit_with(self);
            return;
        }
        if self.found.is_none() && self.error.is_dyn(self.cx, ty) {
            self.found = Some(ty);
        }
        ty.super_visit_with(self);
    }
}
