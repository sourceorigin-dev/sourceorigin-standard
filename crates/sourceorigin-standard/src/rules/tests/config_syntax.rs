use super::Fixture;
use crate::rules::{self, Rule};

#[test]
fn a_config_file_that_doesnt_parse_is_a_violation_and_the_rules_go_on() {
    let fixture = Fixture::new();
    fixture
        .add_crate("bare", "")
        .write("deny.toml", "[bans]\nwildcards = \"deny\"\n[licenses\n")
        .write("rustfmt.toml", "edition = \"2024\"\n")
        .write("budget.toml", "[expect]\nbare = \n")
        .write(".github/workflows/ci.yml", "on: push\njobs: [\n")
        .write("t3.json", "{\n  \"scripts\": [\n}\n")
        .write("justfile", "ok:\n    echo\n\nbad recipe(:\n");
    let violations: Vec<String> = fixture
        .violations(Rule::ConfigSyntax)
        .iter()
        .map(|violation| violation.split(": ").next().unwrap_or_default().to_owned())
        .collect();
    assert_eq!(
        violations,
        [
            ".github/workflows/ci.yml:3",
            "budget.toml:2",
            "deny.toml:3",
            "justfile:4",
            "t3.json:3",
        ]
    );
    assert_eq!(fixture.violations(Rule::WorkspaceLints).len(), 1);
}

#[test]
fn a_manifest_cargo_cant_read_is_the_one_violation() {
    let fixture = Fixture::new();
    fixture
        .add_crate("bare", "")
        .write("crates/bare/Cargo.toml", "[package\nname = \"bare\"\n");
    let violations = rules::check(fixture.0.root()).expect("the rules run");
    assert_eq!(
        violations
            .iter()
            .map(|violation| (
                violation.rule,
                violation.path.to_string_lossy(),
                violation.line
            ))
            .collect::<Vec<_>>(),
        [(Rule::ConfigSyntax, "crates/bare/Cargo.toml".into(), 1)]
    );
    assert!(
        rules::fix(fixture.0.root())
            .expect("nothing to fix")
            .is_empty()
    );
}

#[test]
fn a_cargo_configuration_cargo_cant_read_is_the_one_violation() {
    let fixture = Fixture::new();
    fixture
        .add_crate("bare", "")
        .write(".cargo/config.toml", "[alias]\nx = \"y\"\n[build\n");
    let violations = rules::check(fixture.0.root()).expect("the rules run");
    assert_eq!(
        violations
            .iter()
            .map(|violation| (
                violation.path.to_string_lossy().into_owned(),
                violation.line
            ))
            .collect::<Vec<_>>(),
        [(".cargo/config.toml".to_owned(), 3)]
    );
}

#[test]
fn a_message_naming_no_file_lands_on_the_root_manifest_and_one_naming_a_member_on_its_manifest() {
    let fixture = Fixture::new();
    fixture
        .add_crate("bare", "")
        .write(
            "crates/bare/Cargo.toml",
            "[package]\nname = \"bare\"\nedition = [\n",
        )
        .write("fixtures/broken/Cargo.toml", "[package\n");
    let place = |stderr: &str| {
        let unparsed = super::super::source::Unparsed::cargo(fixture.0.root(), stderr)
            .expect("the place is found");
        (unparsed.path.to_string_lossy().into_owned(), unparsed.line)
    };
    for stderr in ["error: failed to load manifest for workspace member", ""] {
        assert_eq!(place(stderr), ("Cargo.toml".to_owned(), 1), "{stderr:?}");
    }
    assert_eq!(
        place("error: failed to load manifest for workspace member `crates/bare`"),
        ("crates/bare/Cargo.toml".to_owned(), 3)
    );
}

#[test]
fn a_manifest_cargo_refuses_though_it_parses_is_the_one_cargo_names_with_its_whole_message() {
    let fixture = Fixture::new();
    fixture
        .add_crate("bare", "")
        .write(
            "crates/bare/Cargo.toml",
            "[package]\nname = \"bare\"\nedition = \"2077\"\n",
        )
        .write("fixtures/broken/Cargo.toml", "[package\n");
    let violations = rules::check(fixture.0.root()).expect("the rules run");
    let [violation] = violations.as_slice() else {
        panic!("one violation: {violations:?}");
    };
    assert_eq!(
        (
            violation.rule,
            violation.path.to_string_lossy(),
            violation.line
        ),
        (Rule::ConfigSyntax, "crates/bare/Cargo.toml".into(), 1)
    );
    assert!(
        violation.message.contains("Caused by:") && violation.message.contains("2077"),
        "{}",
        violation.message
    );
}

#[test]
fn the_innermost_file_cargo_names_is_the_one_and_a_warning_names_none() {
    let fixture = Fixture::new();
    fixture
        .add_crate("b", "\n[dependencies]\nz = { path = \"../z\" }\n")
        .write(
            "crates/z/Cargo.toml",
            "[package]\nname = \"z\"\nedition = \"2077\"\n",
        )
        .write("crates/z/src/lib.rs", "")
        .write(".cargo/config", "[build]\n");
    let violations = rules::check(fixture.0.root()).expect("the rules run");
    assert_eq!(
        violations
            .iter()
            .map(|violation| violation.path.to_string_lossy().into_owned())
            .collect::<Vec<_>>(),
        ["crates/z/Cargo.toml"]
    );
    let unparsed = super::super::source::Unparsed::cargo(
        fixture.0.root(),
        "warning: `.cargo/config` is deprecated in favor of `config.toml`\n\
         error: something cargo couldn't read",
    )
    .expect("the place is found");
    assert_eq!(unparsed.path.to_string_lossy(), "Cargo.toml");
}
