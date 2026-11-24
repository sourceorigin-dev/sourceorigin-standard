//! Test code, which several rules exempt: what `#[cfg(test)]` or a test
//! attribute marks. The module walk reads it for the files that are test
//! code as a whole.

use syn::punctuated::Punctuated;
use syn::{Attribute, Meta, Token};

/// An item's attributes, read for whether they mark test code.
pub(super) trait TestAttributes {
    /// Whether a `cfg` among them holds only in tests: it requires `test`
    /// or a feature named for tests, as `#[cfg(test)]`,
    /// `#[cfg(all(test, unix))]` and
    /// `#[cfg(any(test, feature = "test-util"))]` do.
    fn cfg_test(&self) -> bool;

    /// Whether they mark test code: [`cfg_test`](Self::cfg_test), or a test
    /// attribute such as `#[test]` or `#[tokio::test]`.
    fn test(&self) -> bool;
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

    fn test(&self) -> bool {
        self.cfg_test()
            || self.iter().any(|attribute| {
                attribute
                    .path()
                    .segments
                    .last()
                    .is_some_and(|segment| segment.ident == "test")
            })
    }
}

/// A `cfg` predicate.
trait Predicate {
    /// Whether it holds only when `test` or a feature named for tests
    /// does.
    fn requires_test(&self) -> bool;

    /// Whether the feature `name` is named for tests: a `-` or `_`
    /// separated segment of it is `test`, `tests` or `testing`, such as
    /// `test-util`, `integration_tests` or `testing`, but not `attestation`
    /// or `latest-api`.
    fn names_tests(name: &str) -> bool {
        name.split(['-', '_'])
            .any(|segment| matches!(segment, "test" | "tests" | "testing"))
    }
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
            Self::NameValue(feature) => {
                feature.path.is_ident("feature")
                    && matches!(
                        &feature.value,
                        syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(name), .. })
                            if Self::names_tests(&name.value())
                    )
            }
        }
    }
}
