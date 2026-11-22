//! The UI tests: each lint's `ui/<lint>.rs` example produces the
//! diagnostics in its `.stderr`, with the library's other lints allowed.

use dylint_testing::ui::Test;

/// Every lint of the library, which each test allows but its own.
const LINTS: &[&str] = &[
    "wildcard_enum_local",
    "string_literal_logic",
    "error_source_not_marked",
];

/// The test of `lint` on the example `example`.
fn ui(lint: &str, example: &str) -> Test {
    let mut test = Test::example(env!("CARGO_PKG_NAME"), example);
    test.rustc_flags(
        LINTS
            .iter()
            .filter(|&&other| other != lint)
            .flat_map(|other| ["-A", other]),
    );
    test
}

/// One test per lint, on the example named after it.
macro_rules! lints {
    ($($lint:ident),+ $(,)?) => {
        $(
            #[test]
            fn $lint() {
                ui(stringify!($lint), stringify!($lint)).run();
            }
        )+
    };
}

lints!(
    wildcard_enum_local,
    string_literal_logic,
    error_source_not_marked,
);
