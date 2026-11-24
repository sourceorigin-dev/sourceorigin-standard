use super::{Fixture, INHERITS};
use crate::rules::Rule;

/// `reason = "…"`, which every expectation carries.
const EXPECT: &str = "expect(unsafe_code, reason = \"FFI\")";

#[test]
fn unsafe_code_is_expected_only_inside_the_listed_modules() {
    let fixture = Fixture::new();
    fixture
        .add_crate("core", INHERITS)
        .write(
            "crates/core/src/lib.rs",
            &format!(
                "#[{EXPECT}]\nmod sys;\nmod net;\nmod other;\n\
                 #[cfg_attr(unix, allow(dead_code), {EXPECT})]\nmod unix;\n\
                 #[path = \"platform/linux.rs\"]\nmod os;\n\
                 mod inline {{\n    #![{EXPECT}]\n}}\n\
                 fn read() {{\n    #[{EXPECT}]\n    let byte = 0;\n}}\n"
            ),
        )
        .write("crates/core/src/sys.rs", "")
        .write("crates/core/src/unix.rs", "")
        .write("crates/core/src/net/mod.rs", "mod socket;\n")
        .write(
            "crates/core/src/net/socket.rs",
            &format!("fn open() {{\n    #[{EXPECT}]\n    let fd = 0;\n}}\n"),
        )
        .write(
            "crates/core/src/platform/linux.rs",
            &format!("#![{EXPECT}]\n"),
        )
        .write("crates/core/src/other.rs", &format!("#![{EXPECT}]\n"))
        .write(
            "crates/core/build.rs",
            &format!("fn main() {{\n    #[{EXPECT}]\n    let raw = 0;\n}}\n"),
        )
        .write(
            "standard.toml",
            "unsafe-modules = [\n    \"core::sys\",\n    \"core::net\",\n    \"core::unix\",\n    \
             \"core::inline\",\n    \"core::os\",\n    \"build_script_build\",\n    \
             \"core::missing\",\n]\n",
        );
    assert_eq!(
        fixture.violations(Rule::UnsafeModules),
        [
            "crates/core/src/lib.rs:13: `core` expects `unsafe_code`, but no module \
             `unsafe-modules` in standard.toml lists holds it: list the smallest module that \
             needs it (unsafe-modules)",
            "crates/core/src/other.rs:1: `core::other` expects `unsafe_code`, but no module \
             `unsafe-modules` in standard.toml lists holds it: list the smallest module that \
             needs it (unsafe-modules)",
            "standard.toml:8: `core::missing` is in `unsafe-modules`, but holds no \
             `#[expect(unsafe_code, reason = \"…\")]`: remove it (unsafe-modules)",
        ]
    );
}

#[test]
fn a_file_two_binaries_share_must_be_listed_under_both() {
    let fixture = Fixture::new();
    fixture
        .add_crate("tools", INHERITS)
        .write(
            "crates/tools/src/bin/first.rs",
            "#[path = \"../shared.rs\"]\nmod shared;\nfn main() {}\n",
        )
        .write(
            "crates/tools/src/bin/second.rs",
            "#[path = \"../shared.rs\"]\nmod shared;\nfn main() {}\n",
        )
        .write("crates/tools/src/shared.rs", &format!("#![{EXPECT}]\n"))
        .write("standard.toml", "unsafe-modules = [\"first::shared\"]\n");
    assert_eq!(
        fixture.violations(Rule::UnsafeModules),
        [
            "crates/tools/src/shared.rs:1: `second::shared` expects `unsafe_code`, but no \
             module `unsafe-modules` in standard.toml lists holds it: list the smallest module \
             that needs it (unsafe-modules)"
        ]
    );
}

#[test]
fn a_file_that_doesnt_parse_is_a_violation_and_the_rules_go_on() {
    let fixture = Fixture::new();
    fixture
        .add_crate("core", INHERITS)
        .write("crates/core/src/lib.rs", &format!("#![{EXPECT}]\n"))
        .write("crates/core/tests/ui/broken.rs", "fn broken(\n");
    let syntax = fixture.violations(Rule::RustSyntax);
    assert_eq!(syntax.len(), 1, "{syntax:?}");
    assert!(
        syntax[0].starts_with("crates/core/tests/ui/broken.rs:"),
        "{syntax:?}"
    );
    assert_eq!(fixture.violations(Rule::UnsafeModules).len(), 1);
}
