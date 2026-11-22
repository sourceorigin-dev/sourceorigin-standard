use super::{Fixture, INHERITS};
use crate::rules::Rule;

#[test]
fn listed_crates_use_json_only_in_tests() {
    let fixture = Fixture::new();
    fixture
        .add_crate("typed", INHERITS)
        .add_crate("free", INHERITS)
        .write(
            "crates/typed/src/lib.rs",
            "fn body() -> Value {\n    json!({ \"error\": \"invalid\" })\n}\n\
             fn log() {\n    println!(\"{}\", serde_json::json!(1));\n}\n\
             #[test]\nfn builds() {\n    let _ = json!({});\n}\n",
        )
        .write(
            "crates/free/src/lib.rs",
            "fn body() -> Value {\n    json!({})\n}\n",
        )
        .write(
            "standard.toml",
            "no-json-macro = [\n    \"typed\",\n    \"missing\",\n]\n",
        );
    let message = "`json!` builds a document no type checks: serialize a struct that describes \
                   it (no-json-macro)";
    assert_eq!(
        fixture.violations(Rule::NoJsonMacro),
        [
            format!("crates/typed/src/lib.rs:2: {message}"),
            format!("crates/typed/src/lib.rs:5: {message}"),
            "standard.toml:3: `missing` is in `no-json-macro`, but is no workspace crate: remove \
             it (no-json-macro)"
                .to_owned(),
        ]
    );
}
