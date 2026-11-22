use super::{Fixture, INHERITS};
use crate::rules::Rule;

#[test]
fn the_toolchain_is_stable_with_rustfmt_and_clippy() {
    let fixture = Fixture::new();
    fixture.add_crate("core", INHERITS).write(
        "rust-toolchain.toml",
        "[toolchain]\nchannel = \"1.85\"\ncomponents = [\"clippy\", \"llvm-tools\"]\n\
         targets = [\"wasm32-unknown-unknown\"]\n",
    );
    assert_eq!(
        fixture.violations(Rule::Toolchain),
        [
            "rust-toolchain.toml:2: the toolchain isn't the `stable` channel: set `channel = \
             \"stable\"`, which rustup keeps current (toolchain)",
            "rust-toolchain.toml:3: the toolchain lacks the `rustfmt` component: add it to \
             `components` (toolchain)",
        ]
    );
}

#[test]
fn fix_writes_a_missing_toolchain_file() {
    let fixture = Fixture::new();
    fixture
        .add_crate("core", INHERITS)
        .remove("rust-toolchain.toml");
    assert_eq!(
        fixture.violations(Rule::Toolchain),
        [
            "rust-toolchain.toml:1: no rust-toolchain.toml: pin the `stable` channel there \
          (toolchain)"
        ]
    );
    fixture.fix_rule(Rule::Toolchain);
    assert_eq!(fixture.violations(Rule::Toolchain), Vec::<String>::new());
}
