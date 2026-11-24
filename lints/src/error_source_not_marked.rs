//! `error_source_not_marked`: `thiserror` errors that hide their source.

use rustc_hir::{Attribute, FieldDef, HirId, Item, ItemKind, VariantData};
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::{Symbol, sym};

use crate::error::ErrorTrait;
use crate::report::Report;

dylint_linting::declare_lint! {
    /// Flags a field of a `thiserror` error type whose type is an error (see
    /// `ErrorTrait::is_optional_error`), unless it is named `source` or
    /// marked `#[source]` or `#[from]`. Mark it `#[source]`, or `#[from]` to
    /// convert from it, and leave it out of the `#[error]` message.
    ///
    /// Not flagged: `#[error(transparent)]` types and variants (an enum's
    /// `#[error]` stands for each variant without one), and variants that
    /// already have a source.
    pub ERROR_SOURCE_NOT_MARKED,
    Warn,
    "an error field of a `thiserror` type that isn't marked as its source"
}

dylint_linting::declare_lint_pass!(ErrorSourceNotMarked => [ERROR_SOURCE_NOT_MARKED]);

impl<'tcx> LateLintPass<'tcx> for ErrorSourceNotMarked {
    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
        let Some(error) = ErrorTrait::of(cx) else {
            return;
        };
        let attributes = Attributes::new();
        let own = attributes.error(cx, item.hir_id());
        let variants: Vec<(Option<&Attribute>, &VariantData<'_>)> = match item.kind {
            ItemKind::Struct(_, _, ref data) => vec![(own, data)],
            ItemKind::Enum(_, _, ref def) => def
                .variants
                .iter()
                .map(|variant| (attributes.error(cx, variant.hir_id).or(own), &variant.data))
                .collect(),
            _ => return,
        };
        for (attribute, data) in variants {
            let Some(attribute) = attribute else {
                continue;
            };
            if Attributes::is_transparent(attribute)
                || data
                    .fields()
                    .iter()
                    .any(|field| attributes.is_source(cx, field))
            {
                continue;
            }
            for field in data.fields() {
                let ty = cx
                    .tcx
                    .type_of(field.def_id)
                    .instantiate_identity()
                    .skip_norm_wip();
                if error.is_optional_error(cx, ty) {
                    Report::new(
                        ERROR_SOURCE_NOT_MARKED,
                        "this error is not marked as the error's source",
                        "mark it `#[source]`, or `#[from]` to convert from it, and leave it out \
                         of the `#[error]` message",
                    )
                    .emit(cx, field.hir_id, field.span);
                }
            }
        }
    }
}

/// The `thiserror` attributes the lint reads.
struct Attributes {
    /// `#[error]`, which `thiserror` derives `Display` from.
    error: Symbol,
    /// `#[source]`, and the field name that implies it.
    source: Symbol,
}

impl Attributes {
    fn new() -> Self {
        Self {
            error: Symbol::intern("error"),
            source: Symbol::intern("source"),
        }
    }

    /// The `#[error]` attribute on the item or variant `hir_id`.
    fn error<'a>(&self, cx: &'a LateContext<'_>, hir_id: HirId) -> Option<&'a Attribute> {
        cx.tcx
            .hir_attrs(hir_id)
            .iter()
            .find(|attribute| attribute.has_name(self.error))
    }

    /// Whether `attribute` is `#[error(transparent)]`.
    fn is_transparent(attribute: &Attribute) -> bool {
        attribute
            .meta_item_list()
            .is_some_and(|items| items.iter().any(|item| item.has_name(sym::transparent)))
    }

    /// Whether `thiserror` takes `field` as its variant's source: it is named
    /// `source`, or marked `#[source]` or `#[from]`.
    fn is_source(&self, cx: &LateContext<'_>, field: &FieldDef<'_>) -> bool {
        field.ident.name == self.source
            || cx
                .tcx
                .hir_attrs(field.hir_id)
                .iter()
                .any(|attribute| attribute.has_any_name(&[self.source, sym::from]))
    }
}
