use super::{Fixture, INHERITS};
use crate::config::{Config, ConfigError};
use std::path::PathBuf;

use crate::rules::{Rule, Violation, describe};

#[test]
fn a_deviation_skips_its_rule_until_the_repository_follows_it() {
    let fixture = Fixture::new();
    fixture.add_crate("core", "").write(
        "standard.toml",
        "[deviations]\nworkspace-lints = \"generated crates\"\n",
    );
    assert_eq!(
        fixture.violations(Rule::WorkspaceLints),
        Vec::<String>::new()
    );

    fixture.add_crate("core", INHERITS);
    assert_eq!(
        fixture.violations(Rule::WorkspaceLints),
        [
            "standard.toml:2: the repository follows `workspace-lints`: remove its deviation \
             (\"generated crates\") (workspace-lints)"
        ]
    );
}

#[test]
fn a_scoped_deviation_excuses_only_its_paths_and_crates() {
    let fixture = Fixture::new();
    fixture
        .add_crate("generated", "")
        .add_crate("vendored", "")
        .add_crate("written", "")
        .add_crate("clean", INHERITS)
        .write(
            "standard.toml",
            "[deviations.workspace-lints]\nreason = \"generated or vendored\"\n\
             paths = [\"crates/generated/**\", \"crates/gone/**\"]\n\
             crates = [\"vendored\", \"clean\"]\n",
        );
    assert_eq!(
        fixture.violations(Rule::WorkspaceLints),
        [
            "crates/written/Cargo.toml:1: no `[lints]`: add `[lints] workspace = true` \
             (workspace-lints)",
            "standard.toml:3: `crates/gone/**` in the deviation from `workspace-lints` \
             (\"generated or vendored\") excuses no violation: remove it (workspace-lints)",
            "standard.toml:4: `clean` in the deviation from `workspace-lints` (\"generated or \
             vendored\") excuses no violation: remove it (workspace-lints)",
        ]
    );
}

#[test]
fn a_deviation_needs_a_reason() {
    assert!(matches!(
        Config::parse("[deviations]\nworkspace-lints = \" \"\n"),
        Err(ConfigError::NoReason {
            rule: "workspace-lints",
            line: 2
        })
    ));
}

#[test]
fn a_scoped_deviation_needs_paths_or_crates() {
    assert!(matches!(
        Config::parse("[deviations]\nworkspace-lints = { reason = \"r\" }\n"),
        Err(ConfigError::NoScope {
            rule: "workspace-lints",
            line: 2
        })
    ));
}

#[test]
fn every_rule_is_described_by_its_name_and_doc() {
    let description = describe();
    for rule in Rule::ALL {
        let first = rule.doc().lines().next().expect("a doc").trim();
        assert!(
            description.contains(&format!("- `{}`: {first}", rule.name())),
            "{description}"
        );
    }
}

#[test]
fn a_violation_prints_its_fix_then_how_to_challenge_the_rule() {
    let violation = Violation {
        rule: Rule::WorkspaceLints,
        path: PathBuf::from("crates/core/Cargo.toml"),
        line: 1,
        message: "no `[lints]`: add `[lints] workspace = true`".to_owned(),
    };
    assert_eq!(
        violation.to_string(),
        "crates/core/Cargo.toml:1: [workspace-lints] no `[lints]`: add `[lints] workspace = \
         true`\n  If the rule is wrong here, ask the user, then add `workspace-lints = { reason \
         = \"<why>\", paths = [\"crates/core/Cargo.toml\"] }` under `[deviations]` in \
         standard.toml (a bare reason applies it to the whole repository), or change the rule \
         in sourceorigin-standard for all."
    );
}
