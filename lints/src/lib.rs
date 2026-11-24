//! The engineering standard's rules that need types, as one Dylint library:
//! each module is a lint, and [`register_lints`] registers them all.
#![feature(rustc_private)]

dylint_linting::dylint_library!();

extern crate rustc_ast;
extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_middle;
extern crate rustc_session;
extern crate rustc_span;

mod code;
mod report;
mod string_literal_logic;
mod wildcard_enum_local;

/// Registers each lint with its pass: `lint => pass`.
macro_rules! register {
    ($store:ident: $($lint:path => $pass:expr,)+) => {
        $store.register_lints(&[$($lint),+]);
        $($store.register_late_lint_pass(Box::new(|_| Box::new($pass)));)+
    };
}

/// Registers every lint. Dylint loads the library and calls it by name.
#[expect(
    unsafe_code,
    reason = "Dylint finds the function by its unmangled name"
)]
#[expect(
    clippy::no_mangle_with_rust_abi,
    reason = "Dylint calls it with the Rust ABI of the compiler it loads into"
)]
#[unsafe(no_mangle)]
pub fn register_lints(sess: &rustc_session::Session, store: &mut rustc_lint::LintStore) {
    dylint_linting::init_config(sess);
    register!(store:
        wildcard_enum_local::WILDCARD_ENUM_LOCAL => wildcard_enum_local::WildcardEnumLocal,
        string_literal_logic::STRING_LITERAL_LOGIC => string_literal_logic::StringLiteralLogic,
    );
}
