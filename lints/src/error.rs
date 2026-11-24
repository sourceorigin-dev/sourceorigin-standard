//! `std::error::Error`, as the error lints recognise it.

use clippy_utils::ty::implements_trait;
use rustc_hir::def_id::DefId;
use rustc_lint::LateContext;
use rustc_middle::ty::{self, Ty};
use rustc_span::{Symbol, sym};

/// The `std::error::Error` trait.
#[derive(Clone, Copy)]
pub(crate) struct ErrorTrait(DefId);

impl ErrorTrait {
    /// The trait, from its diagnostic item.
    pub(crate) fn of(cx: &LateContext<'_>) -> Option<Self> {
        cx.tcx
            .get_diagnostic_item(Symbol::intern("Error"))
            .map(Self)
    }

    /// Whether `ty` is a `dyn` trait object of `Error`, or of a trait with
    /// `Error` as a supertrait.
    pub(crate) fn is_dyn<'tcx>(self, cx: &LateContext<'tcx>, ty: Ty<'tcx>) -> bool {
        matches!(ty.kind(), ty::Dynamic(..)) && implements_trait(cx, ty, self.0, &[])
    }

    /// Whether `ty` is an error: it implements `Error`, or is a `dyn Error`,
    /// or a `Box`, `Arc` or `Rc` of one.
    pub(crate) fn is_error<'tcx>(self, cx: &LateContext<'tcx>, ty: Ty<'tcx>) -> bool {
        match ty.kind() {
            ty::Adt(adt, args)
                if ty.is_box()
                    || [sym::Arc, sym::Rc]
                        .into_iter()
                        .any(|pointer| cx.tcx.is_diagnostic_item(pointer, adt.did())) =>
            {
                self.is_error(cx, args.type_at(0))
            }
            ty::Dynamic(..) => self.is_dyn(cx, ty),
            _ => implements_trait(cx, ty, self.0, &[]),
        }
    }

    /// Whether `ty` is an error, or an `Option` of one.
    pub(crate) fn is_optional_error<'tcx>(self, cx: &LateContext<'tcx>, ty: Ty<'tcx>) -> bool {
        match ty.kind() {
            ty::Adt(adt, args) if cx.tcx.is_diagnostic_item(sym::Option, adt.did()) => {
                self.is_error(cx, args.type_at(0))
            }
            _ => self.is_error(cx, ty),
        }
    }
}
