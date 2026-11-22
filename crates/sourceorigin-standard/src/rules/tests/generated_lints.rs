use super::{Fixture, INHERITS};
use crate::rules::Rule;

#[test]
fn generated_files_carry_no_lint_attributes() {
    let fixture = Fixture::new();
    fixture
        .add_crate("core", INHERITS)
        .write(
            ".gitattributes",
            "crates/core/src/generated.rs linguist-generated\n",
        )
        .write(
            "crates/core/src/lib.rs",
            "#[expect(clippy::pedantic, reason = \"generated\")]\nmod generated;\n\
             #[deny(missing_docs)]\nmod written;\n",
        )
        .write(
            "crates/core/src/generated.rs",
            "#![allow(clippy::all)]\npub struct Message;\n\
             #[cfg_attr(test, deny(missing_docs))]\npub fn f() {}\n",
        )
        .write("crates/core/src/written.rs", "#![warn(unused)]\n");
    assert_eq!(
        fixture.violations(Rule::GeneratedLints),
        [
            "crates/core/src/generated.rs:1: a generated file carries no lint attribute: put \
             it on the file's `mod` declaration (generated-lints)",
            "crates/core/src/generated.rs:3: a generated file carries no lint attribute: put \
             it on the file's `mod` declaration (generated-lints)",
        ]
    );
}
