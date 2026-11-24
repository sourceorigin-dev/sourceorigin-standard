use super::{Fixture, INHERITS};
use crate::rules::Rule;

#[test]
fn tests_may_expect_panic_and_print_but_never_unwrap() {
    let fixture = Fixture::new();
    fixture.add_crate("core", INHERITS).write(
        "clippy.toml",
        "allow-expect-in-tests = true\nallow-panic-in-tests = false\nallow-dbg-in-tests = true\n\
         allow-unwrap-in-tests = true\ndoc-valid-idents = [\"DirkEngine\", \"..\"]\n",
    );
    assert_eq!(
        fixture.violations(Rule::ClippyConfig),
        [
            "clippy.toml:2: `allow-panic-in-tests` has another value: set \
             `allow-panic-in-tests = true` (clippy-config)",
            "clippy.toml:4: `allow-unwrap-in-tests` lets tests unwrap: remove it, and `expect` \
             what should hold (clippy-config)",
        ]
    );
    fixture.fix_rule(Rule::ClippyConfig);
    assert_eq!(fixture.violations(Rule::ClippyConfig), Vec::<String>::new());
}
