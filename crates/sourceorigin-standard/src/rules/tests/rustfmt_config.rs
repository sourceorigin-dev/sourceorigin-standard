use super::{Fixture, INHERITS};
use crate::rules::Rule;

#[test]
fn rustfmt_sets_the_standard_settings() {
    let fixture = Fixture::new();
    fixture.add_crate("core", INHERITS).write(
        "rustfmt.toml",
        "edition = \"2021\"\nnewline_style = \"Unix\"\nmax_width = 120\n",
    );
    assert_eq!(
        fixture.violations(Rule::RustfmtConfig),
        [
            "rustfmt.toml:1: `edition` has another value: set `edition = \"2024\"` \
             (rustfmt-config)",
            "rustfmt.toml:1: `use_field_init_shorthand` is unset: add \
             `use_field_init_shorthand = true` (rustfmt-config)",
        ]
    );
    fixture.fix_rule(Rule::RustfmtConfig);
    assert_eq!(
        fixture.violations(Rule::RustfmtConfig),
        Vec::<String>::new()
    );
}

#[test]
fn fix_writes_a_missing_rustfmt_toml() {
    let fixture = Fixture::new();
    fixture
        .add_crate("core", INHERITS)
        .remove("rustfmt.toml")
        .fix_rule(Rule::RustfmtConfig);
    assert_eq!(
        fixture.violations(Rule::RustfmtConfig),
        Vec::<String>::new()
    );
}
