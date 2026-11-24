use super::{Fixture, INHERITS};
use crate::rules::Rule;

#[test]
fn every_crate_stays_within_its_ceiling() {
    let fixture = Fixture::new();
    fixture
        .add_crate("exact", INHERITS)
        .add_crate("under", INHERITS)
        .add_crate("over", INHERITS)
        .add_crate("unlisted", INHERITS)
        .write(
            "crates/exact/src/lib.rs",
            "#![expect(clippy::pedantic, reason = \"r\")]\n\
             #[cfg_attr(test, expect(clippy::panic, reason = \"r\"))]\n\
             fn f() {\n    #[expect(unused, reason = \"r\")]\n    let x = 0;\n}\n",
        )
        .write(
            "crates/under/src/lib.rs",
            "#[expect(dead_code, reason = \"r\")]\nfn f() {\n    // INVARIANT: one.\n    \
             let a = 0;\n    // INVARIANT: two,\n    // on two lines.\n    let b = 0;\n    \
             // SAFETY: a marked comment,\n    // INVARIANT: and a third invariant.\n    \
             let c = 0;\n}\n",
        )
        .write(
            "crates/over/src/lib.rs",
            "#[expect(dead_code, reason = \"r\")]\nfn f() {}\n#[allow(dead_code)]\nfn g() {}\n\
             #[expect(dead_code, reason = \"r\")]\nfn h() {}\n",
        )
        .write(
            "crates/unlisted/src/lib.rs",
            "#[expect(dead_code, reason = \"r\")]\nfn f() {}\n",
        )
        .write(
            "budget.toml",
            "[expect]\nexact = 3\nunder = 4\nover = 1\nremoved = 2\n\n[invariant]\nunder = 1\n",
        );
    assert_eq!(
        fixture.violations(Rule::Budget),
        [
            "budget.toml:1: `unlisted` has 1 `#[expect]` attributes, over its ceiling of 0: \
             remove one, or raise `unlisted` under `[expect]` in budget.toml, which the owner \
             reviews (budget)",
            "budget.toml:4: `over` has 2 `#[expect]` attributes, over its ceiling of 1: remove \
             one, or raise `over` under `[expect]` in budget.toml, which the owner reviews \
             (budget)",
            "budget.toml:5: `removed` is no workspace crate: remove it from `[expect]` (budget)",
            "budget.toml:8: `under` has 3 `// INVARIANT:` comments, over its ceiling of 1: \
             remove one, or raise `under` under `[invariant]` in budget.toml, which the owner \
             reviews (budget)",
        ]
    );
}
