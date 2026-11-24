//! Where code sits, as the lints that exempt some code read it.

use clippy_utils::is_in_test;
use rustc_hir::HirId;
use rustc_lint::{LateContext, LintContext};

/// What a lint asks of the code around a node.
pub(crate) trait Code {
    /// Whether `hir_id` is test code: a `#[test]` function, a
    /// `#[cfg(test)]` module, or an integration test (a test crate whose root
    /// is `tests/<name>.rs` or `tests/<name>/main.rs`). A library's or binary's own test build is not, so
    /// its other code reads as in its normal build.
    fn in_tests(&self, hir_id: HirId) -> bool;
}

impl Code for LateContext<'_> {
    fn in_tests(&self, hir_id: HirId) -> bool {
        let integration = self.sess().is_test_crate()
            && self
                .sess()
                .local_crate_source_file()
                .and_then(rustc_span::RealFileName::into_local_path)
                .is_some_and(|root| {
                    root.ancestors()
                        .skip(1)
                        .take(2)
                        .any(|directory| directory.file_name().is_some_and(|name| name == "tests"))
                });
        integration || is_in_test(self.tcx, hir_id)
    }
}
