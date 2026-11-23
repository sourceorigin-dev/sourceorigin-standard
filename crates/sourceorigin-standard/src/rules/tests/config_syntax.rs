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
fn the_broken_manifest_is_found_whatever_cargo_says() {
    let fixture = Fixture::new();
    fixture.add_crate("bare", "").write(
        "crates/bare/Cargo.toml",
        "[package]\nname = \"bare\"\nedition = [\n",
    );
    for stderr in [
        "error: failed to load manifest for workspace member",
        "\u{1b}[1m\u{1b}[91merror\u{1b}[0m: unclosed array\n \u{1b}[94m--> \u{1b}[0mcrates/bare/Cargo.toml:3:11",
        "",
    ] {
        let unparsed = super::super::source::Unparsed::cargo(fixture.0.root(), stderr)
            .expect("git lists the manifests");
        assert_eq!(
            (unparsed.path.to_string_lossy().into_owned(), unparsed.line),
            ("crates/bare/Cargo.toml".to_owned(), 3),
            "{stderr:?}"
        );
    }
}
