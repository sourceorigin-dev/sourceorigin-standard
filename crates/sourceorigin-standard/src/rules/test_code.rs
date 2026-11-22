//! Test code, which several rules exempt: what `#[cfg(test)]` marks.

use syn::punctuated::Punctuated;
use syn::{Attribute, Meta, Token};

/// An item's attributes, read for whether they mark test code.
pub(super) trait TestAttributes {
    /// Whether a `cfg` among them requires `test`, as `#[cfg(test)]` and
    /// `#[cfg(all(test, unix))]` do.
    fn cfg_test(&self) -> bool;
}

impl TestAttributes for [Attribute] {
    fn cfg_test(&self) -> bool {
        self.iter().any(|attribute| {
            attribute.path().is_ident("cfg")
                && attribute
                    .parse_args::<Meta>()
                    .is_ok_and(|meta| meta.requires_test())
        })
    }
}

/// A `cfg` predicate.
trait Predicate {
    /// Whether it holds only when `test` does.
    fn requires_test(&self) -> bool;
}

impl Predicate for Meta {
    fn requires_test(&self) -> bool {
        match self {
            Self::Path(path) => path.is_ident("test"),
            Self::List(list) => {
                let Ok(predicates) =
                    list.parse_args_with(Punctuated::<Self, Token![,]>::parse_terminated)
                else {
                    return false;
                };
                if list.path.is_ident("all") {
                    predicates.iter().any(Self::requires_test)
                } else if list.path.is_ident("any") {
                    !predicates.is_empty() && predicates.iter().all(Self::requires_test)
                } else {
                    false
                }
            }
            Self::NameValue(_) => false,
        }
    }
}
